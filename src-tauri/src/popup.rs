use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindowBuilder,
};

use crate::events::DeviceEvent;
use crate::settings::SettingsState;

/// WebView2 launch flags tuned for a small, short-lived popup. Measured on this PC: private memory
/// of the helper processes drops from about 175 MB (defaults) to about 82 MB. Software rendering
/// is fine for one small card; one renderer and no spare pre-warmed renderer cut the process count.
/// These replace Tauri's default flags, so its three `disable-features` entries are kept.
const WEBVIEW_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,site-per-process,IsolateOrigins,OptimizationHints,Translate,SpareRendererForSitePerProcess --disable-site-isolation-trials --disable-renderer-backgrounding --disable-component-update --renderer-process-limit=1 --in-process-gpu --disable-gpu --disable-software-rasterizer";

const LABEL: &str = "popup";
const WIDTH: f64 = 380.0;
const HEIGHT: f64 = 270.0;
const MARGIN: f64 = 16.0;
const TICK: Duration = Duration::from_millis(250);

#[derive(Default)]
pub struct PopupState {
    current: Mutex<Option<DeviceEvent>>,
    /// Bumped on every show; a close timer only acts if its generation is still current.
    generation: AtomicU64,
    hovered: AtomicBool,
}

/// Show (or update) the popup. The window exists only while visible, so idle RAM stays low.
pub fn show(app: &AppHandle, event: DeviceEvent) -> tauri::Result<()> {
    let state = app.state::<PopupState>();
    *state.current.lock().unwrap() = Some(event.clone());
    let generation = state.generation.fetch_add(1, Ordering::SeqCst) + 1;

    if let Some(window) = app.get_webview_window(LABEL) {
        window.emit("device", &event)?;
    } else {
        let builder = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
            .inner_size(WIDTH, HEIGHT)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .focusable(false)
            .visible(false);
        let window = builder.additional_browser_args(WEBVIEW_ARGS).build()?;

        if let Some(monitor) = app.primary_monitor()? {
            let scale = monitor.scale_factor();
            let area = monitor.work_area();
            let x = area.position.x as f64 + (area.size.width as f64 - WIDTH * scale) / 2.0;
            let y = area.position.y as f64 + area.size.height as f64 - (HEIGHT + MARGIN) * scale;
            window.set_position(PhysicalPosition::new(x as i32, y as i32))?;
        }
        window.show()?;
    }

    schedule_close(app.clone(), generation);
    Ok(())
}

/// Close after SHOW_FOR unless the user is hovering or a newer event replaced this one.
fn schedule_close(app: AppHandle, generation: u64) {
    thread::spawn(move || {
        let secs = app.state::<SettingsState>().0.lock().unwrap().popup_seconds.clamp(2, 30);
        thread::sleep(Duration::from_secs(secs));
        loop {
            let state = app.state::<PopupState>();
            if state.generation.load(Ordering::SeqCst) != generation {
                return;
            }
            if !state.hovered.load(Ordering::SeqCst) {
                if let Some(window) = app.get_webview_window(LABEL) {
                    let _ = window.destroy();
                }
                *state.current.lock().unwrap() = None;
                return;
            }
            thread::sleep(TICK);
        }
    });
}

/// Called by the popup UI once it has loaded.
#[tauri::command]
pub fn popup_ready(state: State<PopupState>) -> Option<DeviceEvent> {
    state.current.lock().unwrap().clone()
}

#[tauri::command]
pub fn popup_hover(state: State<PopupState>, hovered: bool) {
    state.hovered.store(hovered, Ordering::SeqCst);
}

/// The popup's close button.
#[tauri::command]
pub fn popup_close(app: AppHandle, state: State<PopupState>) {
    // A newer generation makes any pending auto-close timer a no-op.
    state.generation.fetch_add(1, Ordering::SeqCst);
    state.hovered.store(false, Ordering::SeqCst);
    *state.current.lock().unwrap() = None;
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.destroy();
    }
}
