#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    usb_disk_remover_lib::run();
}
