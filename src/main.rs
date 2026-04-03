// USB Disk Remover
// A portable Windows utility to safely eject removable drives.

// #![windows_subsystem = "windows"]  // commented out during development so stdout is visible

use crate::ui::App;
use native_windows_gui as nwg;
use nwg::NativeUi;

mod drives;
mod eject;
mod ui;
mod utils;

fn main() {
    nwg::init().expect("Failed to init ngw");
    nwg::Font::set_global_family("Segoe UI").ok();
    let _app = App::build_ui(Default::default()).expect("Failed to build UI.");
    nwg::dispatch_thread_events();
}
