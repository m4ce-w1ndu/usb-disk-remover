use windows::{
    Win32::{
        Foundation::{CloseHandle, MAX_PATH},
        Storage::FileSystem::{
            BusType1394, BusTypeUsb, CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ,
            FILE_SHARE_WRITE, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
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

use crate::utils::{is_bit_set, str_to_utf16vec};

/// Maximum valid ASCII value
const ASCII_MAX: u8 = 127;

/// Drive type constants from Windows API
const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;

/// The type of bus a removable device is connected through.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum BusType {
    Usb,
    Firewire,
    Unknown,
}

/// A single removable drive visible to the system.
#[derive(Debug, Clone, serde::Serialize)]
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

/// Generic device properties
#[derive(Debug, Clone)]
struct DeviceProperties {
    bus_type: BusType,
    vendor: String,
    product: String,
}

/// Returns all removable drives currently visible to the system.
pub fn enumerate_drives() -> Vec<RemovableDrive> {
    let letters = logical_drive_letters();

    letters
        .into_iter()
        .filter(|p| is_removable(p))
        .filter_map(|p| drive_info(&p))
        .filter(|d| is_removable_bus(&d.bus_type))
        .collect()
}

/// Returns drive root paths for every letter present in the system bitmask.
fn logical_drive_letters() -> Vec<String> {
    const START_LETTER: char = 'A';

    let mut drives = Vec::new();

    let logical_drives = unsafe { GetLogicalDrives() };

    for i in 0..27 {
        let ascii_code = if is_bit_set(logical_drives, i) {
            u32::from(START_LETTER) + u32::from(i)
        } else {
            0
        };

        if ascii_code != 0 && ascii_code <= ASCII_MAX as u32 {
            let letter = (ascii_code as u8) as char;
            let drive_letter = format!("{}:\\", letter);
            drives.push(drive_letter);
        }
    }

    drives
}

/// Returns true if the drive at `root` is removable or fixed (for external HDDs).
fn is_removable(root: &str) -> bool {
    let as_utf16vec = str_to_utf16vec(root);
    let as_pcwstr = PCWSTR(as_utf16vec.as_ptr());

    matches!(
        unsafe { GetDriveTypeW(as_pcwstr) },
        DRIVE_REMOVABLE | DRIVE_FIXED
    )
}

/// Queries volume label and hardware strings for `root`.
fn drive_info(root: &str) -> Option<RemovableDrive> {
    let label = volume_label(root)?;
    let props = query_device_properties(root)?;

    Some(RemovableDrive {
        mount_point: root.to_string(),
        label,
        vendor: props.vendor,
        product: props.product,
        bus_type: props.bus_type,
        is_card_reader: false,
    })
}

/// Returns the volume label for `root`, or None on failure.
fn volume_label(root: &str) -> Option<String> {
    let root_utf16vec = str_to_utf16vec(root);
    let mut output_buffer = [0u16; MAX_PATH as usize];

    let volume_info = unsafe {
        GetVolumeInformationW(
            PCWSTR(root_utf16vec.as_ptr()),
            Some(&mut output_buffer),
            None,
            None,
            None,
            None,
        )
    };

    let null_pos = output_buffer.iter().position(|&c| c == 0).unwrap_or(0);
    let label = String::from_utf16_lossy(&output_buffer[..null_pos]).to_string();

    volume_info.ok().map(|_| label)
}

/// Returns true if the given bus is a removable bus.
fn is_removable_bus(bus: &BusType) -> bool {
    bus == &BusType::Usb || bus == &BusType::Firewire
}

/// Queries a device to obtain its properties.
fn query_device_properties(drive_letter: &str) -> Option<DeviceProperties> {
    const OUT_BUF_LEN: usize = 1024;

    let device_format = format!("\\\\.\\{}:", drive_letter.chars().next().unwrap());
    let device_utf16vec = str_to_utf16vec(&device_format);
    let device_pcwstr = PCWSTR(device_utf16vec.as_ptr());

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

    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceProperty,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0; 1],
    };

    let mut output_buffer = [0u8; OUT_BUF_LEN];
    let mut bytes_returned = 0u32;

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

    let device_descriptor: &STORAGE_DEVICE_DESCRIPTOR =
        unsafe { &*(output_buffer.as_ptr() as *const STORAGE_DEVICE_DESCRIPTOR) };

    let mut vendor_string = String::new();
    let mut product_string = String::new();

    if device_descriptor.VendorIdOffset != 0 {
        let ptr = unsafe { output_buffer.as_ptr().add(device_descriptor.VendorIdOffset as usize) as *const i8 };
        vendor_string = unsafe { std::ffi::CStr::from_ptr(ptr) }.to_string_lossy().trim().to_string();
    }

    if device_descriptor.ProductIdOffset != 0 {
        let ptr = unsafe { output_buffer.as_ptr().add(device_descriptor.ProductIdOffset as usize) as *const i8 };
        product_string = unsafe { std::ffi::CStr::from_ptr(ptr) }.to_string_lossy().trim().to_string();
    }

    let dev_props = DeviceProperties {
        bus_type: match device_descriptor.BusType {
            BusTypeUsb => BusType::Usb,
            BusType1394 => BusType::Firewire,
            _ => BusType::Unknown,
        },
        vendor: vendor_string,
        product: product_string,
    };

    unsafe { let _ = CloseHandle(handle); }

    Some(dev_props)
}
