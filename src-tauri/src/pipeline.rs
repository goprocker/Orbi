use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use tauri::AppHandle;

use crate::core::{pick_launch, Action, Core};
use tauri::Manager;

use crate::events::{DeviceEvent, Status};
use crate::settings::SettingsState;
use crate::{popup, watchers, TRAY_ID};

/// Events arriving this soon after launch are devices that were already connected. They are
/// folded into one popup so opening Orbi does not flash a card per device.
const LAUNCH_WINDOW: Duration = Duration::from_millis(1500);

/// Sender handle watchers (and the tray's mock menu) use to push events into the core.
pub struct Bus(pub Sender<DeviceEvent>);

/// Wire watchers -> core -> popup/tray. The consumer thread sleeps on the channel when idle.
pub fn start(app: &AppHandle) -> Bus {
    let (tx, rx) = mpsc::channel::<DeviceEvent>();
    watchers::start_all(&tx);

    let app = app.clone();
    thread::spawn(move || {
        let mut core = Core::default();
        let launch_deadline = Instant::now() + LAUNCH_WINDOW;
        let mut launching = true;
        let mut launch_shown: Vec<DeviceEvent> = Vec::new();

        loop {
            let received = if launching {
                rx.recv_timeout(launch_deadline.saturating_duration_since(Instant::now()))
            } else {
                rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
            };
            let event = match received {
                Ok(event) => event,
                Err(RecvTimeoutError::Timeout) => {
                    launching = false;
                    if let Some(e) = pick_launch(std::mem::take(&mut launch_shown)) {
                        show(&app, e);
                    }
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => return,
            };

            let enabled = app.state::<SettingsState>().0.lock().unwrap().enabled(&event.device_type);
            if !enabled && event.status != Status::Disconnected {
                continue;
            }
            if let Action::Show(e) = core.handle(event, Instant::now()) {
                if launching {
                    launch_shown.push(e);
                } else {
                    show(&app, e);
                }
            }
            if let Some(tray) = app.tray_by_id(TRAY_ID) {
                let _ = tray.set_tooltip(Some(core.tooltip()));
            }
        }
    });

    Bus(tx)
}

fn show(app: &AppHandle, event: DeviceEvent) {
    if let Err(err) = popup::show(app, event) {
        eprintln!("[orbi] popup failed: {err}");
    }
}
