extern crate native_windows_gui as nwg;
use native_windows_derive::NwgUi;
use nwg::stretch::geometry::{Rect, Size};
use nwg::stretch::style::{Dimension as D, FlexDirection};
use std::cell::RefCell;

use crate::drives::{enumerate_drives, RemovableDrive};
use crate::eject::eject_drive;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows::Win32::Graphics::Gdi::{CreateSolidBrush, HBRUSH};
use windows::Win32::UI::Controls::SetWindowTheme;
use windows::Win32::UI::Shell::ExtractIconExW;
use windows::Win32::UI::WindowsAndMessaging::{
    GCLP_HBRBACKGROUND, HICON, SendMessageW, SetClassLongPtrW, WM_SETICON,
};
use windows::core::PCWSTR;

// LVM_FIRST + 31: retrieves the header control handle from a ListView
const LVM_GETHEADER: u32 = 0x101F;

#[derive(Default, NwgUi)]
pub struct App {
    #[nwg_control(size: (600, 440), center: true, title: "USB Disk Remover")]
    #[nwg_events(
        OnWindowClose: [nwg::stop_thread_dispatch()],
        OnInit: [App::on_init]
    )]
    window: nwg::Window,

    #[nwg_control(
        list_style: nwg::ListViewStyle::Detailed,
        focus: true,
        ex_flags: nwg::ListViewExFlags::FULL_ROW_SELECT | nwg::ListViewExFlags::GRID
    )]
    #[nwg_events(
        OnListViewDoubleClick: [App::on_remove],
        OnListViewItemChanged: [App::on_selection_changed]
    )]
    #[nwg_layout_item(layout: layout,
        flex_grow: 1.0,
        size: Size { width: D::Auto, height: D::Auto },
        margin: Rect { start: D::Points(0.0), end: D::Points(0.0), top: D::Points(0.0), bottom: D::Points(8.0) }
    )]
    drive_list: nwg::ListView,

    #[nwg_control(text: "\u{23CF}  Safely Remove")]
    #[nwg_events(OnButtonClick: [App::on_remove])]
    #[nwg_layout_item(layout: layout,
        size: Size { width: D::Auto, height: D::Points(36.0) },
        margin: Rect { start: D::Points(0.0), end: D::Points(0.0), top: D::Points(0.0), bottom: D::Points(0.0) }
    )]
    remove_button: nwg::Button,

    #[nwg_layout(
        parent: window,
        flex_direction: FlexDirection::Column,
        padding: Rect {
            start: D::Points(10.0),
            end: D::Points(10.0),
            top: D::Points(10.0),
            bottom: D::Points(38.0)
        }
    )]
    layout: nwg::FlexboxLayout,

    #[nwg_control(parent: window)]
    status_bar: nwg::StatusBar,

    drives: RefCell<Vec<RemovableDrive>>,
}

impl App {
    fn on_init(&self) {
        self.apply_dark_mode();
        self.load_drives();
    }

    fn apply_dark_mode(&self) {
        let hwnd = HWND(self.window.handle.hwnd().unwrap() as _);

        // Enable dark title bar via DWM
        let dark: u32 = 1;
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }

        let dark_explorer: Vec<u16> = "DarkMode_Explorer\0".encode_utf16().collect();
        let dark_cfd: Vec<u16> = "DarkMode_CFD\0".encode_utf16().collect();
        let dark_items: Vec<u16> = "DarkMode_ItemsView\0".encode_utf16().collect();

        // Paint the window client area dark (#191919).
        // SetClassLongPtrW changes the background brush for the window's class.
        // Since NWG uses a unique class per window type and this app has only one
        // main window, this is safe and avoids subclassing.
        unsafe {
            let brush: HBRUSH = CreateSolidBrush(COLORREF(0x00191919));
            let brush_val: isize = std::mem::transmute(brush);
            SetClassLongPtrW(hwnd, GCLP_HBRBACKGROUND, brush_val);
        }

        // Apply dark theme to the window itself (scrollbars, borders)
        unsafe {
            let _ = SetWindowTheme(hwnd, PCWSTR(dark_explorer.as_ptr()), PCWSTR::null());
        }

        // Dark theme for ListView and its header control
        if let nwg::ControlHandle::Hwnd(list_hwnd) = self.drive_list.handle {
            let list_hwnd = HWND(list_hwnd as _);
            unsafe {
                let _ = SetWindowTheme(list_hwnd, PCWSTR(dark_explorer.as_ptr()), PCWSTR::null());

                // The ListView header is a separate child control — darken it too
                let result = SendMessageW(list_hwnd, LVM_GETHEADER, Some(WPARAM(0)), Some(LPARAM(0)));
                if result.0 != 0 {
                    let _ = SetWindowTheme(
                        HWND(result.0 as _),
                        PCWSTR(dark_items.as_ptr()),
                        PCWSTR::null(),
                    );
                }
            }
        }

