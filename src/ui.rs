extern crate native_windows_gui as nwg;
use native_windows_derive::NwgUi;
use nwg::stretch::geometry::{Rect, Size};
use nwg::stretch::style::{Dimension as D, FlexDirection};
use std::cell::RefCell;

use crate::drives::{enumerate_drives, RemovableDrive};
use crate::eject::eject_drive;

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows::Win32::UI::Controls::SetWindowTheme;
use windows::core::PCWSTR;

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
        margin: Rect { start: D::Points(0.0), end: D::Points(0.0), top: D::Points(0.0), bottom: D::Points(6.0) }
    )]
    drive_list: nwg::ListView,

    #[nwg_control(text: "Safely Remove")]
    #[nwg_events(OnButtonClick: [App::on_remove])]
    #[nwg_layout_item(layout: layout,
        size: Size { width: D::Auto, height: D::Points(32.0) },
        margin: Rect { start: D::Points(0.0), end: D::Points(0.0), top: D::Points(0.0), bottom: D::Points(0.0) }
    )]
    remove_button: nwg::Button,

    #[nwg_layout(
        parent: window,
        flex_direction: FlexDirection::Column,
        padding: Rect {
            start: D::Points(8.0),
            end: D::Points(8.0),
            top: D::Points(8.0),
            bottom: D::Points(36.0)
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

        // Enable dark title bar
        let dark: u32 = 1;
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }

        // Apply dark theme to ListView and Button
        let dark_theme: Vec<u16> = "DarkMode_Explorer\0".encode_utf16().collect();
        let button_theme: Vec<u16> = "DarkMode_CFD\0".encode_utf16().collect();

        if let nwg::ControlHandle::Hwnd(list_hwnd) = self.drive_list.handle {
            unsafe {
                let _ = SetWindowTheme(
                    HWND(list_hwnd as _),
                    PCWSTR(dark_theme.as_ptr()),
                    PCWSTR::null(),
                );
            }
        }

        if let nwg::ControlHandle::Hwnd(btn_hwnd) = self.remove_button.handle {
            unsafe {
                let _ = SetWindowTheme(
                    HWND(btn_hwnd as _),
                    PCWSTR(button_theme.as_ptr()),
                    PCWSTR::null(),
                );
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
