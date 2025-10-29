use std::cell::RefCell;
use std::rc::Rc;

use crate::gl::Renderer;
use crate::solver::Solver;
use crate::stream::Streamliner;
use leptos::prelude::*;

#[component]
pub fn App() -> impl IntoView {
    view! { <FieldCanvas/> }
}

#[component]
fn FieldCanvas() -> impl IntoView {
    let field_lines = RwSignal::new(true);
    let num_field_lines = RwSignal::new(16.0);
    view! {
        <main class="min-h-screen flex">
            <aside class="w-96 p-4 border-r border-zinc-800 bg-zinc-950 text-zinc-100">
                <h2 class="font-semibold mb-2">sidebar</h2>
                <p class="text-sm opacity-70">Leptos + WebGL2 + Tailwind</p>
                <div class="panel">
                  <h3>Layers</h3>
                  <label >
                    <input type="checkbox" bind:checked=field_lines />
                    Show electric field lines
                  </label>
                  <div >
                    <label >Density</label>
                    <input type="range" min="6" max="36" step="1"
                           style="width:100%;" />
                  </div>
                  <label class="toggle" >
                    <input type="checkbox" />
                    Show tooltip (cell data)
                  </label>
                </div>

            </aside>
            <section class="flex-1 relative bg-black">
                <CanvasGL class="w-full h-screen block" field_lines=field_lines/>
            </section>
        </main>
    }
}

#[component]
fn CanvasGL(class: &'static str, field_lines: RwSignal<bool>) -> impl IntoView {
    let canvas_ref = NodeRef::new();

    canvas_ref.on_load(move |_| {
        let ren = Rc::new(RefCell::new(
            Renderer::new(canvas_ref.get().expect("canvas")).expect("gl"),
        ));
        let mut solver = Solver::new(256, 256);
        solver.add(0.35, 0.5, 1.0);
        solver.add(0.65, 0.5, -1.0);
        let mut stream = Streamliner::new();

        Renderer::start_raf_rc(ren.clone(), move |r, time_sec| {
            solver.step(time_sec);
            r.borrow_mut()
                .update_field_texture(solver.w as i32, solver.h as i32, &solver.field);

            if field_lines.get_untracked() && stream.dirty() {
                let lines = stream.recompute(&solver, 16, 0.8, 600);
                r.borrow_mut().update_lines(&lines);
            }

            let mut rmut = r.borrow_mut();
            rmut.draw();
            if field_lines.get_untracked() {
                rmut.draw_lines()
            };
        });
    });

    view! { <canvas node_ref=canvas_ref class=class></canvas> }
}
