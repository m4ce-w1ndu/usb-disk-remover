extern crate native_windows_gui as nwg;
use native_windows_derive::NwgUi;
use nwg::stretch::geometry::{Rect, Size};
use nwg::stretch::style::{Dimension as D, FlexDirection};
use std::cell::RefCell;

use crate::drives::{enumerate_drives, RemovableDrive};
use crate::eject::eject_drive;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, DrawTextW,
    DT_END_ELLIPSIS, DT_LEFT, DT_SINGLELINE, DT_VCENTER, EndPaint, FillRect, GetSysColor, HDC,
    HBRUSH, HGDIOBJ, InvalidateRect, PAINTSTRUCT, SetBkMode, SetTextColor, SYS_COLOR_INDEX,
    TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ,
};
use windows::Win32::UI::Controls::{IMAGELIST_CREATION_FLAGS, ImageList_Create, SetWindowTheme};
use windows::Win32::UI::Shell::ExtractIconExW;
use windows::Win32::UI::WindowsAndMessaging::{
    GCLP_HBRBACKGROUND, HICON, SendMessageW, SetClassLongPtrW, WM_SETICON,
};
use windows::core::PCWSTR;

const LVM_SETBKCOLOR: u32 = 0x1001;
const LVM_SETTEXTCOLOR: u32 = 0x1024;
const LVM_SETTEXTBKCOLOR: u32 = 0x1026;
const LVM_GETHEADER: u32 = 0x101F;
const LVM_SETIMAGELIST: u32 = 0x1003;
const LVSIL_SMALL: usize = 1;
const SB_SETBKCOLOR: u32 = 0x2001; // CCM_SETBKCOLOR = CCM_FIRST (0x2000) + 1

const WM_PAINT: u32 = 0x000F;
const WM_SETTINGCHANGE: u32 = 0x001A;
const WM_ERASEBKGND: u32 = 0x0014;
const SB_GETTEXTW: u32 = 0x040D; // WM_USER + 13

// COLOR_WINDOW = 5, COLOR_WINDOWTEXT = 8 (Win32 SYS_COLOR_INDEX constants)
const COLOR_WINDOW: SYS_COLOR_INDEX = SYS_COLOR_INDEX(5);
const COLOR_WINDOWTEXT: SYS_COLOR_INDEX = SYS_COLOR_INDEX(8);

/// Returns true when the user has Windows dark mode enabled for apps.
fn is_dark_mode() -> bool {
    let path: Vec<u16> =
        "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
            .encode_utf16()
            .collect();
    let value: Vec<u16> = "AppsUseLightTheme\0".encode_utf16().collect();

    unsafe {
        let mut hkey = std::mem::zeroed();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(path.as_ptr()),
            None,
            KEY_READ,
            &mut hkey,
        )
        .is_err()
        {
            return false; // can't read registry → assume light
        }

        let mut data: u32 = 1; // default: light mode
        let mut size = std::mem::size_of::<u32>() as u32;
        let _ = RegQueryValueExW(
            hkey,
            PCWSTR(value.as_ptr()),
            None,
            None,
            Some(&mut data as *mut u32 as *mut u8),
            Some(&mut size),
        );
        let _ = RegCloseKey(hkey);

        data == 0 // 0 = dark, 1 = light
    }
}

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
        ex_flags: nwg::ListViewExFlags::FULL_ROW_SELECT | nwg::ListViewExFlags::GRID,
        double_buffer: false
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

    // Keeps the WM_SETTINGCHANGE raw handler alive for the window's lifetime.
    handler: RefCell<Option<nwg::RawEventHandler>>,

    // Keeps the ListView WM_ERASEBKGND handler alive for the window's lifetime.
    // This intercepts UxTheme's background paint so the dark color is always correct.
    list_handler: RefCell<Option<nwg::RawEventHandler>>,

    // Same for the status bar — SB_SETBKCOLOR alone is ignored while UxTheme is active.
    sb_handler: RefCell<Option<nwg::RawEventHandler>>,
}

