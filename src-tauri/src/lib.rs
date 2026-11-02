mod core;
mod events;
mod mock;
mod pipeline;
mod popup;
mod settings;
mod watchers;

use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager, RunEvent,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

pub const TRAY_ID: &str = "orbi";

const SWITCHES: [(&str, &str); 4] = [
    ("bluetooth", "Earbuds and Bluetooth"),
    ("wifi", "Wi-Fi"),
    ("ethernet", "Ethernet"),
    ("usb", "USB drives"),
];

const MOCKS: [(&str, &str); 6] = [
    ("mock_earbuds", "Mock: earbuds"),
    ("mock_airpods", "Mock: AirPods (L / R / case)"),
    ("mock_dubstep", "Mock: Dubstep Buds"),
    ("mock_wifi", "Mock: Wi-Fi"),
    ("mock_ethernet", "Mock: Ethernet"),
    ("mock_usb", "Mock: USB drive"),
];

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .manage(popup::PopupState::default())
        .invoke_handler(tauri::generate_handler![popup::popup_ready, popup::popup_hover, popup::popup_close])
        .setup(|app| {
            let loaded = settings::load(app.handle());
            app.manage(settings::SettingsState(std::sync::Mutex::new(loaded.clone())));
            let bus = pipeline::start(app.handle());
            let menu = Menu::new(app)?;

            for (key, label) in SWITCHES {
                let on = loaded.enabled(&match key {
                    "bluetooth" => events::DeviceType::Bluetooth,
                    "wifi" => events::DeviceType::Wifi,
                    "ethernet" => events::DeviceType::Ethernet,
                    _ => events::DeviceType::Usb,
                });
                menu.append(&CheckMenuItem::with_id(app, format!("toggle_{key}"), label, true, on, None::<&str>)?)?;
            }
            if !loaded.autostart_initialized {
                // first run: start with Windows so Orbi is always there when something connects
                if let Err(e) = app.autolaunch().enable() {
                    eprintln!("[orbi] could not enable autostart: {e}");
                }
                let state = app.state::<settings::SettingsState>();
                let mut s = state.0.lock().unwrap();
                s.autostart_initialized = true;
                settings::save(app.handle(), &s);
            }
            let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);
            menu.append(&CheckMenuItem::with_id(app, "autostart", "Start with Windows", true, autostart_on, None::<&str>)?)?;
            menu.append(&PredefinedMenuItem::separator(app)?)?;

            for (id, label) in MOCKS {
                menu.append(&MenuItem::with_id(app, id, label, true, None::<&str>)?)?;
            }
            menu.append(&PredefinedMenuItem::separator(app)?)?;
            menu.append(&MenuItem::with_id(app, "quit", "Quit Orbi", true, None::<&str>)?)?;

            TrayIconBuilder::with_id(TRAY_ID)
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Orbi: no devices")
                .menu(&menu)
                .on_menu_event(move |app, event| {
                    let id = event.id.as_ref();
                    if id == "quit" {
                        app.exit(0);
                    } else if id == "autostart" {
                        // the check item already flipped itself, so mirror it into the registry
                        let launcher = app.autolaunch();
                        let result = if launcher.is_enabled().unwrap_or(false) { launcher.disable() } else { launcher.enable() };
                        if let Err(e) = result {
                            eprintln!("[orbi] autostart change failed: {e}");
                        }
                    } else if let Some(key) = id.strip_prefix("toggle_") {
                        let state = app.state::<settings::SettingsState>();
                        let mut s = state.0.lock().unwrap();
                        if s.toggle(key) {
                            settings::save(app, &s);
                        }
                    } else if let Some(ev) = mock::event(id) {
                        let _ = bus.0.send(ev);
                    }
                })
                .build(app)?;

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            // The popup window closes itself; keep the tray process alive until Quit.
            if let RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
