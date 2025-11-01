mod app;
mod gl;

mod seed;
mod camera;
mod em3d;
mod stream3d;

use app::App;
use leptos::{logging, mount};

pub fn main() {
    console_error_panic_hook::set_once();
    logging::log!("csr mode - mounting to body");
    mount::mount_to_body(App);
}