impl App {
    fn on_init(&self) {
        self.apply_theme();
        self.load_drives();

        // Re-apply the theme whenever the user switches dark/light mode.
        // WM_SETTINGCHANGE (0x001A) with lParam → "ImmersiveColorSet" fires on
        // every Windows color-scheme change.  We re-apply on any settings change;
        // the cost is negligible since it only triggers rarely.
        let self_ptr = self as *const App;
        let raw = nwg::bind_raw_event_handler(
            &self.window.handle,
            0x1_0001, // must be > 0xFFFF (NWG reserves 0x0000–0xFFFF)
            move |_hwnd, msg, _w, _l| {
                if msg == WM_SETTINGCHANGE {
                    // SAFETY: self_ptr is valid for the lifetime of the App struct.
                    // The RawEventHandler is stored in App::handler, so the closure
                    // cannot outlive the App.
                    unsafe { (*self_ptr).apply_theme() };
                }
                None
            },
        );
        *self.handler.borrow_mut() = raw.ok();

        // Intercept WM_ERASEBKGND on the ListView so we own the background paint.
        // Without this, UxTheme (applied via DarkMode_Explorer) paints the background
        // with its own white color, overriding LVM_SETBKCOLOR.
        let list_raw = nwg::bind_raw_event_handler(
            &self.drive_list.handle,
            0x1_0002,
            move |hwnd, msg, wparam, _| {
                if msg == WM_ERASEBKGND {
                    let color = if is_dark_mode() {
                        COLORREF(0x00191919)
                    } else {
                        unsafe { COLORREF(GetSysColor(COLOR_WINDOW)) }
                    };
                    unsafe {
                        let dc = HDC(wparam as *mut _);
                        let hwnd_w = HWND(hwnd as *mut _);
                        let brush = CreateSolidBrush(color);
                        let mut rect: RECT = std::mem::zeroed();
                        let _ = GetClientRect(hwnd_w, &mut rect);
                        FillRect(dc, &rect, brush);
                        let _ = DeleteObject(HGDIOBJ(brush.0));
                    }
                    return Some(1); // background handled
                }
                None
            },
        );
        *self.list_handler.borrow_mut() = list_raw.ok();

        // Status bar: intercept WM_PAINT so we own the full paint cycle.
        // WM_ERASEBKGND alone is not enough — the control repaints its background
        // white in WM_PAINT even when UxTheme is disabled and SB_SETBKCOLOR is set.
        // In light mode we return None so native rendering runs unchanged.
        let sb_raw = nwg::bind_raw_event_handler(
            &self.status_bar.handle,
            0x1_0003,
            move |hwnd, msg, _wparam, _| {
                if msg == WM_ERASEBKGND {
                    // Suppress the default background erase; WM_PAINT fills it.
                    return Some(1);
                }
                if msg == WM_PAINT {
                    if !is_dark_mode() {
                        return None; // native rendering is correct in light mode
                    }
                    unsafe {
                        let hwnd_w = HWND(hwnd as *mut _);
                        let mut ps: PAINTSTRUCT = std::mem::zeroed();
                        let hdc = BeginPaint(hwnd_w, &mut ps);

                        // Dark background
                        let brush = CreateSolidBrush(COLORREF(0x00191919));
                        let mut client: RECT = std::mem::zeroed();
                        let _ = GetClientRect(hwnd_w, &mut client);
                        FillRect(hdc, &client, brush);
                        let _ = DeleteObject(HGDIOBJ(brush.0));

                        // Text from part 0 in white
                        let mut buf = [0u16; 512];
                        let res = SendMessageW(
                            hwnd_w,
                            SB_GETTEXTW,
                            Some(WPARAM(0)),
                            Some(LPARAM(buf.as_mut_ptr() as isize)),
                        );
                        let text_len = (res.0 as u32 & 0xFFFF) as usize;
                        if text_len > 0 && text_len <= 512 {
                            SetTextColor(hdc, COLORREF(0x00FFFFFF));
                            SetBkMode(hdc, TRANSPARENT);
                            let mut tr = RECT {
                                left: client.left + 4,
                                top: client.top,
                                right: client.right - 4,
                                bottom: client.bottom,
                            };
                            DrawTextW(
                                hdc,
                                &mut buf[..text_len],
                                &mut tr,
                                DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
                            );
                        }

                        let _ = EndPaint(hwnd_w, &ps);
                    }
                    return Some(0);
                }
                None
            },
        );
        *self.sb_handler.borrow_mut() = sb_raw.ok();
    }

    fn apply_theme(&self) {
        let dark = is_dark_mode();
        let hwnd = HWND(self.window.handle.hwnd().unwrap() as _);

        // Dark/light title bar via DWM
        let dark_flag: u32 = if dark { 1 } else { 0 };
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark_flag as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }

        let dark_explorer: Vec<u16> = "DarkMode_Explorer\0".encode_utf16().collect();
        let dark_items: Vec<u16> = "DarkMode_ItemsView\0".encode_utf16().collect();
        // Passing an empty string to SetWindowTheme resets the control to its default theme.
        let reset: Vec<u16> = "\0".encode_utf16().collect();
        // " " (a single space) as pszSubAppName disables visual styles on a control.
        // With Common Controls v6 (required by the manifest), visual styles are always
        // active and cause UxTheme to own all ListView painting, ignoring LVM_SETBKCOLOR.
        // The space trick opts the control out of themed rendering so our LVM messages work.
        let no_theme: Vec<u16> = " \0".encode_utf16().collect();

        // Window background brush
        unsafe {
            let bg = if dark {
                COLORREF(0x00191919)
            } else {
                COLORREF(GetSysColor(COLOR_WINDOW))
            };
            let brush: HBRUSH = CreateSolidBrush(bg);
            let brush_val: isize = std::mem::transmute(brush);
            SetClassLongPtrW(hwnd, GCLP_HBRBACKGROUND, brush_val);
        }

        // Window scrollbars / borders
        unsafe {
            let theme = if dark {
                PCWSTR(dark_explorer.as_ptr())
            } else {
                PCWSTR(reset.as_ptr())
            };
            let _ = SetWindowTheme(hwnd, theme, PCWSTR::null());
        }

