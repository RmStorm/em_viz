use crate::gl::Renderer;
use crate::{camera, em3d::Charge3D};
use glam::Vec3;
use leptos::{logging::*, prelude::*};

// tunables
#[component]
pub fn App() -> impl IntoView {
    view! { <FieldCanvas/> }
}

#[component]
fn FieldCanvas() -> impl IntoView {
    let seeds_per_charge = RwSignal::new(50usize);

    view! {
        <main class="min-h-screen flex">
            <aside class="w-96 p-4 border-r border-zinc-800 bg-zinc-950 text-zinc-100 space-y-4">
                <h2 class="font-semibold text-lg">Controls</h2>
                <p class="text-sm opacity-70">Leptos + WebGL2 + Tailwind</p>

                <div class="space-y-2">
                  <h3 class="font-semibold">Layers</h3>

                  <div class="mt-2">
                    <label class="text-sm block mb-1">
                      Streamline density (seeds / charge): <span class="font-mono">{move || seeds_per_charge.get()}</span>
                    </label>
                    <input
                      type="range" min="10" max="200" step="1"
                      prop:value=move || seeds_per_charge.get()
                      on:input=move |ev| {
                        if let Ok(v) = event_target_value(&ev).parse::<usize>() {
                            seeds_per_charge.set(v);
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
                  seeds_per_charge=seeds_per_charge
                />
            </section>
        </main>
    }
}
#[component]
fn CanvasGL(class: &'static str, seeds_per_charge: RwSignal<usize>) -> impl IntoView {
    let canvas_ref: NodeRef<leptos::html::Canvas> = NodeRef::new();

    let charges3d = vec![
        Charge3D {
            pos: Vec3::new(-0.4, 0.0, 0.0),
            q: 1.0,
        },
        Charge3D {
            pos: Vec3::new(0.4, 0.0, 0.0),
            q: -1.0,
        },
    ];

    // ribbons buffer as reactive state
    let ribbons_sig = RwSignal::new(Vec::<Vec<f32>>::new());

    canvas_ref.on_load(move |_| {
        let canvas = canvas_ref.get().expect("canvas");
        let ren = std::rc::Rc::new(std::cell::RefCell::new(
            Renderer::new(canvas.clone()).expect("gl"),
        ));

        // camera
        let (mut cam, orbit_ctl) = {
            let mut cam = camera::Camera::new(
                (canvas.client_width() as f32) / (canvas.client_height() as f32),
            );
            let ctl = camera::OrbitController::new().attach(&canvas);
            cam.update_from_orbit(&ctl.borrow().orbit());
            (cam, ctl)
        };

        // recompute ribbons when seed slider changes
        {
            use crate::seed::fibonacci_sphere;
            use crate::stream3d::integrate_streamline_ribbon_signed;

            let charges3d = charges3d.clone();
            Effect::new(move |_| {
                let n = seeds_per_charge.get(); // react
                log!("Changing to {:?} seeds_per_charge", seeds_per_charge);
                let soft2 = 0.0025;
                let k = 1.0;
                let stepsize = 0.015;
                let max_pts = 600;
                let shell_r = 0.06;

                let mut ribs = Vec::<Vec<f32>>::new();
                for c in &charges3d {
                    let sign = if c.q >= 0.0 { 1.0 } else { -1.0 };
                    for s in fibonacci_sphere(c.pos, shell_r, n) {
                        ribs.push(integrate_streamline_ribbon_signed(
                            s, &charges3d, k, soft2, stepsize, max_pts, sign,
                        ));
                    }
                }
                ribbons_sig.set(ribs);
            });
        }

        // RAF
        Renderer::start_raf_rc(ren.clone(), move |ren, _t| {
            // camera aspect + update
            let cw = canvas.client_width() as f32;
            let ch = canvas.client_height() as f32;
            if ch > 0.0 {
                cam.aspect = cw / ch;
            }
            cam.update_from_orbit(&orbit_ctl.borrow().orbit());

            // draw
            let mut r = ren.borrow_mut();
            r.clear_color_depth(0.02, 0.02, 0.05, 1.0);

            // upload current ribbons (only when content changed it updates the VBO)
            let ribs = ribbons_sig.get_untracked();
            r.update_ribbons(&ribs);

            let view = cam.view().to_cols_array();
            let proj = cam.proj().to_cols_array();

            let centers = [[-0.4f32, 0.0, 0.0], [0.4f32, 0.0, 0.0]];
            r.update_charge_points(&centers);

            r.draw_ribbons_beautified(&view, &proj);
            r.draw_charges(&view, &proj, 35.0);
        });
    });

    view! {
        <div class="relative">
            <canvas node_ref=canvas_ref class=class></canvas>
        </div>
    }
}
