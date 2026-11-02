use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Instant;

use tauri::AppHandle;

use crate::core::{Action, Core};
use crate::events::DeviceEvent;
use crate::{popup, watchers, TRAY_ID};

/// Sender handle watchers (and the tray's mock menu) use to push events into the core.
pub struct Bus(pub Sender<DeviceEvent>);

/// Wire watchers -> core -> popup/tray. The consumer thread sleeps on the channel when idle.
pub fn start(app: &AppHandle) -> Bus {
    let (tx, rx) = mpsc::channel::<DeviceEvent>();
    watchers::start_all(&tx);

    let app = app.clone();
    thread::spawn(move || {
        let mut core = Core::default();
        for event in rx {
            if let Action::Show(e) = core.handle(event, Instant::now()) {
                if let Err(err) = popup::show(&app, e) {
                    eprintln!("[orbi] popup failed: {err}");
                }
            }
            if let Some(tray) = app.tray_by_id(TRAY_ID) {
                let _ = tray.set_tooltip(Some(core.tooltip()));
            }
        }
    });

    Bus(tx)
}
