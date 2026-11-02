mod core;
mod events;
mod mock;
mod pipeline;
mod popup;
mod watchers;

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    RunEvent,
};

pub const TRAY_ID: &str = "orbi";

const MOCKS: [(&str, &str); 5] = [
    ("mock_earbuds", "Mock: earbuds"),
    ("mock_airpods", "Mock: AirPods (L / R / case)"),
    ("mock_wifi", "Mock: Wi-Fi"),
    ("mock_ethernet", "Mock: Ethernet"),
    ("mock_usb", "Mock: USB drive"),
];

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(popup::PopupState::default())
        .invoke_handler(tauri::generate_handler![popup::popup_ready, popup::popup_hover])
        .setup(|app| {
            let bus = pipeline::start(app.handle());
            let menu = Menu::new(app)?;
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
