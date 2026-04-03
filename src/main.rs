// USB Disk Remover
// A portable Windows utility to safely eject removable drives.

// #![windows_subsystem = "windows"]  // commented out during development so stdout is visible

mod drives;
mod eject;
mod utils;

fn main() {
    let drives = drives::enumerate_drives();
    println!("{:#?}", drives);

    if let Some(drive) = drives.first() {
        println!("Attempting to eject {}...", drive.mount_point);
        match eject::eject_drive(drive) {
            Ok(()) => println!("Ejected successfully"),
            Err(e) => println!("Eject failed: {:?}", e),
        }
    }
}
