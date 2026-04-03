use crate::drives::enumerate_drives;
use crate::{drives::RemovableDrive, utils::str_to_utf16vec};
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_GET_DEVICE_INTERFACE_LIST_PRESENT, CM_Get_Device_Interface_List_SizeW,
    CM_Get_Device_Interface_ListW, CM_Get_Parent, CM_LOCATE_DEVNODE_NORMAL, CM_Locate_DevNodeW,
    CM_Request_Device_EjectW, CONFIGRET,
};
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Ioctl::{IOCTL_STORAGE_GET_DEVICE_NUMBER, STORAGE_DEVICE_NUMBER};
use windows::core::PCWSTR;
use windows::{
    Win32::{
        Foundation::HANDLE,
        System::{
            IO::DeviceIoControl,
            Ioctl::{FSCTL_DISMOUNT_VOLUME, FSCTL_LOCK_VOLUME},
        },
    },
    core::GUID,
};

/// GUID_DEVINTERFACE_DISK value
const GUID_DEVINTERFACE_DISK: GUID = GUID {
    data1: 0x53f56307,
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
    // Get all drives
    let drives = enumerate_drives();

    // Get drive device number
    let target_dev_number =
        query_device_info(&drive.mount_point).ok_or(EjectError::DeviceNotFound)?;

    // Find and collect their device number
    let siblings: Vec<&RemovableDrive> = drives
        .iter()
        .filter_map(|d| {
            let num = query_device_info(&d.mount_point)?;
            (num.DeviceNumber == target_dev_number.DeviceNumber).then_some(d)
        })
        .collect();

    // Lock and dismout all drives
    for drive in siblings {
        let handle = open_volume(&drive.mount_point)?;
        let result = lock_volume(handle).and_then(|_| dismount_volume(handle));
        unsafe {
            _ = CloseHandle(handle);
        };
        result?;
    }

    let devinst = get_device_node(&drive.mount_point)?;
    let parent = get_parent_node(devinst)?;

    request_eject(parent)
}

/// Opens the volume and returns its device HANDLE
fn open_volume(mount_point: &str) -> Result<HANDLE, EjectError> {
    // Get drive letter as single char
    let drive_letter = mount_point.chars().next().unwrap();

    // Create device node path
    let dev_path = format!("\\\\.\\{}:", drive_letter);
    let dev_path = str_to_utf16vec(&dev_path);

    // Open file and get handle
    let handle = unsafe {
        CreateFileW(
            PCWSTR(dev_path.as_ptr()),
            (GENERIC_READ | GENERIC_WRITE).0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    };

    if let Ok(hnd) = handle {
        Ok(hnd)
    } else {
        Err(EjectError::DeviceNotFound)
    }
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
    // 1. Open "\\.\X:" with CreateFileW (read-only, no write access needed here).
    //    Call DeviceIoControl with IOCTL_STORAGE_GET_DEVICE_NUMBER to get a
    //    STORAGE_DEVICE_NUMBER containing DeviceNumber and PartitionNumber.
    //    Close the handle immediately after.
    //
    // 2. Call CM_Get_Device_Interface_List_SizeW with GUID_DEVINTERFACE_VOLUME
    //    and None as the filter to get the required buffer size, then call
    //    CM_Get_Device_Interface_ListW to fill a buffer with all volume interface
    //    paths (double-null-terminated list).
    //
    // 3. For each non-empty entry in the buffer, open it with CreateFileW and
    //    call IOCTL_STORAGE_GET_DEVICE_NUMBER. If DeviceNumber AND PartitionNumber
    //    both match the values from step 1, this is the correct interface path.
    //    Close each handle after querying.
    //
    // 4. Call CM_Locate_DevNodeW with the matching interface path and
    //    CM_LOCATE_DEVNODE_NORMAL to get the DEVINST (u32).
    //
    // Return EjectError::DeviceNotFound on any failure.

    // Query device number
    let device_number = query_device_info(mount_point).ok_or(EjectError::DeviceNotFound)?;

    let mut buffer_len = 0;
    let size_result = unsafe {
        CM_Get_Device_Interface_List_SizeW(
            &mut buffer_len,
            &GUID_DEVINTERFACE_DISK,
            None,
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
            &GUID_DEVINTERFACE_DISK,
            None,
            &mut buffer,
            CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
        )
    };

    match list_result {
        CONFIGRET(0) => {}
        _ => return Err(EjectError::DeviceNotFound),
    }

    // Check each entry and open it
    let mut matched_entry: Option<Vec<u16>> = None;
    for entry in buffer.split(|&c| c == 0).filter(|p| !p.is_empty()) {
        // NULL-terminate the entry
        let mut entry = entry.to_vec();
        entry.push(0u16);

        // Get entry handle
        let entry_handle = unsafe {
            CreateFileW(
                PCWSTR(entry.as_ptr()),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )
        };

        if entry_handle.is_err() {
            continue;
        }

        // Unwrap the handle
        let entry_handle = entry_handle.unwrap();

        // Get device number
        let mut entry_device_number: STORAGE_DEVICE_NUMBER = unsafe { std::mem::zeroed() };
        let mut entry_output_size: u32 = 0;

        let entry_result = unsafe {
            DeviceIoControl(
                entry_handle,
                IOCTL_STORAGE_GET_DEVICE_NUMBER,
                None,
                0,
                Some(&mut entry_device_number as *mut _ as *mut _),
                std::mem::size_of::<STORAGE_DEVICE_NUMBER>() as u32,
                Some(&mut entry_output_size as *mut u32),
                None,
            )
        };

        unsafe { _ = CloseHandle(entry_handle) };

        if entry_result.is_ok() && entry_device_number.DeviceNumber == device_number.DeviceNumber {
            matched_entry = Some(entry);
            break;
        }
    }

    // Find entry matching our volume GUID
    let interface_path = matched_entry.ok_or(EjectError::DeviceNotFound)?;
    let interface_path_slice = &interface_path[..interface_path.len().saturating_sub(1)];
    let interface_path = String::from_utf16_lossy(&interface_path_slice);

    // Get device instance ID
    let device_instid: String = interface_path
        .strip_prefix(r"\\?\")
        .unwrap_or(&interface_path)
        .rsplit_once('#')
        .map(|(prefix, _guid)| prefix)
        .unwrap_or(&interface_path)
        .replace('#', r"\");

    // Back to intermediate representation
    let mut device_instid = str_to_utf16vec(&device_instid);
    device_instid.push(0u16);

    // Locate device node
    let mut dev_inst: u32 = 0;
    let locate_result = unsafe {
        CM_Locate_DevNodeW(
            &mut dev_inst,
            PCWSTR(device_instid.as_ptr()),
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

/// Queries a device number from its mount point.
fn query_device_info(mount_point: &str) -> Option<STORAGE_DEVICE_NUMBER> {
    let handle = open_volume(mount_point).ok()?;
    let mut device_number: STORAGE_DEVICE_NUMBER = unsafe { std::mem::zeroed() };
    let mut output_size: u32 = 0;
    let result = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_GET_DEVICE_NUMBER,
            None,
            0,
            Some(&mut device_number as *mut _ as *mut _),
            std::mem::size_of::<STORAGE_DEVICE_NUMBER>() as u32,
            Some(&mut output_size as *mut u32),
            None,
        )
    };
    unsafe {
        _ = CloseHandle(handle);
    }
    result.ok()?;
    Some(device_number)
}
