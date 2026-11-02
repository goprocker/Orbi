use crate::events::{DeviceEvent, DeviceType, Metrics, Part, Status};

fn part(key: &str, level: u8, charging: bool) -> Part {
    Part { key: key.into(), level, charging }
}

/// Fake events behind the tray's "Mock" menu, used until real watchers land.
pub fn event(id: &str) -> Option<DeviceEvent> {
    let (device_type, name, parts, metrics) = match id {
        "mock_earbuds" => (
            DeviceType::Bluetooth,
            "Dubstep Buzz X12",
            vec![part("main", 90, false)],
            Metrics::default(),
        ),
        "mock_airpods" => (
            DeviceType::Bluetooth,
            "AirPods Pro",
            vec![part("left", 82, false), part("right", 35, true), part("case", 12, false)],
            Metrics::default(),
        ),
        "mock_dubstep" => (
            DeviceType::Bluetooth,
            "Dubstep Buds",
            vec![part("left", 90, false), part("right", 70, true), part("case", 55, false)],
            Metrics::default(),
        ),
        "mock_wifi" => (
            DeviceType::Wifi,
            "Home Wi-Fi",
            vec![],
            Metrics { ssid: Some("Home Wi-Fi".into()), signal: Some(78), speed: Some("866 Mbps".into()), ..Default::default() },
        ),
        "mock_ethernet" => (
            DeviceType::Ethernet,
            "Ethernet",
            vec![],
            Metrics { speed: Some("1 Gbps".into()), ..Default::default() },
        ),
        "mock_usb" => (
            DeviceType::Usb,
            "SanDisk Ultra",
            vec![],
            Metrics { used_pct: Some(58), free_space: Some("26.8 GB free".into()), ..Default::default() },
        ),
        _ => return None,
    };
    Some(DeviceEvent {
        id: id.into(),
        device_type,
        name: name.into(),
        status: Status::Connected,
        parts,
        metrics,
    })
}
