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
pub struct AppState {
    pub charges: RwSignal<Vec<Charge3D>>,
    pub drag: RwSignal<Drag3D>,

    // realtime camera bits used by picking/render; updated EVERY FRAME
    pub eye_rt: RwSignal<Vec3>,
    pub inv_vp: RwSignal<Mat4>, // cached inverse(proj*view)

    // controls
    pub rebuild: RwSignal<u64>,
    pub seeds_per_charge_e: RwSignal<usize>,
    pub show_e: RwSignal<bool>,
    pub point_size_px: RwSignal<f32>,

    // task flags
    pub computing: RwSignal<bool>,
    pub pending_rebuild: RwSignal<bool>,

    // pause / play RAF-driven simulation & rendering
    pub paused: RwSignal<bool>,
}

impl AppState {
    pub fn new(
        initial_charges: Vec<Charge3D>,
        seeds_default: usize,
        point_size_default: f32,
    ) -> Self {
        Self {
            charges: RwSignal::new(initial_charges),
            drag: RwSignal::new(Drag3D::default()),
            eye_rt: RwSignal::new(Vec3::ZERO),
            inv_vp: RwSignal::new(Mat4::IDENTITY),

            rebuild: RwSignal::new(0),
            seeds_per_charge_e: RwSignal::new(seeds_default),
            show_e: RwSignal::new(true),
            point_size_px: RwSignal::new(point_size_default),

            computing: RwSignal::new(false),
            pending_rebuild: RwSignal::new(false),

            paused: RwSignal::new(false),
        }
    }

    #[inline]
    pub fn bump_rebuild(&self) {
        self.rebuild.update(|k| *k = k.wrapping_add(1));
    }
}