        // Dark theme for button
        if let nwg::ControlHandle::Hwnd(btn_hwnd) = self.remove_button.handle {
            unsafe {
                let _ = SetWindowTheme(
                    HWND(btn_hwnd as _),
                    PCWSTR(dark_cfd.as_ptr()),
                    PCWSTR::null(),
                );
            }
        }

        // Dark theme for status bar
        if let nwg::ControlHandle::Hwnd(sb_hwnd) = self.status_bar.handle {
            unsafe {
                let _ = SetWindowTheme(
                    HWND(sb_hwnd as _),
                    PCWSTR(dark_explorer.as_ptr()),
                    PCWSTR::null(),
                );
            }
        }

        self.set_window_icon();
    }

    fn set_window_icon(&self) {
        let hwnd = HWND(self.window.handle.hwnd().unwrap() as _);

        // Load the removable-drive icon from shell32.dll (index 7: removable storage).
        // If the extraction fails the window simply keeps the default application icon.
        let shell32: Vec<u16> = "shell32.dll\0".encode_utf16().collect();
        unsafe {
            let mut large: HICON = std::mem::zeroed();
            let mut small: HICON = std::mem::zeroed();

            ExtractIconExW(
                PCWSTR(shell32.as_ptr()),
                7,
                Some(&mut large as *mut HICON),
                Some(&mut small as *mut HICON),
                1,
            );

            // Use transmute to convert HICON to isize safely regardless of
            // whether the inner type is isize or *mut c_void.
            let large_val: isize = std::mem::transmute(large);
            let small_val: isize = std::mem::transmute(small);

            if large_val != 0 {
                let _ = SendMessageW(hwnd, WM_SETICON, Some(WPARAM(1)), Some(LPARAM(large_val)));
            }
            if small_val != 0 {
                let _ = SendMessageW(hwnd, WM_SETICON, Some(WPARAM(0)), Some(LPARAM(small_val)));
            }
        }
    }

    fn load_drives(&self) {
        if self.drive_list.column_len() == 0 {
            self.drive_list.insert_column(nwg::InsertListViewColumn {
                index: Some(0),
                fmt: None,
                width: Some(65),
                text: Some("Drive".to_string()),
            });
            self.drive_list.insert_column(nwg::InsertListViewColumn {
                index: Some(1),
                fmt: None,
                width: Some(150),
                text: Some("Label".to_string()),
            });
            self.drive_list.insert_column(nwg::InsertListViewColumn {
                index: Some(2),
                fmt: None,
                width: Some(340),
                text: Some("Device".to_string()),
            });
        }

        self.drive_list.clear();

        let drives = enumerate_drives();

        if drives.is_empty() {
            self.status_bar.set_text(0, "No removable drives found.");
        } else {
            self.status_bar.set_text(
                0,
                &format!("{} removable drive(s) detected.", drives.len()),
            );
        }

        for (i, drive) in drives.iter().enumerate() {
            let device = format!("{} {}", drive.vendor, drive.product);
            let i = i as i32;

            self.drive_list.insert_item(nwg::InsertListViewItem {
                index: Some(i),
                column_index: 0,
                text: Some(drive.mount_point.clone()),
                image: None,
            });
            self.drive_list.insert_item(nwg::InsertListViewItem {
                index: Some(i),
                column_index: 1,
                text: Some(drive.label.clone()),
                image: None,
            });
            self.drive_list.insert_item(nwg::InsertListViewItem {
                index: Some(i),
                column_index: 2,
                text: Some(device),
                image: None,
            });
        }

        *self.drives.borrow_mut() = drives;
    }

    fn on_selection_changed(&self) {
        let drives = self.drives.borrow();
        match self.drive_list.selected_item().and_then(|i| drives.get(i)) {
            Some(drive) => {
                let bus = format!("{:?}", drive.bus_type);
                self.status_bar.set_text(
                    0,
                    &format!(
                        "{}  \u{2014}  {} {}  \u{2014}  {}",
                        drive.mount_point, drive.vendor, drive.product, bus
                    ),
                );
            }
            None => {
                self.status_bar.set_text(
                    0,
                    &format!("{} removable drive(s) detected.", drives.len()),
                );
            }
        }
    }

    fn on_remove(&self) {
        let drive: Option<RemovableDrive> = self
            .drive_list
            .selected_item()
            .and_then(|idx| self.drives.borrow().get(idx).cloned());

        match drive {
            None => {
                nwg::simple_message("No selection", "Please select a drive to remove.");
            }
            Some(drive) => match eject_drive(&drive) {
                Ok(()) => {
                    nwg::simple_message(
                        "Ejected",
                        &format!("{} was safely removed.", drive.mount_point),
                    );
                    self.load_drives();
                }
                Err(e) => {
                    nwg::simple_message(
                        "Error",
                        &format!("Could not eject {}: {:?}", drive.mount_point, e),
                    );
                }
            },
        }
    }
}
