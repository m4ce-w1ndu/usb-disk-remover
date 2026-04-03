use windows::Win32::Foundation::HANDLE;

use crate::drives::RemovableDrive;

/// The types of error that could happen on ejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EjectError {
    LockFailed,
    DismountFailed,
    EjectFailed,
    DeviceNotFound,
}

/// Ejects the device leveraging Windows' removable device API.
pub fn eject_drive(drive: &RemovableDrive) -> Result<(), EjectError> {
    todo!()
}

fn lock_volume(handle: HANDLE) -> Result<(), EjectError> {
    todo!()
}

fn dismount_volume(handle: HANDLE) -> Result<(), EjectError> {
    todo!()
}

fn get_device_node(mount_point: &str) -> Result<u32, EjectError> {
    todo!()
}

fn get_parent_node(devinst: u32) -> Result<u32, EjectError> {
    todo!()
}

fn request_eject(devinst: u32) -> Result<u32, EjectError> {
    todo!()
}
