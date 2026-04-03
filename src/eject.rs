use crate::{drives::RemovableDrive, utils::str_to_utf16vec};
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_GET_DEVICE_INTERFACE_LIST_PRESENT, CM_Get_Device_Interface_List_SizeW,
    CM_Get_Device_Interface_ListW, CM_Get_Parent, CM_LOCATE_DEVNODE_NORMAL, CM_Locate_DevNodeW,
    CM_Request_Device_EjectW, CONFIGRET,
};
use windows::core::PCWSTR;
use windows::{
    Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::GetVolumeNameForVolumeMountPointW,
        System::{
            IO::DeviceIoControl,
            Ioctl::{FSCTL_DISMOUNT_VOLUME, FSCTL_LOCK_VOLUME},
        },
    },
    core::GUID,
};

/// GUID_DEVINTERFACE_VOLUME value
const GUID_DEVINTERFACE_VOLUME: GUID = GUID {
    data1: 0x53f5630d,
    data2: 0xb6bf,
    data3: 0x11d0,
    data4: [0x94, 0xf2, 0x00, 0xa0, 0xc9, 0x1e, 0xfb, 0x8b],
};

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

/// Returns the device identifier node for the given mount point.
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

    // Canonical path max length of buffer
    const CANONICAL_PATH_LEN: usize = 1024;

    // Volume name buffer
    let mut volume_name = [0u16; CANONICAL_PATH_LEN];
    let mp_utf16vec = str_to_utf16vec(mount_point);

    // Get volume canonical path
    let volume_name_result = unsafe {
        GetVolumeNameForVolumeMountPointW(PCWSTR(mp_utf16vec.as_ptr()), &mut volume_name)
    };

    // Convert to string on success
    if volume_name_result.is_err() {
        return Err(EjectError::DeviceNotFound);
    }

    let null_pos = volume_name.iter().position(|&c| c == 0).unwrap_or(0);
    let volume_name = String::from_utf16_lossy(&volume_name[..null_pos]).to_string();

    // Get volume name as PCWSTR intermediate representation
    let volume_name_ir = str_to_utf16vec(&volume_name);

    let mut buffer_len = 0;

    let size_result = unsafe {
        CM_Get_Device_Interface_List_SizeW(
            &mut buffer_len,
            &GUID_DEVINTERFACE_VOLUME,
            PCWSTR(volume_name_ir.as_ptr()),
            CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
        )
    };

    match size_result {
        CONFIGRET(0) if buffer_len > 1 => {}
        _ => return Err(EjectError::DeviceNotFound),
    }

    // Allocate a buffer to retrieve the list
    let mut buffer = vec![0u16; buffer_len as usize];
    let list_result = unsafe {
        CM_Get_Device_Interface_ListW(
            &GUID_DEVINTERFACE_VOLUME,
            PCWSTR(volume_name_ir.as_ptr()),
            &mut buffer,
            CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
        )
    };

    match list_result {
        CONFIGRET(0) => {}
        _ => return Err(EjectError::DeviceNotFound),
    }

    // Find entry matching our volume GUID
    let interface_path = buffer
        .split(|&c| c == 0)
        .find(|path| !path.is_empty())
        .ok_or(EjectError::DeviceNotFound);

    // Convert to proper intermediate format
    let mut interface_path = interface_path?.to_vec();
    interface_path.push(0u16);

    // Locate device node
    let mut dev_inst: u32 = 0;
    let locate_result = unsafe {
        CM_Locate_DevNodeW(
            &mut dev_inst,
            PCWSTR(interface_path.as_ptr()),
            CM_LOCATE_DEVNODE_NORMAL,
        )
    };

    if locate_result != CONFIGRET(0) {
        return Err(EjectError::DeviceNotFound);
    }

    Ok(dev_inst)
}

/// Returns the parent node of the current device instance.
fn get_parent_node(devinst: u32) -> Result<u32, EjectError> {
    // Parent device instance
    let mut parent_devinst: u32 = 0;
    let result = unsafe { CM_Get_Parent(&mut parent_devinst, devinst, 0) };

    match result {
        r if r != CONFIGRET(0) => Err(EjectError::DeviceNotFound),
        _ => Ok(parent_devinst),
    }
}

/// Requests a device ejection operation to Windows' PnP manager.
fn request_eject(devinst: u32) -> Result<(), EjectError> {
    let result = unsafe { CM_Request_Device_EjectW(devinst, None, None, 0) };
    match result {
        CONFIGRET(0) => Ok(()),
        _ => Err(EjectError::EjectFailed),
    }
}