        // ListView
        if let nwg::ControlHandle::Hwnd(lhwnd) = self.drive_list.handle {
            let lhwnd = HWND(lhwnd as _);
            unsafe {
                // Disable visual styles on the ListView body in dark mode. With Common
                // Controls v6 active, UxTheme ignores LVM_SETBKCOLOR entirely; the space
                // trick opts the control out so the LVM color messages below take effect.
                // In light mode, restore the default theme (null, null).
                if dark {
                    let _ = SetWindowTheme(lhwnd, PCWSTR(no_theme.as_ptr()), PCWSTR::null());
                } else {
                    let _ = SetWindowTheme(lhwnd, PCWSTR::null(), PCWSTR::null());
                }

                let (bg, fg) = if dark {
                    (COLORREF(0x00191919), COLORREF(0x00FFFFFF))
                } else {
                    (
                        COLORREF(GetSysColor(COLOR_WINDOW)),
                        COLORREF(GetSysColor(COLOR_WINDOWTEXT)),
                    )
                };
                // Set both the list background and the per-row text background to the same
                // color. Using CLR_NONE for SETTEXTBKCOLOR makes rows transparent, which
                // exposes any theme-painted background beneath them.
                SendMessageW(lhwnd, LVM_SETBKCOLOR, Some(WPARAM(0)), Some(LPARAM(bg.0 as isize)));
                SendMessageW(lhwnd, LVM_SETTEXTBKCOLOR, Some(WPARAM(0)), Some(LPARAM(bg.0 as isize)));
                SendMessageW(lhwnd, LVM_SETTEXTCOLOR, Some(WPARAM(0)), Some(LPARAM(fg.0 as isize)));

                // Header control
                let hdr = SendMessageW(lhwnd, LVM_GETHEADER, Some(WPARAM(0)), Some(LPARAM(0)));
                if hdr.0 != 0 {
                    let hdr_theme = if dark {
                        PCWSTR(dark_items.as_ptr())
                    } else {
                        PCWSTR(reset.as_ptr())
                    };
                    let _ = SetWindowTheme(HWND(hdr.0 as _), hdr_theme, PCWSTR::null());
                }
            }
        }

        // Button — DarkMode_Explorer gives push buttons the correct dark surface;
        // DarkMode_CFD is for ComboBox/Edit controls and leaves buttons white.
        if let nwg::ControlHandle::Hwnd(bhwnd) = self.remove_button.handle {
            unsafe {
                let theme = if dark {
                    PCWSTR(dark_explorer.as_ptr())
                } else {
                    PCWSTR(reset.as_ptr())
                };
                let _ = SetWindowTheme(HWND(bhwnd as _), theme, PCWSTR::null());
            }
        }

        // Status bar — same pattern as the ListView: UxTheme owns the paint surface
        // and ignores SB_SETBKCOLOR while visual styles are active. The space trick
        // opts the control out of themed rendering so SB_SETBKCOLOR takes effect.
        // CLR_DEFAULT (0xFF000000) restores the system colour in light mode.
        if let nwg::ControlHandle::Hwnd(sbhwnd) = self.status_bar.handle {
            let sbhwnd_w = HWND(sbhwnd as _);
            unsafe {
                if dark {
                    let _ = SetWindowTheme(sbhwnd_w, PCWSTR(no_theme.as_ptr()), PCWSTR::null());
                } else {
                    let _ = SetWindowTheme(sbhwnd_w, PCWSTR::null(), PCWSTR::null());
                }
                let bg = if dark { LPARAM(0x00191919) } else { LPARAM(0xFF000000_u32 as isize) };
                SendMessageW(sbhwnd_w, SB_SETBKCOLOR, Some(WPARAM(0)), Some(bg));
            }
        }

        // Force a full repaint so the new background/colors take effect immediately.
        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, true);
            if let nwg::ControlHandle::Hwnd(lhwnd) = self.drive_list.handle {
                let _ = InvalidateRect(Some(HWND(lhwnd as _)), None, true);
            }
            if let nwg::ControlHandle::Hwnd(sbhwnd) = self.status_bar.handle {
                let _ = InvalidateRect(Some(HWND(sbhwnd as _)), None, true);
            }
        }

        self.set_window_icon();
    }

    fn set_window_icon(&self) {
        let hwnd = HWND(self.window.handle.hwnd().unwrap() as _);

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

            // Set row height via a 1×28 ImageList — the standard Win32 trick for
            // report-view row height (column width is irrelevant, only cy matters).
            if let nwg::ControlHandle::Hwnd(lhwnd) = self.drive_list.handle {
                unsafe {
                    let himl = ImageList_Create(1, 28, IMAGELIST_CREATION_FLAGS(0x20), 0, 0);
                    if !himl.is_invalid() {
                        SendMessageW(
                            HWND(lhwnd as _),
                            LVM_SETIMAGELIST,
                            Some(WPARAM(LVSIL_SMALL)),
                            Some(LPARAM(himl.0)),
                        );
                    }
                }
            }
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
