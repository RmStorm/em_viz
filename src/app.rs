use crate::perf::{self, Scope};
use crate::state::AppState;
use crate::wgpu_renderer::WgpuRenderer;
use crate::{camera, picking};
use glam::Vec3;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::HtmlInputElement;

#[component]
pub fn App() -> impl IntoView {
    view! { <FieldCanvas/> }
}

#[component]
fn FieldCanvas() -> impl IntoView {
    let charges3d = vec![
        crate::em3d::Charge3D {
            pos: Vec3::new(0.0, 0.0, 0.4),
            q: 1.0,
            vel: Vec3::ZERO,
        },
        crate::em3d::Charge3D {
            pos: Vec3::new(0.0, 0.0, -0.4),
            q: -1.0,
            vel: Vec3::ZERO,
        },
    ];

    let app = AppState::new(charges3d, 14.0);

    view! {
      <main class="h-screen flex overflow-hidden">
        <aside class="w-96 h-full overflow-y-auto p-4 border-r border-zinc-800 bg-zinc-950 text-zinc-100 space-y-4">
          <h2 class="font-semibold text-lg">Controls</h2>
          <p class="text-sm opacity-70">Leptos + WebGPU + Tailwind</p>

          <div class="space-y-6">
            <section class="space-y-2">
              <h3 class="font-semibold text-sm uppercase tracking-wide opacity-70">Layers</h3>
              <label class="flex items-center gap-2 text-sm">
                <input type="checkbox"
                  prop:checked=move || app.show_e.get()
                  on:input=move |ev| {
                    if let Some(target) = ev.target() {
                      let input: HtmlInputElement = target.unchecked_into();
                      app.show_e.set(input.checked());
                    }
                  }/>
                "Show Electric (E)"
              </label>
              <label class="flex items-center gap-2 text-sm opacity-40">
                <input type="checkbox" disabled=true />
                "Show Magnetic (B) — coming soon"
              </label>
            </section>

            <section class="space-y-2">
              <h3 class="font-semibold text-sm uppercase tracking-wide opacity-70">Streamlines</h3>
              <label class="text-sm block">
                "E seeds / charge: "
                <span class="font-mono">{move || app.seeds_per_charge_e.get()}</span>
              </label>
              <input type="range" min="4" max="500" step="1" class="w-full"
                bind:value=app.seeds_per_charge_e
                // prop:value=move || app.seeds_per_charge_e.get().to_string()
                // slider handler
                // on:input=move |ev| {
                //     if let Some(target) = ev.target() {
                //         let input: HtmlInputElement = target.unchecked_into();
                //         if let Ok(v) = input.value().parse() {
                //             app.seeds_per_charge_e.set(v);
                //         }
                //     }
                // }
                />
            </section>

            <section class="space-y-2">
              <h3 class="font-semibold text-sm uppercase tracking-wide opacity-70">Rendering</h3>
              <label class="text-sm block">
                "Point size (px): "
                <span class="font-mono">{move || format!("{:.1}", app.point_size_px.get())}</span>
              </label>
              <input type="range" min="4" max="48" step="0.5" class="w-full"
                prop:value=move || app.point_size_px.get().to_string()
                on:input=move |ev| {
                  if let Some(target) = ev.target() {
                    let input: HtmlInputElement = target.unchecked_into();
                    if let Ok(v) = input.value().parse::<f32>() {
                      app.point_size_px.set(v);
                    }
                  }
                }/>
            </section>

            <section class="space-y-2">
              <h3 class="font-semibold text-sm uppercase tracking-wide opacity-70">
                Playback
              </h3>
              <button
                class="text-sm px-3 py-1 rounded bg-zinc-800 hover:bg-zinc-700 transition-colors"
                on:click=move |_| {
                  app.paused.update(|p| *p = !*p);
                }
              >
                {move || if app.paused.get() { "Play" } else { "Pause" }}
              </button>
              <p class="text-xs opacity-60">
                {move || if app.paused.get() { "Paused" } else { "Running" }}
              </p>
            </section>
          </div>
        </aside>

        <section class="flex-1 h-full relative bg-black overflow-hidden">
          <CanvasWG class="w-full h-full block" app=app />
          <div class="w-[500px] absolute right-2 top-2 px-2 py-1 rounded bg-black/60 text-lime-400 text-[17px] font-mono pointer-events-none whitespace-pre leading-tight">
            {move || app.hud_text.get()}
          </div>
        </section>
      </main>
    }
}

