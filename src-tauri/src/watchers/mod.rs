use std::sync::mpsc::Sender;

use crate::events::DeviceEvent;
use crate::mock;

/// A source of device events. Implementations subscribe to Windows notifications (no polling)
/// and push `DeviceEvent`s into the channel; they must not block the caller of `start`.
pub trait Watcher: Send {
    fn name(&self) -> &'static str;
    fn start(self: Box<Self>, tx: Sender<DeviceEvent>);
}

/// Emits one mock device at launch, selected with `--mock=<id>` (dev aid).
struct MockWatcher {
    id: String,
}

impl Watcher for MockWatcher {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn start(self: Box<Self>, tx: Sender<DeviceEvent>) {
        if let Some(event) = mock::event(&self.id) {
            let _ = tx.send(event);
        }
    }
}

/// Start every enabled watcher. Real watchers (bluetooth, wifi, ...) are added here.
pub fn start_all(tx: &Sender<DeviceEvent>) {
    let mut watchers: Vec<Box<dyn Watcher>> = Vec::new();

    if let Some(id) = std::env::args().find_map(|a| a.strip_prefix("--mock=").map(str::to_owned)) {
        watchers.push(Box::new(MockWatcher { id }));
    }

    for watcher in watchers {
        eprintln!("[orbi] starting watcher: {}", watcher.name());
        watcher.start(tx.clone());
    }
}
