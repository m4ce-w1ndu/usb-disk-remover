use windows::{
    Win32::{
        Foundation::CloseHandle,
        Storage::FileSystem::{
            BusType1394, BusTypeUsb, CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ,
            FILE_SHARE_WRITE, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW, OPEN_ALWAYS,
            OPEN_EXISTING,
        },
        System::{
            IO::DeviceIoControl,
            Ioctl::{
                IOCTL_STORAGE_QUERY_PROPERTY, PropertyStandardQuery, STORAGE_DEVICE_DESCRIPTOR,
                STORAGE_PROPERTY_QUERY, StorageDeviceProperty,
            },
        },
    },
    core::PCWSTR,
};

use std::ffi::c_void;

use crate::utils::{is_bit_set, str_to_utf16vec};

/// Maximum valid ASCII value
const ASCII_MAX: u8 = 127;

/// Drive type constants from Windows API
const DRIVE_UNKNOWN: u32 = 0;
const DRIVE_NO_ROOT_DIR: u32 = 1;
const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;
const DRIVE_REMOTE: u32 = 4;
const DRIVE_CDROM: u32 = 5;
const DRIVE_RAMDISK: u32 = 6;

/// The type of bus a removable device is connected through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusType {
    Usb,
    Firewire,
    Unknown,
}

/// A single removable drive visible to the system.
#[derive(Debug, Clone)]
pub struct RemovableDrive {
    /// Drive letter + backslash, e.g. "E:\"
    pub mount_point: String,
    /// User-visible volume label, e.g. "My USB Drive"
    pub label: String,
    /// Hardware vendor string, e.g. "SanDisk"
    pub vendor: String,
    /// Hardware product string, e.g. "Ultra"
    pub product: String,
    /// Connection bus type
    pub bus_type: BusType,
    /// Whether this device is a card reader (set by user config or detection)
    pub is_card_reader: bool,
}

/// Returns all removable drives currently visible to the system.
pub fn enumerate_drives() -> Vec<RemovableDrive> {
    let letters = logical_drive_letters();

    letters
        .into_iter()
        .filter(|path| is_removable(path))
        .filter_map(|path| drive_info(&path))
        .collect()
}

/// Returns drive root paths for every letter present in the system bitmask.
/// e.g. ["C:\\", "E:\\", "F:\\"]
fn logical_drive_letters() -> Vec<String> {
    const START_LETTER: char = 'A';

    let mut drives = Vec::new();

    // Call GetLogicalDrives() and iterate the 26-bit bitmask.
    // Bit 0 = A, bit 1 = B, ..., bit 25 = Z.
    // Push "X:\\" for each set bit.
    let logical_drives = unsafe { GetLogicalDrives() };

    for i in 0..27 {
        let ascii_code = if is_bit_set(logical_drives, i) {
            u32::from(START_LETTER) + u32::from(i)
        } else {
            0
        };

        // If the code is a valid letter
        if ascii_code != 0 && ascii_code <= ASCII_MAX as u32 {
            let letter = (ascii_code as u8) as char;
            let drive_letter = format!("{0}:\\", letter);
            drives.push(drive_letter);
        }
    }

    drives
}

/// Returns true if the drive at `root` is removable.
fn is_removable(root: &str) -> bool {
    // Encode `root` as a null-terminated wide string (PCWSTR),
    // call GetDriveTypeW(), and return true when the result == DRIVE_REMOVABLE.
    let as_utf16vec = str_to_utf16vec(root);
    let as_pcwstr = PCWSTR(as_utf16vec.as_ptr());

    let removable_flag = unsafe { GetDriveTypeW(as_pcwstr) == DRIVE_REMOVABLE };

    // Get drive bus
    let drive_bus = bus_type_from_drive(&root.to_owned());
    let removable_bus = drive_bus.is_some() && is_removable_bus(drive_bus.unwrap());

    // The drive is removable if it has the removable flag or its bus
    // is flagged as a removable bus
    removable_flag || removable_bus
}

/// Queries volume label and (later) hardware strings for `root`.
/// Returns None if the drive disappeared between enumeration and query.
fn drive_info(root: &str) -> Option<RemovableDrive> {
    let label = volume_label(root)?;

    // TODO: query vendor/product via DeviceIoControl (next step)
    Some(RemovableDrive {
        mount_point: root.to_string(),
        label,
        vendor: String::new(),
        product: String::new(),
        bus_type: BusType::Unknown,
        is_card_reader: false,
    })
}

/// Returns the volume label for `root`, or None on failure.
fn volume_label(root: &str) -> Option<String> {
    // TODO: encode `root` as PCWSTR, allocate a [u16; MAX_PATH] buffer,
    // call GetVolumeInformationW(), then convert the buffer to a String
    // with String::from_utf16_lossy().
    None
}

/// Returns true if the given bus is a removable bus.
fn is_removable_bus(bus: BusType) -> bool {
    bus == BusType::Usb || bus == BusType::Firewire
}

/// Returns the bus type of the drive related with the given letter
fn bus_type_from_drive(drive_letter: &String) -> Option<BusType> {
    const OUT_BUF_LEN: usize = 1024;

    // TODO: open a handle to the drive using CreateFileW()
    // using DOS logical path (\\.\X:)
    // Then call DeviceIoControl with IOCTL_STORAGE_QUERY_PROPERTY
    // with a STORAGE_PROPERTY_QUERY struct with PropertyId = StorageDeviceProperty
    // and QueryType = PropertyStandardQuery

    let device_format = format!("\\\\.\\{}:", drive_letter.chars().next().unwrap());
    let device_utf16vec = str_to_utf16vec(&device_format);
    let device_pcwstr = PCWSTR(device_utf16vec.as_ptr());

    // Get the device handle
    let handle = unsafe {
        CreateFileW(
            device_pcwstr,
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
        .ok()?
    };

    // Query properties
    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceProperty,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0; 1],
    };

    let mut output_buffer = [0u8; OUT_BUF_LEN];
    let mut bytes_returned = 0u32;

    // Perform device query
    let _ = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some(&query as *const _ as *const _),
            std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            Some(output_buffer.as_mut_ptr() as *mut _),
            output_buffer.len() as u32,
            Some(&mut bytes_returned),
            None,
        )
        .ok()?
    };

    // Convert output buffer to storage descriptor
    let device_descriptor: &STORAGE_DEVICE_DESCRIPTOR =
        unsafe { &*(output_buffer.as_ptr() as *const STORAGE_DEVICE_DESCRIPTOR) };

    // Match and map the device type
    let bus_type = match device_descriptor.BusType {
        BusTypeUsb => BusType::Usb,
        BusType1394 => BusType::Firewire,
        _ => BusType::Unknown,
    };

    // Close file handle
    let _ = unsafe { CloseHandle(handle) };

    Some(bus_type)
}
