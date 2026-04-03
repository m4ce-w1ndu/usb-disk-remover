# USB Disk Remover

A lightweight, portable Windows utility for safely ejecting removable USB and Firewire drives. Written in Rust.

## Features

- Detects USB and Firewire drives, including external hard drives that Windows reports as fixed disks
- Displays vendor, product name, and volume label for each connected device
- Safely locks and dismounts volumes before ejection, using the Windows PnP manager
- Handles multi-partition devices correctly by ejecting at the physical device level
- Native Win32 GUI with no GPU usage and a minimal memory footprint
- Portable, no installation required

## Requirements

- Windows 10 or later (64-bit)
- No administrator rights required

## Building from Source

### Prerequisites

- [Rust](https://rustup.rs/) (edition 2024)
- A C compiler accessible to the linker (provided by Visual Studio Build Tools or `winget install Microsoft.VisualStudio.2022.BuildTools`)

### Steps

```bash
git clone https://github.com/m4ce-w1ndu/usb-disk-remover
cd usb-disk-remover
cargo build --release
```

The compiled binary will be at `target/release/usb-disk-remover.exe`.

## Usage

Launch `usb-disk-remover.exe`. Any connected removable drives will appear in the list. Select a drive and click Remove, or double-click an entry to eject it immediately.

The application does not require installation and can be run directly from a USB drive.

## How It Works

Drive detection queries the Windows storage stack via `DeviceIoControl` with `IOCTL_STORAGE_QUERY_PROPERTY` to determine the bus type of each logical volume. This allows the application to correctly identify external USB hard drives, which Windows classifies as fixed disks but are still safely ejectable.

Ejection follows a two-phase process. First, each volume belonging to the target device is locked and dismounted through the file system layer. Then the physical device is ejected via `CM_Request_Device_Eject`, the same mechanism used by the Windows "Safely Remove Hardware" dialog. This ensures all partitions on a multi-partition device are torn down cleanly before the device is removed.

## Acknowledgements

This project is a Rust port of [USB Disk Ejector](https://github.com/quickandeasysoftware/USB-Disk-Ejector) by QuickAndEasySoftware, which provided the original design and behavioural specification.

## License

[MIT](LICENSE)
