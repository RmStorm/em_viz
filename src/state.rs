use crate::em3d::Charge3D;
use glam::{Mat4, Vec3};
use leptos::prelude::*;

#[derive(Clone, Copy, Default)]
pub struct Drag3D {
    pub active: bool,
    pub idx: usize,
    pub plane_p: Vec3,
    pub plane_n: Vec3,
    pub hit_offset: Vec3,
}

#[derive(Clone, Copy)]
pub struct VizState {
    pub charges: RwSignal<Vec<Charge3D>>,
    pub drag: RwSignal<Drag3D>,

    pub view: RwSignal<Mat4>,
    pub proj: RwSignal<Mat4>,
    pub eye: RwSignal<Vec3>,

    pub ribbons: RwSignal<Vec<Vec<f32>>>,
    pub rebuild: RwSignal<u64>,

    pub seeds_per_charge: RwSignal<usize>,
}

impl VizState {
    pub fn new(initial_charges: Vec<Charge3D>, seeds_default: usize) -> Self {
        Self {
            charges: RwSignal::new(initial_charges),
            drag: RwSignal::new(Drag3D::default()),
            view: RwSignal::new(Mat4::IDENTITY),
            proj: RwSignal::new(Mat4::IDENTITY),
            eye: RwSignal::new(Vec3::ZERO),
            ribbons: RwSignal::new(Vec::new()),
            rebuild: RwSignal::new(0),
            seeds_per_charge: RwSignal::new(seeds_default),
        }
    }

    #[inline]
    pub fn bump_rebuild(&self) {
        self.rebuild.update(|k| *k = k.wrapping_add(1));
    }

    #[inline]
    pub fn viewproj(&self) -> (Mat4, Mat4) {
        (self.view.get_untracked(), self.proj.get_untracked())
    }
}

fn fmt3(v: glam::Vec3) -> String {
    format!("{:+.3}, {:+.3}, {:+.3}", v.x, v.y, v.z)
}
fn fmt1(x: f32) -> String {
    format!("{:+.3}", x)
}

#[component]
pub fn Matrix4Table(m: Signal<Mat4>) -> impl IntoView {
    // Render rows from the current matrix value
    let rows = move || {
        let a = m.get().to_cols_array(); // column-major
        (0..4)
            .map(|r| {
                let row = [a[4 * r], a[1 + 4 * r], a[2 + 4 * r], a[3 + 4 * r]];
                view! {
                  <tr>
                    {row.iter().map(|v| view!{
                       <td class="px-1">{format!("{:+.3}", v)}</td>
                     }).collect::<Vec<_>>()}
                  </tr>
                }
            })
            .collect::<Vec<_>>()
    };

    view! {
      <table class="text-xs font-mono opacity-80">
        <tbody>{rows}</tbody>
      </table>
    }
}

#[component]
pub fn VizDebug(state: crate::state::VizState) -> impl IntoView {
    view! {
      <div class="space-y-3 text-zinc-200">
        <h3 class="font-semibold text-sm uppercase tracking-wide opacity-70">Debug</h3>

        <div class="text-sm">
          <div class="opacity-70">Seeds / charge</div>
          <div class="font-mono">{state.seeds_per_charge}</div>
        </div>

        <div class="text-sm">
          <div class="opacity-70">Eye</div>
          <div class="font-mono">{move || fmt3(state.eye.get())}</div>
        </div>

        <div class="text-sm">
          <div class="opacity-70 mb-1">View</div>
          <Matrix4Table m=state.view.into() />
        </div>

        <div class="text-sm">
          <div class="opacity-70 mb-1">Proj</div>
          <Matrix4Table m=state.proj.into() />
        </div>

        <div class="text-sm">
          <div class="opacity-70">Ribbons</div>
          <div class="font-mono">{move || state.ribbons.get().len()}</div>
        </div>

        <div class="text-sm">
          <div class="opacity-70 mb-1">Charges</div>
          <table class="text-xs font-mono opacity-80">
            <thead><tr><th class="text-left pr-2">i</th><th class="text-left pr-2">pos</th><th class="text-left">q</th></tr></thead>
            <tbody>
              {move || state.charges.get().into_iter().enumerate().map(|(i,c)| view!{
                <tr>
                  <td class="pr-2">{i}</td>
                  <td class="pr-2">{fmt3(c.pos)}</td>
                  <td>{fmt1(c.q)}</td>
                </tr>
              }).collect::<Vec<_>>()}
            </tbody>
          </table>
        </div>

        <div class="text-sm">
          <div class="opacity-70">Drag</div>
          <div class="font-mono">{
            move || {
              let d = state.drag.get();
              format!("active={} idx={} plane_p=({}) plane_n=({})",
                d.active, d.idx, fmt3(d.plane_p), fmt3(d.plane_n))
            }}
          </div>
        </div>
      </div>
    }
}
