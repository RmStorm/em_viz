use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write as _;

use leptos::logging::log;

pub fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or_else(js_sys::Date::now)
}

#[derive(Clone, Debug)]
pub struct TimingEntry {
    pub ms: f64,
    pub prev_ms: f64,
}

thread_local! {
    static FRAME_TIMINGS: RefCell<HashMap<String, TimingEntry>> =
        RefCell::new(HashMap::new());
}

/// Shared helper: CPU + GPU both call this.
pub fn record_timing(label: impl Into<String>, ms: f64) {
    let label = label.into();
    log!("hmm {}", label);
    FRAME_TIMINGS.with(|map| {
        let mut map = map.borrow_mut();

        if let Some(prev) = map.get_mut(&label) {
            prev.prev_ms = prev.ms;
            prev.ms = ms;
        } else {
            map.insert(
                label,
                TimingEntry {
                    prev_ms: f64::NAN,
                    ms,
                },
            );
        };
    });
}

/// Simple scope timer: on drop, push timing into FRAME_TIMINGS
pub struct Scope {
    label: String,
    start: f64,
}

impl Scope {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            start: now_ms(),
        }
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        let dt = now_ms() - self.start;
        record_timing(&self.label, dt);

        // 🔇 No console logging here anymore.
        // If you want optional debug logging:
        // web_sys::console::log_1(&format!("[perf] {}: {:.3} ms", self.label, dt).into());
    }
}

/// Called once per frame from the RAF loop.
pub fn drain_frame_timings() -> String {
    FRAME_TIMINGS.with(|state| {
        let mut map = state.borrow_mut();

        if map.is_empty() {
            "".to_string()
        } else {
            let mut s = String::new();
            let mut labels: Vec<String> = map.keys().cloned().collect();
            labels.sort();
            for label in labels {
                let entry = map.get_mut(&label).unwrap();
                if entry.ms.is_nan() {
                    let _ = write!(s, "{}: NAN (prev: {:.2}ms)", label, entry.prev_ms);
                } else {
                    let _ = write!(s, "{}: {:.2}ms", label, entry.ms);
                }
                s.push('\n');
                entry.prev_ms = entry.ms;
                entry.ms = f64::NAN;
            }
            s
        }
    })
}
