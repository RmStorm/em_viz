use crate::perf::Scope;
use crate::state::{AppState, Drag3D};
use glam::Vec3;
use leptos::prelude::*;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::JsCast;
use web_sys::PointerEvent;

// --- minimal picking helpers (you can keep yours)
pub fn screen_to_ndc(x: f32, y: f32, rect: &web_sys::DomRect) -> (f32, f32) {
    let nx = ((x as f64 - rect.left()) / rect.width()) as f32;
    let ny = ((y as f64 - rect.top()) / rect.height()) as f32;
    (nx * 2.0 - 1.0, 1.0 - ny * 2.0)
}

pub fn ray_from_ndc_with_inv(ndc: (f32, f32), inv_vp: glam::Mat4, eye: Vec3) -> (Vec3, Vec3) {
    let p_ndc = glam::Vec4::new(ndc.0, ndc.1, 0.0, 1.0);
    let q_ndc = glam::Vec4::new(ndc.0, ndc.1, 1.0, 1.0);
    let p = inv_vp * p_ndc;
    let q = inv_vp * q_ndc;
    let p = (p.truncate() / p.w).extend(1.0).truncate();
    let q = (q.truncate() / q.w).extend(1.0).truncate();
    let ro = eye;
    let rd = (q - p).normalize_or_zero();
    (ro, rd)
}

pub fn ray_sphere(ro: Vec3, rd: Vec3, c: Vec3, r: f32) -> Option<f32> {
    let oc = ro - c;
    let b = oc.dot(rd);
    let c2 = oc.dot(oc) - r * r;
    let disc = b * b - c2;
    if disc < 0.0 {
        return None;
    }
    let t = -b - disc.sqrt();
    if t > 0.0 { Some(t) } else { None }
}

pub fn ray_plane(ro: Vec3, rd: Vec3, p0: Vec3, n: Vec3) -> Option<f32> {
    let denom = rd.dot(n);
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = (p0 - ro).dot(n) / denom;
    if t > 0.0 { Some(t) } else { None }
}

// --- public API
pub fn attach(canvas_ref: NodeRef<leptos::html::Canvas>, app: AppState) {
    // prevent context menu on RMB
    let on_ctx =
        wasm_bindgen::closure::Closure::<dyn FnMut(_)>::new(move |e: web_sys::MouseEvent| {
            e.prevent_default();
        });
    canvas_ref
        .get_untracked()
        .unwrap()
        .add_event_listener_with_callback("contextmenu", on_ctx.as_ref().unchecked_ref())
        .unwrap();
    on_ctx.forget();

    // pointerdown (LMB picks a charge)
    let on_down = wasm_bindgen::closure::Closure::<dyn FnMut(_)>::new(move |e: PointerEvent| {
        if e.button() != 0 {
            return;
        }

        let eye = app.eye_rt.get_untracked();
        let inv_vp = app.inv_vp.get_untracked();

        let rect = canvas_ref
            .get_untracked()
            .expect("canvas")
            .get_bounding_client_rect();
        let ndc = screen_to_ndc(e.client_x() as f32, e.client_y() as f32, &rect);
        let (ro, rd) = ray_from_ndc_with_inv(ndc, inv_vp, eye);

        let cs = app.charges.get_untracked();
        let pick_r = 0.3;
        let mut best: Option<(usize, f32)> = None;
        for (i, c) in cs.iter().enumerate() {
            if let Some(t) = ray_sphere(ro, rd, c.pos, pick_r)
                && t > 0.0
                && best.is_none_or(|(_, b)| t < b)
            {
                best = Some((i, t));
            }
        }
        if let Some((idx, t)) = best {
            // draggable plane: through hit point, facing the camera
            // use camera forward from inv(view); simplest good proxy is ray dir.
            let hit = ro + rd * t;
            let fwd = rd;

            app.drag.set(Drag3D {
                active: true,
                idx,
                plane_p: hit,
                plane_n: fwd,
                hit_offset: Vec3::ZERO,
            });

            let _ = canvas_ref
                .get_untracked()
                .unwrap()
                .set_pointer_capture(e.pointer_id());
            e.prevent_default();
        }
    });
    canvas_ref
        .get_untracked()
        .unwrap()
        .add_event_listener_with_callback("pointerdown", on_down.as_ref().unchecked_ref())
        .unwrap();
    on_down.forget();

    // let rebuild_debounce_move = rebuild_debounce.clone();
    let on_move = wasm_bindgen::closure::Closure::<dyn FnMut(_)>::new(move |e: PointerEvent| {
        let d = app.drag.get_untracked();
        if !d.active {
            return;
        }

        let eye = app.eye_rt.get_untracked();
        let inv_vp = app.inv_vp.get_untracked();

        let rect = canvas_ref
            .get_untracked()
            .expect("canvas")
            .get_bounding_client_rect();
        let ndc = screen_to_ndc(e.client_x() as f32, e.client_y() as f32, &rect);
        let (ro, rd) = ray_from_ndc_with_inv(ndc, inv_vp, eye);

        if let Some(t) = ray_plane(ro, rd, d.plane_p, d.plane_n) {
            let p = ro + rd * t + d.hit_offset;

            // write directly; this triggers the upload_charges effect
            app.charges.update(|cs| {
                if let Some(ch) = cs.get_mut(d.idx) {
                    ch.pos = p;
                }
            });

            // mark that we owe a rebuild when dragging stops
            app.pending_rebuild.set(true);
        }
        e.prevent_default();
    });
    web_sys::window()
        .unwrap()
        .add_event_listener_with_callback("pointermove", on_move.as_ref().unchecked_ref())
        .unwrap();
    on_move.forget();

    // pointerup/cancel
    let on_up = wasm_bindgen::closure::Closure::<dyn FnMut(_)>::new(move |_e: PointerEvent| {
        app.drag.update(|d| d.active = false);

        // Mark that we owe a rebuild; if we're idle, kick it now; if not, the
        // compute effect will see `pending_rebuild` and run one more when done.
        app.pending_rebuild.set(true);
        if !app.computing.get_untracked() {
            app.pending_rebuild.set(false);
            app.bump_rebuild();
        }
    });
    let w = web_sys::window().unwrap();
    w.add_event_listener_with_callback("pointerup", on_up.as_ref().unchecked_ref())
        .unwrap();
    w.add_event_listener_with_callback("pointercancel", on_up.as_ref().unchecked_ref())
        .unwrap();
    on_up.forget();
}
