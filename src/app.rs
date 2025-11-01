use crate::gl::Renderer;
use crate::solver::Solver;
use crate::stream::Streamliner;
use leptos::{logging::*, prelude::*};

// tunables
const LPF_ALPHA: f32 = 0.85; // 0..1 (higher = snappier)
const VMAX: f32 = 1.0; // clamp velocity magnitude in UV/s

#[component]
pub fn App() -> impl IntoView {
    view! { <FieldCanvas/> }
}

#[component]
fn FieldCanvas() -> impl IntoView {
    // UI state
    let field_lines = RwSignal::new(true);
    let seeds_per_axis = RwSignal::new(5u32);

    view! {
        <main class="min-h-screen flex">
            <aside class="w-96 p-4 border-r border-zinc-800 bg-zinc-950 text-zinc-100 space-y-4">
                <h2 class="font-semibold text-lg">Controls</h2>
                <p class="text-sm opacity-70">Leptos + WebGL2 + Tailwind</p>

                <div class="space-y-2">
                  <h3 class="font-semibold">Layers</h3>
                  <label class="flex items-center gap-2">
                    <input type="checkbox" bind:checked=field_lines />
                    <span>Show electric field lines</span>
                  </label>

                  <div class="mt-2">
                    <label class="text-sm block mb-1">
                      Streamline density (seeds / axis): <span class="font-mono">{move || seeds_per_axis.get()}</span>
                    </label>
                    <input
                      type="range" min="3" max="10" step="1"
                      prop:value=move || seeds_per_axis.get()
                      on:input=move |ev| {
                        if let Ok(v) = event_target_value(&ev).parse::<u32>() {
                            seeds_per_axis.set(v);
                        }
                      }
                      style="width:100%;"
                    />
                  </div>
                </div>

            </aside>

            <section class="flex-1 relative bg-black">
                <CanvasGL
                  class="w-full h-screen block"
                  field_lines=field_lines
                  seeds_per_axis=seeds_per_axis
                />
            </section>
        </main>
    }
}

