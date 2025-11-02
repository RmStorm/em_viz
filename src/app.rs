use crate::gl::Renderer;
use crate::picking;
use crate::state::{VizDebug, VizState};
use crate::{camera, em3d::Charge3D};
use glam::Vec3;
use leptos::{logging::*, prelude::*};

#[component]
pub fn App() -> impl IntoView {
    view! { <FieldCanvas/> }
}

#[component]
fn FieldCanvas() -> impl IntoView {
    let charges3d = vec![
        Charge3D {
            pos: Vec3::new(0.0, 0.0, 0.4),
            q: 1.0,
        },
        Charge3D {
            pos: Vec3::new(0.0, 0.0, -0.4),
            q: -1.0,
        },
    ];
    let state = VizState::new(charges3d, /*seeds_default*/ 30);

    view! {
        <main class="min-h-screen flex">
            <aside class="w-96 p-4 border-r border-zinc-800 bg-zinc-950 text-zinc-100 space-y-4">
                <h2 class="font-semibold text-lg">Controls</h2>
                <p class="text-sm opacity-70">Leptos + WebGL2 + Tailwind</p>

                <div class="space-y-2">
                  <h3 class="font-semibold">Layers</h3>
                  <div class="mt-2">
                    <label class="text-sm block mb-1">
                      Streamline density (seeds / charge): <span class="font-mono">{move || state.seeds_per_charge.get()}</span>
                    </label>
                    <input
                      type="range" min="10" max="200" step="1"
                      prop:value=move || state.seeds_per_charge.get()
                      on:input=move |ev| {
                        if let Ok(v) = event_target_value(&ev).parse::<usize>() {
                            state.seeds_per_charge.set(v);
                        }
                      }
                      style="width:100%;"
                    />
                  </div>
                <VizDebug state=state/>
                </div>

            </aside>

            <section class="flex-1 relative bg-black">
                <CanvasGL
                  class="w-full h-screen block"
                  state=state
                />
            </section>
        </main>
    }
}

#[component]
fn CanvasGL(class: &'static str, state: VizState) -> impl IntoView {
    let canvas_ref: NodeRef<leptos::html::Canvas> = NodeRef::new();

    canvas_ref.on_load(move |_| {
        log!("Canvas is loaded!");
        let ren = std::rc::Rc::new(std::cell::RefCell::new(
            Renderer::new(canvas_ref.get().expect("canvas")).expect("gl"),
        ));
        picking::attach(canvas_ref, state);

        // recompute ribbons when seed slider changes
        Effect::new(move |_| {
            let _ = state.rebuild.get(); // trigger
            let cs = state.charges.get();
            let n = state.seeds_per_charge.get();
            let soft2 = 0.0025;
            let k = 1.0;
            let h = 0.015;
            let max_pts = 1600;
            let shell_r = 0.06;

            let mut ribs = Vec::<Vec<f32>>::new();
            for c in &cs {
                let sign = if c.q >= 0.0 { 1.0 } else { -1.0 };
                for s in crate::seed::fibonacci_sphere(c.pos, shell_r, n) {
                    ribs.push(crate::stream3d::integrate_streamline_ribbon_signed(
                        s, &cs, k, soft2, h, max_pts, sign,
                    ));
                }
            }
            state.ribbons.set(ribs);
        });

        // camera
        let (mut cam, orbit_ctl) = {
            let canvas = canvas_ref.get().expect("canvas");
            let mut cam = camera::Camera::new(
                (canvas.client_width() as f32) / (canvas.client_height() as f32),
            );
            let ctl = camera::OrbitController::new().attach(&canvas);
            cam.update_from_orbit(&ctl.borrow().orbit());
            (cam, ctl)
        };

        // RAF
        let canvas = canvas_ref.get().expect("canvas");
        Renderer::start_raf_rc(ren.clone(), move |ren, _t| {
            // camera aspect + update
            let cw = canvas.client_width() as f32;
            let ch = canvas.client_height() as f32;
            if ch > 0.0 {
                cam.aspect = cw / ch;
            }
            cam.update_from_orbit(&orbit_ctl.borrow().orbit());

            // compute view, proj, eye
            state.view.set(cam.view());
            state.proj.set(cam.proj());
            state
                .eye
                .set(/* compute eye from orbit or view inverse */ cam.eye);

            let mut r = ren.borrow_mut();
            r.clear_color_depth(0.02, 0.02, 0.05, 1.0);

            let ribs = state.ribbons.get_untracked();
            r.update_ribbons(&ribs);

            let centers: Vec<[f32; 3]> = state
                .charges
                .get_untracked()
                .iter()
                .map(|c| [c.pos.x, c.pos.y, c.pos.z])
                .collect();
            r.update_charge_points(&centers);

            let view = state.view.get_untracked().to_cols_array();
            let proj = state.proj.get_untracked().to_cols_array();
            r.draw_ribbons(&view, &proj);
            r.draw_charges(&view, &proj, 14.0);
        });
    });

    view! {
        <div class="relative">
            <canvas node_ref=canvas_ref class=class></canvas>
        </div>
    }
}
