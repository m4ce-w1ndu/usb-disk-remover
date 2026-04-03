use tauri::{
    AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

mod drives;
mod eject;
mod utils;

use drives::{enumerate_drives, RemovableDrive};
use eject::eject_drive;

const MAIN_LABEL: &str = "main";

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
fn list_drives() -> Vec<RemovableDrive> {
    enumerate_drives()
}

#[tauri::command]
fn remove_drive(mount_point: String) -> Result<(), String> {
    let drives = enumerate_drives();
    let drive = drives
        .iter()
        .find(|d| {
            d.mount_point.trim_end_matches('\\') == mount_point.trim_end_matches('\\')
        })
        .ok_or_else(|| "Drive not found".to_string())?;

    eject_drive(drive).map_err(|e| format!("{e:?}"))
}

// ── Mica (Windows 11 system backdrop) ────────────────────────────────────────

#[cfg(target_os = "windows")]
fn apply_mica(window: &WebviewWindow) {
    use windows::Win32::{
        Foundation::HWND,
        Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE},
        UI::Controls::SetWindowTheme,
    };
    use windows::core::PCWSTR;

    let Ok(hwnd_raw) = window.hwnd() else { return };
    let hwnd = HWND(hwnd_raw.0 as _);

    unsafe {
        // Dark title bar follows system setting — set it unconditionally;
        // Windows ignores it if the system is in light mode.
        let dark: u32 = 1;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark as *const u32 as *const _,
            std::mem::size_of::<u32>() as u32,
        );

        // DWMSBT_MAINWINDOW = 2 → Mica material
        let backdrop: u32 = 2;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &backdrop as *const u32 as *const _,
            std::mem::size_of::<u32>() as u32,
        );

        // DarkMode_Explorer gives the WebView scrollbars a dark look
        let theme: Vec<u16> = "DarkMode_Explorer\0".encode_utf16().collect();
        let _ = SetWindowTheme(hwnd, PCWSTR(theme.as_ptr()), PCWSTR::null());
    }
}

#[cfg(not(target_os = "windows"))]
fn apply_mica(_window: &WebviewWindow) {}

// ── Window management ─────────────────────────────────────────────────────────

fn create_main_window(app: &AppHandle) {
    let Ok(window) = WebviewWindowBuilder::new(app, MAIN_LABEL, WebviewUrl::App("/".into()))
        .title("USB Disk Remover")
        .inner_size(640.0, 460.0)
        .min_inner_size(420.0, 300.0)
        .center()
        .decorations(true)
        // transparent = true lets WebView2 render over the Mica backdrop
        .transparent(true)
        .build()
    else {
        return;
    };

    // Hide to tray when the X button is clicked — don't destroy the WebView2 process
    let win = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = win.hide();
        }
    });

    apply_mica(&window);
}

fn toggle_window(app: &AppHandle) {
    match app.get_webview_window(MAIN_LABEL) {
        Some(window) => {
            // Hide/show after first creation — instant response, one cold-start only
            if window.is_visible().unwrap_or(false) {
                let _ = window.hide();
            } else {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        None => {
            // First click: cold-start WebView2 once, stays resident after this
            create_main_window(app);
        }
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let about = MenuItem::with_id(app, "about", "About USB Disk Remover", true, None::<&str>)?;
            let sep = tauri::menu::PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&about, &sep, &quit])?;

            let mut builder = TrayIconBuilder::new()
                .tooltip("USB Disk Remover — click to open")
                .menu(&menu)
                .menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_window(tray.app_handle());
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "about" => show_about(app),
                    _ => {}
                });

            if let Some(icon) = app.default_window_icon() {
                builder = builder.icon(icon.clone());
            }

            builder.build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![list_drives, remove_drive])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}

fn show_about(app: &AppHandle) {
    if app.get_webview_window("about").is_some() {
        return;
    }
    let Ok(window) = WebviewWindowBuilder::new(
        app,
        "about",
        WebviewUrl::App("/about".into()),
    )
    .title("About USB Disk Remover")
    .inner_size(360.0, 240.0)
    .resizable(false)
    .center()
    .transparent(true)
    .build() else { return };

    apply_mica(&window);
}
