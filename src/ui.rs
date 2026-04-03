extern crate native_windows_gui as nwg;
use native_windows_derive::NwgUi;
use std::cell::RefCell;

use crate::drives::{enumerate_drives, RemovableDrive};
use crate::eject::eject_drive;

#[derive(Default, NwgUi)]
pub struct App {
    #[nwg_control(size: (560, 420), center: true, title: "USB Disk Remover")]
    #[nwg_events(
        OnWindowClose: [nwg::stop_thread_dispatch()],
        OnInit: [App::load_drives]
    )]
    window: nwg::Window,

    #[nwg_control(
        list_style: nwg::ListViewStyle::Detailed,
        focus: true,
        ex_flags: nwg::ListViewExFlags::FULL_ROW_SELECT
            | nwg::ListViewExFlags::GRID
    )]
    #[nwg_events(
        OnListViewDoubleClick: [App::on_remove],
        OnListViewItemChanged: [App::on_selection_changed]
    )]
    #[nwg_layout_item(layout: layout, row: 0, col: 0, row_span: 9)]
    drive_list: nwg::ListView,

    #[nwg_control(text: "Safely Remove")]
    #[nwg_events(OnButtonClick: [App::on_remove])]
    #[nwg_layout_item(layout: layout, row: 9, col: 0)]
    remove_button: nwg::Button,

    #[nwg_layout(parent: window, spacing: 4, margin: [8, 8, 8, 8])]
    layout: nwg::GridLayout,

    // Anchors itself to the bottom of the window automatically
    #[nwg_control(parent: window)]
    status_bar: nwg::StatusBar,

    drives: RefCell<Vec<RemovableDrive>>,
}

impl App {
    fn load_drives(&self) {
        // Clear items only, not columns — rebuild columns on first load
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
                width: Some(290),
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
                        "{}  —  {} {}  —  {}",
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
