use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Bluetooth,
    Wifi,
    Ethernet,
    Usb,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Connected,
    Disconnected,
    Updated,
}

/// One chargeable piece of a device: left/right bud, case, or the single "main" battery.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Part {
    pub key: String,
    pub level: u8,
    pub charging: bool,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Metrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssid: Option<String>,
    /// 0-100
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<String>,
    /// 0-100, how full a storage device is
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub free_space: Option<String>,
}

/// Normalized event every watcher emits; the popup picks a layout from `type`.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct DeviceEvent {
    pub id: String,
    #[serde(rename = "type")]
    pub device_type: DeviceType,
    pub name: String,
    pub status: Status,
    pub parts: Vec<Part>,
    pub metrics: Metrics,
}
