use windows::Win32::{
    Foundation::HANDLE,
    System::{IO::DeviceIoControl, Ioctl::FSCTL_LOCK_VOLUME},
};

use crate::drives::RemovableDrive;

/// The types of error that could happen on ejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EjectError {
    LockFailed,
    DismountFailed,
    EjectFailed,
    DeviceNotFound,
}

/// Ejects the device leveraging Windows' PnP manager.
pub fn eject_drive(drive: &RemovableDrive) -> Result<(), EjectError> {
    todo!()
}

/// Locks the volume using DeviceIoControl.
fn lock_volume(handle: HANDLE) -> Result<(), EjectError> {
    // Call DeviceIoControl without buffers and FSCTL_LOCK_VOLUME
    let result =
        unsafe { DeviceIoControl(handle, FSCTL_LOCK_VOLUME, None, 0, None, 0, None, None) };

    if result.is_err() {
        Err(EjectError::LockFailed)
    } else {
        Ok(())
    }
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
