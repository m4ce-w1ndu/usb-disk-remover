// USB Disk Remover
// A portable Windows utility to safely eject removable drives.

// #![windows_subsystem = "windows"]  // commented out during development so stdout is visible

mod drives;
mod eject;
mod utils;

fn main() {
    let drives = drives::enumerate_drives();
    println!("{:#?}", drives);
}