#[component]
fn CanvasGL(
    class: &'static str,
    field_lines: RwSignal<bool>,
    seeds_per_axis: RwSignal<u32>,
) -> impl IntoView {
    use crate::solver::Charge;
    use wasm_bindgen::{JsCast, closure::Closure};
    use web_sys::{HtmlCanvasElement, HtmlDivElement, PointerEvent, window};

    let canvas_ref = NodeRef::new();
    let overlay_ref = NodeRef::new();

    // Reactive charges so DOM overlay updates positions
    let charges = RwSignal::new(vec![
        Charge {
            u: 0.35,
            v: 0.60,
            q: 1.0,
            vu: 0.0,
            vv: 0.0,
        },
        Charge {
            u: 0.65,
            v: 0.50,
            q: -1.0,
            vu: 0.0,
            vv: 0.0,
        },
    ]);

    // Which charge (if any) are we dragging?
    let dragging_idx = RwSignal::new(None::<usize>);

    // Mount: GL setup + global pointer listeners
    canvas_ref.on_load(move |_| {
        let canvas: HtmlCanvasElement = canvas_ref.get().expect("canvas");
        let ren = std::rc::Rc::new(std::cell::RefCell::new(
            Renderer::new(canvas.clone()).expect("gl"),
        ));
        let mut solver = Solver::new(256, 256);
        let mut stream = Streamliner::new();

        // Global pointermove
        {
            let on_move = Closure::<dyn FnMut(_)>::new(move |e: PointerEvent| {
                if let Some(i) = dragging_idx.get_untracked() {
                    // CSS -> UV (flip Y to y-up)
                    let (u, v) = {
                        let overlay: HtmlDivElement = overlay_ref.get_untracked().unwrap();
                        let rect = overlay.get_bounding_client_rect();
                        let u = ((e.client_x() as f64 - rect.left()) / rect.width()).clamp(0.0, 1.0)
                            as f32;
                        let v = (1.0
                            - ((e.client_y() as f64 - rect.top()) / rect.height()).clamp(0.0, 1.0))
                            as f32;
                        (u, v)
                    };
                    charges.update(|cs| {
                        if let Some(c) = cs.get_mut(i) {
                            c.u = u;
                            c.v = v;
                        }
                    });
                    e.prevent_default();
                }
            });
            window()
                .unwrap()
                .add_event_listener_with_callback("pointermove", on_move.as_ref().unchecked_ref())
                .unwrap();
            on_move.forget();
        }

        // Global pointerup/cancel
        {
            let on_up = Closure::<dyn FnMut(_)>::new(move |_e: PointerEvent| {
                // zero velocity on release
                if let Some(i) = dragging_idx.get_untracked() {
                    charges.update(|cs| {
                        if let Some(c) = cs.get_mut(i) {
                            c.vu = 0.0;
                            c.vv = 0.0;
                        }
                    });
                }
                dragging_idx.set(None);
            });
            let w = window().unwrap();
            w.add_event_listener_with_callback("pointerup", on_up.as_ref().unchecked_ref())
                .unwrap();
            w.add_event_listener_with_callback("pointercancel", on_up.as_ref().unchecked_ref())
                .unwrap();
            on_up.forget();
        }

        // Per-charge history for velocity (prev pos, filtered velocity)
        let mut prev_uv: Vec<(f32, f32)> =
            charges.get_untracked().iter().map(|c| (c.u, c.v)).collect();
        let mut vf_uv: Vec<(f32, f32)> = vec![(0.0, 0.0); prev_uv.len()];
        let mut last_time = 0.0f32; // seconds since start (from RAF)
        // RAF loop: read charges → solver, draw as before
        Renderer::start_raf_rc(ren.clone(), move |ren, time_sec| {
            // dt from RAF
            let dt = (time_sec - last_time).max(1e-4);
            last_time = time_sec;

            // keep history vectors in sync with charges length (in case you add/remove later)
            let cs_now = charges.get_untracked();
            if prev_uv.len() != cs_now.len() {
                prev_uv = cs_now.iter().map(|c| (c.u, c.v)).collect();
                vf_uv = vec![(0.0, 0.0); cs_now.len()];
            }

            // sync positions and compute velocities
            solver.clear();
            for (i, c) in cs_now.iter().copied().enumerate() {
                solver.add(c.u, c.v, c.q);

                // instantaneous velocity from frame-to-frame delta
                let (pu, pv) = prev_uv[i];
                let vu_i = (c.u - pu) / dt;
                let vv_i = (c.v - pv) / dt;

                // low-pass filter toward vu_i, vv_i
                let (mut vuf, mut vvf) = vf_uv[i];
                vuf += LPF_ALPHA * (vu_i - vuf);
                vvf += LPF_ALPHA * (vv_i - vvf);

                // clamp magnitude
                let mag = (vuf * vuf + vvf * vvf).sqrt();
                if mag > VMAX {
                    let s = VMAX / mag;
                    vuf *= s;
                    vvf *= s;
                }

                // if not dragging this charge, gently decay toward 0 (optional but nice)
                if dragging_idx.get_untracked() != Some(i) {
                    let decay = 0.8; // 0..1 per frame; lower = faster fade
                    vuf *= decay;
                    vvf *= decay;
                }

                // push velocity to solver
                if vuf.abs() > 1e-6 || vvf.abs() > 1e-6 {
                    solver.set_velocity(i, vuf, vvf);
                } else {
                    solver.set_velocity(i, 0.0, 0.0);
                }

                // update history
                prev_uv[i] = (c.u, c.v);
                vf_uv[i] = (vuf, vvf);
            }

            // total E
            solver.step(time_sec);
            ren.borrow_mut()
                .update_field_texture(solver.w as i32, solver.h as i32, &solver.field);

            // streamlines (your current “every frame” is fine)
            stream.set_seeds(seeds_per_axis.get_untracked());
            if field_lines.get_untracked() {
                let lines = stream.recompute(&solver, 0.8, 600);
                ren.borrow_mut().update_lines(&lines);
            }

            // B overlay
            let (w, h) = (solver.w as i32, solver.h as i32);
            {
                let bz = solver.compute_b_from_velocities(1.0);
                let r = ren.borrow_mut();
                r.update_b_texture(w, h, bz);
            }

            // draw
            let mut r = ren.borrow_mut();
            r.draw();
            r.draw_b_overlay(2.0, 0.7);
            if field_lines.get_untracked() {
                r.draw_lines();
            }
        });
    });

    // UI: canvas + absolutely-positioned 16px charge buttons
    view! {
        <div class="relative">
            <canvas node_ref=canvas_ref class=class></canvas>

            <div node_ref=overlay_ref class="pointer-events-none absolute inset-0">
                {charges.get_untracked().iter().enumerate() // TODO: react here when the charges vec actually updates!
                    .map(|(i, _)| {
                    view! {
                        <button
                            aria-label=format!("Charge {}", i + 1)
                            class="pointer-events-auto absolute -translate-x-1/2 -translate-y-1/2
                                   w-4 h-4 rounded-full bg-white/90 ring-2 ring-black/50
                                   hover:ring-cyan-400 cursor-grab active:cursor-grabbing"
                            style = move || {
                                let charge = charges.get()[i];
                                let left_pct = charge.u * 100.0;
                                let top_pct  = (1.0 - charge.v) * 100.0; // solver y-up -> CSS from top
                                format!("left:{left_pct}%; top:{top_pct}%;")
                            }
                            on:pointerdown=move |ev| {
                                ev.stop_propagation();
                                if let Some(target) = ev.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) {
                                    let _ = target.set_pointer_capture(ev.pointer_id());
                                }
                                dragging_idx.set( Some(i));
                            }
                        />
                    }
                })
                .collect::<Vec<_>>()}
            </div>
        </div>
    }
}
