use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::events::DeviceType;

/// User options, stored as JSON in the app config folder (%APPDATA%\com.orbi.app\settings.json).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub bluetooth: bool,
    pub wifi: bool,
    pub ethernet: bool,
    pub usb: bool,
    /// How long the popup stays up, in seconds.
    pub popup_seconds: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { bluetooth: true, wifi: true, ethernet: true, usb: true, popup_seconds: 5 }
    }
}

impl Settings {
    pub fn enabled(&self, kind: &DeviceType) -> bool {
        match kind {
            DeviceType::Bluetooth => self.bluetooth,
            DeviceType::Wifi => self.wifi,
            DeviceType::Ethernet => self.ethernet,
            DeviceType::Usb => self.usb,
        }
    }

    /// Flip one of the per-type switches by its menu key; returns false for an unknown key.
    pub fn toggle(&mut self, key: &str) -> bool {
        let flag = match key {
            "bluetooth" => &mut self.bluetooth,
            "wifi" => &mut self.wifi,
            "ethernet" => &mut self.ethernet,
            "usb" => &mut self.usb,
            _ => return false,
        };
        *flag = !*flag;
        true
    }

    /// Parse JSON, falling back to defaults for anything missing or broken.
    pub fn from_json(text: &str) -> Settings {
        serde_json::from_str(text).unwrap_or_default()
    }
}

pub struct SettingsState(pub Mutex<Settings>);

fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("settings.json"))
}

pub fn load(app: &AppHandle) -> Settings {
    path(app)
        .and_then(|p| fs::read_to_string(p).ok())
        .map(|t| Settings::from_json(&t))
        .unwrap_or_default()
}

pub fn save(app: &AppHandle, settings: &Settings) {
    let Some(p) = path(app) else { return };
    if let Some(dir) = p.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        if let Err(e) = fs::write(&p, json) {
            eprintln!("[orbi] could not save settings: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_broken_json_gives_defaults() {
        assert_eq!(Settings::from_json(""), Settings::default());
        assert_eq!(Settings::from_json("{not json"), Settings::default());
    }

    #[test]
    fn partial_json_keeps_other_defaults() {
        let s = Settings::from_json(r#"{"wifi": false}"#);
        assert!(!s.wifi && s.bluetooth && s.usb && s.ethernet);
        assert_eq!(s.popup_seconds, 5);
    }

    #[test]
    fn toggle_flips_one_switch() {
        let mut s = Settings::default();
        assert!(s.toggle("usb"));
        assert!(!s.usb && s.wifi);
        assert!(!s.toggle("nonsense"));
        assert!(!s.enabled(&DeviceType::Usb) && s.enabled(&DeviceType::Wifi));
    }
}
