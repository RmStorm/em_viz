use std::cell::RefCell;

pub fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or_else(js_sys::Date::now)
}

#[derive(Clone, Debug)]
pub struct TimingEntry {
    pub label: String,
    pub ms: f64,
}

// Per-frame buffer of timing entries.
// In WASM this is effectively just a global, but `thread_local!` is fine.
thread_local! {
    static FRAME_TIMINGS: RefCell<Vec<TimingEntry>> = const { RefCell::new(Vec::new()) };
}

pub fn record_timing(label: impl Into<String>, ms: f64) {
    FRAME_TIMINGS.with(|buf| {
        buf.borrow_mut().push(TimingEntry {
            label: label.into(),
            ms,
        });
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
        record_timing(self.label.clone(), dt);

        // 🔇 No console logging here anymore.
        // If you want optional debug logging:
        // web_sys::console::log_1(&format!("[perf] {}: {:.3} ms", self.label, dt).into());
    }
}

/// Called once per frame from the RAF loop. Returns all timings and clears the buffer.
pub fn drain_frame_timings() -> Vec<TimingEntry> {
    FRAME_TIMINGS.with(|buf| {
        let mut buf = buf.borrow_mut();
        let out = buf.clone();
        buf.clear();
        out
    })
}
