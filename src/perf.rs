pub fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or_else(js_sys::Date::now)
}

pub struct Scope<'a> {
    label: &'a str,
    start: f64,
}

impl<'a> Scope<'a> {
    pub fn new(label: &'a str) -> Self {
        // console.time(label) would be nice, but overlapping labels can collide.
        Self {
            label,
            start: now_ms(),
        }
    }
}
impl<'a> Drop for Scope<'a> {
    fn drop(&mut self) {
        let dt = now_ms() - self.start;
        web_sys::console::log_2(
            &format!("[perf] {}: {:.6} ms", self.label, dt).into(),
            &wasm_bindgen::JsValue::NULL,
        );
    }
}
