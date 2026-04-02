use windows::{
    Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW},
    core::PCWSTR,
};

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
    let as_words = root.encode_utf16().chain([0u16]).collect::<Vec<u16>>();
    let as_pcwstr = PCWSTR(as_words.as_ptr());

    if unsafe { GetDriveTypeW(as_pcwstr) == DRIVE_REMOVABLE } {
        return true;
    }

    false
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

/// Returns true if the bit in position `bit` for `value` is set.
fn is_bit_set(value: u32, bit: u8) -> bool {
    (value & (1 << bit)) != 0
}
