use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::events::{DeviceEvent, Part, Status};

/// A device that reconnects within this window does not pop up again (connection flapping).
const DEBOUNCE: Duration = Duration::from_secs(2);
/// Battery level at or below which an update is worth interrupting the user for.
const LOW: u8 = 20;

#[derive(Debug, PartialEq)]
pub enum Action {
    Show(DeviceEvent),
    Nothing,
}

/// Tracks live devices and decides which watcher events deserve a popup.
#[derive(Default)]
pub struct Core {
    devices: HashMap<String, DeviceEvent>,
    /// Kept after disconnect so a quick reconnect can be debounced.
    last_shown: HashMap<String, Instant>,
}

impl Core {
    pub fn handle(&mut self, event: DeviceEvent, now: Instant) -> Action {
        match event.status {
            Status::Disconnected => {
                self.devices.remove(&event.id);
                Action::Nothing
            }
            Status::Connected => match self.devices.get(&event.id) {
                Some(known) if *known == event => Action::Nothing,
                Some(_) => self.on_update(event, now),
                None => self.on_connect(event, now),
            },
            Status::Updated => {
                if self.devices.contains_key(&event.id) {
                    self.on_update(event, now)
                } else {
                    self.on_connect(event, now)
                }
            }
        }
    }

    fn on_connect(&mut self, event: DeviceEvent, now: Instant) -> Action {
        let flapping = self
            .last_shown
            .get(&event.id)
            .is_some_and(|t| now.duration_since(*t) < DEBOUNCE);
        self.devices.insert(event.id.clone(), event.clone());
        if flapping {
            return Action::Nothing;
        }
        self.last_shown.insert(event.id.clone(), now);
        Action::Show(event)
    }

    /// Updates are stored silently; only a part newly dropping to low battery pops up.
    fn on_update(&mut self, event: DeviceEvent, now: Instant) -> Action {
        let went_low = self.devices.get(&event.id).is_some_and(|prev| {
            event.parts.iter().any(|p| p.level <= LOW && prev_level(&prev.parts, &p.key).is_some_and(|l| l > LOW))
        });
        self.devices.insert(event.id.clone(), event.clone());
        if went_low {
            self.last_shown.insert(event.id.clone(), now);
            Action::Show(event)
        } else {
            Action::Nothing
        }
    }

    /// One line per live device, for the tray tooltip.
    pub fn tooltip(&self) -> String {
        if self.devices.is_empty() {
            return "Orbi: no devices".into();
        }
        let mut lines: Vec<String> = self.devices.values().map(summary).collect();
        lines.sort();
        lines.join("\n")
    }
}

fn prev_level(parts: &[Part], key: &str) -> Option<u8> {
    parts.iter().find(|p| p.key == key).map(|p| p.level)
}

fn summary(e: &DeviceEvent) -> String {
    let detail = match e.parts.as_slice() {
        [] => "connected".to_string(),
        [one] => format!("{}%", one.level),
        many => many.iter().map(|p| format!("{} {}%", p.key, p.level)).collect::<Vec<_>>().join(" "),
    };
    format!("{}: {}", e.name, detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock;

    fn airpods() -> DeviceEvent {
        mock::event("mock_airpods").unwrap()
    }

    fn with_status(mut e: DeviceEvent, s: Status) -> DeviceEvent {
        e.status = s;
        e
    }

    fn with_level(mut e: DeviceEvent, key: &str, level: u8) -> DeviceEvent {
        e.parts.iter_mut().find(|p| p.key == key).unwrap().level = level;
        e
    }

    #[test]
    fn new_device_shows_popup() {
        let mut core = Core::default();
        let e = airpods();
        assert_eq!(core.handle(e.clone(), Instant::now()), Action::Show(e));
    }

    #[test]
    fn identical_repeat_is_ignored() {
        let mut core = Core::default();
        let t = Instant::now();
        core.handle(airpods(), t);
        assert_eq!(core.handle(airpods(), t + Duration::from_secs(10)), Action::Nothing);
    }

    #[test]
    fn quick_reconnect_is_debounced_but_later_reconnect_shows() {
        let mut core = Core::default();
        let t = Instant::now();
        core.handle(airpods(), t);
        core.handle(with_status(airpods(), Status::Disconnected), t + Duration::from_millis(500));
        assert_eq!(core.handle(airpods(), t + Duration::from_secs(1)), Action::Nothing);

        core.handle(with_status(airpods(), Status::Disconnected), t + Duration::from_secs(2));
        assert!(matches!(core.handle(airpods(), t + Duration::from_secs(10)), Action::Show(_)));
    }

    #[test]
    fn update_crossing_low_threshold_shows() {
        let mut core = Core::default();
        let t = Instant::now();
        core.handle(airpods(), t);
        let low = with_status(with_level(airpods(), "left", 19), Status::Updated);
        assert_eq!(core.handle(low.clone(), t + Duration::from_secs(60)), Action::Show(low));
    }

    #[test]
    fn update_already_low_or_normal_is_silent() {
        let mut core = Core::default();
        let t = Instant::now();
        core.handle(airpods(), t); // case already at 12%
        let normal = with_status(with_level(airpods(), "left", 70), Status::Updated);
        assert_eq!(core.handle(normal, t + Duration::from_secs(60)), Action::Nothing);
        let still_low = with_status(with_level(airpods(), "case", 10), Status::Updated);
        assert_eq!(core.handle(still_low, t + Duration::from_secs(120)), Action::Nothing);
    }

    #[test]
    fn update_for_unknown_device_is_treated_as_connect() {
        let mut core = Core::default();
        let e = with_status(airpods(), Status::Updated);
        assert!(matches!(core.handle(e, Instant::now()), Action::Show(_)));
    }

    #[test]
    fn tooltip_lists_live_devices() {
        let mut core = Core::default();
        assert_eq!(core.tooltip(), "Orbi: no devices");
        let t = Instant::now();
        core.handle(airpods(), t);
        core.handle(mock::event("mock_earbuds").unwrap(), t);
        core.handle(mock::event("mock_ethernet").unwrap(), t);
        assert_eq!(
            core.tooltip(),
            "AirPods Pro: left 82% right 35% case 12%\nEthernet: connected\nOrbi Buds: 64%"
        );
        core.handle(with_status(airpods(), Status::Disconnected), t);
        assert!(!core.tooltip().contains("AirPods"));
    }
}