#[component]
fn CanvasWG(class: &'static str, app: AppState) -> impl IntoView {
    let canvas_ref: NodeRef<leptos::html::Canvas> = NodeRef::new();

    canvas_ref.on_load(move |_| {
        // camera + orbit
        picking::attach(canvas_ref, app);
        let canvas = canvas_ref.get().expect("canvas");

        // The WGPU start must be async.. This one unique task does some prep
        // and then kicks off the tail recursive RAF loop thingy..
        wasm_bindgen_futures::spawn_local(create_renderer_and_kick_off_raf_loop(canvas, app));
    });

    view! {
      <div class="absolute inset-0">
        <canvas node_ref=canvas_ref class=class></canvas>
      </div>
    }
}

pub async fn create_renderer_and_kick_off_raf_loop(
    canvas: web_sys::HtmlCanvasElement,
    app: AppState,
) -> () {
    let renderer_sig: RwSignal<Option<WgpuRenderer>, leptos::prelude::LocalStorage> =
        RwSignal::new_local(None);

    // --- canvas sizing (DPR)
    let win = web_sys::window().unwrap();
    let dpr = win.device_pixel_ratio();
    let w = (canvas.client_width() as f64 * dpr).round() as u32;
    let h = (canvas.client_height() as f64 * dpr).round() as u32;
    canvas.set_width(w);
    canvas.set_height(h);

    let (mut cam, orbit_ctl) = {
        let mut cam = camera::Camera::new((w as f32) / (h as f32));
        let ctl = camera::OrbitController::new().attach(&canvas);
        cam.update_from_orbit(&ctl.borrow().orbit());
        (cam, ctl)
    };

    let canvas_for_loop = canvas.clone();

    let mut ren = WgpuRenderer::new(
        canvas,
        &app.charges.get_untracked(),
        app.point_size_px.get_untracked(),
    )
    .await
    .expect("wgpu init");

    ren.update_viewproj(cam.view().to_cols_array(), cam.proj().to_cols_array());
    renderer_sig.set(Some(ren));

    Effect::new(move |_| {
        // read all inputs we care about — this makes the effect derive from them
        // let show_e = app.show_e.get();
        // let n_seeds = app.seeds_per_charge_e.get();
        let px = app.point_size_px.get();
        let charges = app.charges.get(); // positions and q
        // optional: cheaper params while dragging
        // let dragging = app.drag.get().active;

        // always update point size (cheap)
        renderer_sig.update(|opt| {
            if let Some(r) = opt.as_mut() {
                r.set_point_size(px);
            }
        });

        // upload charge centers every time charges change
        renderer_sig.update(|opt| {
            if let Some(r) = opt.as_mut() {
                r.update_charges(&charges);
            }
        });

        // if hidden or empty, clear ribbons and stop
        // if !show_e ||charges.is_empty() {
        //     renderer_sig.update(|opt| {
        //         if let Some(r) = opt.as_mut() {
        //             r.clear_ribbons();
        //         }
        //     });
        // }

        // // build charges4 + seeds inline (simple + explicit)
        // let charges4: Vec<[f32; 4]> = charges
        //     .iter()
        //     .map(|c| [c.pos.x, c.pos.y, c.pos.z, c.q])
        //     .collect();

        // let mut seeds: Vec<[f32; 4]> = Vec::with_capacity(charges.len() * n_seeds);
        // {
        //     let timer_message = &format!("seeds.build n={}", charges.len() * n_seeds);
        //     let _seed_timer = Scope::new(timer_message);
        //     let shell_r = 0.06f32;
        //     for c in &charges {
        //         let sign = if c.q >= 0.0 { 1.0 } else { -1.0 };
        //         for s0 in crate::seed::fibonacci_sphere(c.pos, shell_r, n_seeds) {
        //             seeds.push([s0.x, s0.y, s0.z, sign]);
        //         }
        //     }
        // }
        // // params (wildly cheaper while dragging)
        // // let (h_step, max_pts) = if dragging {
        // //     (0.02f32, 100u32)
        // // } else {
        // //     (0.015f32, 1600u32)
        // // };
        // let (h_step, max_pts) = (0.015f32, 200u32);

        // // submit compute immediately; keep rendering
        // let _render_timer = Scope::new("rendered kickoff");
        // renderer_sig.update(|opt| {
        //     if let Some(r) = opt.as_mut() {
        //         r.start_compute_ribbons_e(&charges4, &seeds, h_step, max_pts);
        //     }
        // });
    });

    // RAF: drive camera + render
    let raf = std::rc::Rc::new(std::cell::RefCell::new(None::<Closure<dyn FnMut(f64)>>));
    let raf2 = raf.clone();

    // State for fps (EWMA to keep it stable)
    use std::cell::Cell;
    thread_local! {
        static LAST_T_MS: Cell<f64> = const { Cell::new(0.0) };
        static EMA_DT_MS: Cell<f64> = const { Cell::new(16.0) }; // start near 60 FPS
    }

    *raf2.borrow_mut() = Some(Closure::wrap(Box::new(move |t_ms: f64| {
        let win = web_sys::window().unwrap();
        // Always schedule the next frame first, so the loop keeps “idling” when paused
        win.request_animation_frame(raf.borrow().as_ref().unwrap().as_ref().unchecked_ref())
            .unwrap();

        // If paused: show HUD "paused", reset timing state, and skip all heavy work
        if app.paused.get_untracked() {
            LAST_T_MS.with(|last| last.set(0.0));
            EMA_DT_MS.with(|ema| ema.set(16.0)); // reset to sane default
            app.hud_text.set("paused".into());
            return;
        }

        // --- FPS/frametime + timing HUD update
        LAST_T_MS.with(|last| {
            let prev = last.get();
            if prev != 0.0 {
                let dt = t_ms - prev; // ms since last frame
                EMA_DT_MS.with(|ema| {
                    // EWMA with ~0.1 smoothing
                    let smoothed = 0.9 * ema.get() + 0.1 * dt;
                    ema.set(smoothed);
                    let fps = if smoothed > 0.0 {
                        1000.0 / smoothed
                    } else {
                        0.0
                    };

                    // Drain perf::Scope + GPU timings collected this frame
                    let timings_str = perf::drain_frame_timings();
                    app.hud_text.set(format!(
                        "{:.1} fps | {:.2} ms\n{}",
                        fps, smoothed, timings_str
                    ));
                });
            }
            last.set(t_ms);
        });

        let dpr = win.device_pixel_ratio();
        let cw = (canvas_for_loop.client_width() as f64 * dpr).round() as u32;
        let ch = (canvas_for_loop.client_height() as f64 * dpr).round() as u32;
        if ch > 0 {
            cam.aspect = (cw as f32) / (ch as f32);
        }
        cam.update_from_orbit(&orbit_ctl.borrow().orbit());

        // compute matrices once
        let view = cam.view();
        let proj = cam.proj();
        let inv_vp = (proj * view).inverse();

        // realtime for picking
        app.inv_vp.set(inv_vp);
        app.eye_rt.set(cam.eye);

        renderer_sig.update_untracked(|opt| {
            let _pre_render = Scope::new("raf pre-render");
            let charges = app.charges.get_untracked(); // positions and q
            let n_seeds = app.seeds_per_charge_e.get_untracked();
            let charges4: Vec<[f32; 4]> = charges
                .iter()
                .map(|c| [c.pos.x, c.pos.y, c.pos.z, c.q])
                .collect();
            let n_seeds_num: usize = n_seeds.parse().expect("Failed to parse integer");
            let mut seeds: Vec<[f32; 4]> = Vec::with_capacity(charges.len() * n_seeds_num);
            {
                let timer_message = &format!("seeds.build n={}", charges.len() * n_seeds_num);
                let _seed_timer = Scope::new(timer_message);
                let shell_r = 0.06f32;
                for c in &charges {
                    let sign = if c.q >= 0.0 { 1.0 } else { -1.0 };
                    for s0 in crate::seed::fibonacci_sphere(c.pos, shell_r, n_seeds_num) {
                        seeds.push([s0.x, s0.y, s0.z, sign]);
                    }
                }
            }
            let (h_step, max_pts) = (0.015f32, 400u32);
            drop(_pre_render);
            if let Some(r) = opt.as_mut() {
                r.resize(cw, ch);
                r.update_viewproj(view.to_cols_array(), proj.to_cols_array());
                r.start_compute_ribbons_e(&charges4, &seeds, h_step, max_pts);
                let _ = r.render();
            }
        });
    }) as Box<dyn FnMut(f64)>));

    web_sys::window()
        .unwrap()
        .request_animation_frame(raf2.borrow().as_ref().unwrap().as_ref().unchecked_ref())
        .unwrap();
}
