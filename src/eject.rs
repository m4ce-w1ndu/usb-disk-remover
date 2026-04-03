use windows::Win32::{
    Foundation::HANDLE,
    System::{
        IO::DeviceIoControl,
        Ioctl::{FSCTL_DISMOUNT_VOLUME, FSCTL_LOCK_VOLUME},
    },
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

/// Dismounts the volume using DeviceIoControl.
fn dismount_volume(handle: HANDLE) -> Result<(), EjectError> {
    let mut bytes_returned: u32 = 0;

    // Call DeviceIoControl without buffers and FSCTL_DISMOUNT_VOLUME
    let result = unsafe {
        DeviceIoControl(
            handle,
            FSCTL_DISMOUNT_VOLUME,
            None,
            0,
            None,
            0,
            Some(&mut bytes_returned),
            None,
        )
    };

    if result.is_err() {
        Err(EjectError::DismountFailed)
    } else {
        Ok(())
    }
}

fn get_device_node(mount_point: &str) -> Result<u32, EjectError> {
    // 1. Call GetVolumeNameForVolumeMountPointW with mount_point ("F:\\") to get
    //    the volume GUID path ("\\?\Volume{guid}\").
    //
    // 2. Call CM_Get_Device_Interface_List_Size with GUID_DEVINTERFACE_VOLUME and
    //    the volume GUID path to get the required buffer size, then call
    //    CM_Get_Device_Interface_List with a buffer of that size to get the
    //    double-null-terminated list of device interface paths. Use the first entry.
    //
    // 3. Call CM_Locate_DevNodeW with the device interface path and
    //    CM_LOCATE_DEVNODE_NORMAL to get the DEVINST (u32).
    //
    // Return EjectError::DeviceNotFound on any failure.
    todo!()
}

fn get_parent_node(devinst: u32) -> Result<u32, EjectError> {
    todo!()
}

fn request_eject(devinst: u32) -> Result<(), EjectError> {
    todo!()
}
