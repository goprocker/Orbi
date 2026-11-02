//! AirPods left / right / case battery from Apple's BLE "proximity pairing" advertisements.
//!
//! AirPods broadcast manufacturer data (company id 0x004C) while the case lid is open or the
//! buds are in use. The layout below follows community reverse-engineering of that undocumented
//! format (OpenPods and similar projects).
//!
//! NOT VERIFIED ON HARDWARE: the parser is unit-tested against hand-built payloads only. Left
//! and right can be swapped if the "flip" bit is read wrongly; confirm with real AirPods.

use std::sync::mpsc::Sender;
use std::sync::Mutex;

use windows::Devices::Bluetooth::Advertisement::{
    BluetoothLEAdvertisementFilter, BluetoothLEAdvertisementReceivedEventArgs, BluetoothLEAdvertisementWatcher,
    BluetoothLEManufacturerData, BluetoothLEScanningMode,
};
use windows::Foundation::TypedEventHandler;
use windows::Storage::Streams::DataReader;

use super::Watcher;
use crate::events::{DeviceEvent, DeviceType, Metrics, Part, Status};

const APPLE: u16 = 0x004C;
/// Ignore AirPods that are not right next to the PC (nearby phones and strangers).
const MIN_RSSI: i16 = -60;

pub struct AirPodsWatcher;

#[derive(Debug, PartialEq, Clone)]
pub struct AirPods {
    pub model: &'static str,
    pub left: Option<u8>,
    pub right: Option<u8>,
    pub case: Option<u8>,
    pub left_charging: bool,
    pub right_charging: bool,
    pub case_charging: bool,
}

impl Watcher for AirPodsWatcher {
    fn name(&self) -> &'static str {
        "airpods"
    }

    fn start(self: Box<Self>, tx: Sender<DeviceEvent>) {
        if let Err(e) = scan(tx) {
            eprintln!("[orbi] airpods: BLE scan failed: {e}");
        }
    }
}

fn scan(tx: Sender<DeviceEvent>) -> windows::core::Result<()> {
    let filter = BluetoothLEAdvertisementFilter::new()?;
    let apple = BluetoothLEManufacturerData::new()?;
    apple.SetCompanyId(APPLE)?;
    filter.Advertisement()?.ManufacturerData()?.Append(&apple)?;

    let watcher = BluetoothLEAdvertisementWatcher::Create(&filter)?;
    watcher.SetScanningMode(BluetoothLEScanningMode::Passive)?;

    let last: Mutex<Option<AirPods>> = Mutex::new(None);
    watcher.Received(&TypedEventHandler::new(
        move |_, args: windows::core::Ref<BluetoothLEAdvertisementReceivedEventArgs>| {
            let Some(args) = args.as_ref() else { return Ok(()) };
            if args.RawSignalStrengthInDBm()? < MIN_RSSI {
                return Ok(());
            }
            for item in args.Advertisement()?.ManufacturerData()? {
                if item.CompanyId()? != APPLE {
                    continue;
                }
                let buffer = item.Data()?;
                let reader = DataReader::FromBuffer(&buffer)?;
                let mut bytes = vec![0u8; buffer.Length()? as usize];
                reader.ReadBytes(&mut bytes)?;
                if let Some(state) = parse(&bytes) {
                    let mut last = last.lock().unwrap();
                    if last.as_ref() != Some(&state) {
                        let _ = tx.send(to_event(&state));
                        *last = Some(state);
                    }
                }
            }
            Ok(())
        },
    ))?;
    watcher.Start()?;
    // Scans for the life of the process.
    std::mem::forget(watcher);
    Ok(())
}

/// Parse the manufacturer data payload (the bytes after the company id).
pub fn parse(d: &[u8]) -> Option<AirPods> {
    // d[0] type 0x07 = proximity pairing, d[1] length, d[2] prefix 0x01
    if d.len() < 8 || d[0] != 0x07 || d[2] != 0x01 {
        return None;
    }
    let model = match u16::from_be_bytes([d[3], d[4]]) {
        0x0220 => "AirPods",
        0x0F20 => "AirPods 2",
        0x1320 => "AirPods 3",
        0x0E20 => "AirPods Pro",
        0x1420 => "AirPods Pro 2",
        0x0A20 => "AirPods Max",
        _ => "AirPods",
    };
    // bit 1 of the status byte's high nibble says which nibble of the battery byte is the left bud
    let flipped = (d[5] >> 4) & 0x02 == 0;
    let (hi, lo) = (d[6] >> 4, d[6] & 0x0F);
    let (left, right) = if flipped { (hi, lo) } else { (lo, hi) };
    let flags = d[7] >> 4;
    let case = d[7] & 0x0F;
    let (left_charging, right_charging) = if flipped {
        (flags & 0b10 != 0, flags & 0b01 != 0)
    } else {
        (flags & 0b01 != 0, flags & 0b10 != 0)
    };

    Some(AirPods {
        model,
        left: percent(left),
        right: percent(right),
        case: percent(case),
        left_charging,
        right_charging,
        case_charging: flags & 0b100 != 0,
    })
}

/// 0-10 in tenths; 15 means "not available" (bud out of range or in the case with it closed).
fn percent(raw: u8) -> Option<u8> {
    (raw <= 10).then_some(raw * 10)
}

fn to_event(a: &AirPods) -> DeviceEvent {
    let mut parts = Vec::new();
    for (key, level, charging) in [
        ("left", a.left, a.left_charging),
        ("right", a.right, a.right_charging),
        ("case", a.case, a.case_charging),
    ] {
        if let Some(level) = level {
            parts.push(Part { key: key.into(), level, charging });
        }
    }
    DeviceEvent {
        id: "airpods".into(),
        device_type: DeviceType::Bluetooth,
        name: a.model.into(),
        status: Status::Connected,
        parts,
        metrics: Metrics::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 07 19 01 | model 0E20 | status 0x55 | battery 0x98 | flags/case 0x45 | lid ...
    fn payload(status: u8, battery: u8, flags_case: u8) -> Vec<u8> {
        let mut d = vec![0x07, 0x19, 0x01, 0x0E, 0x20, status, battery, flags_case, 0x00];
        d.resize(27, 0);
        d
    }

    #[test]
    fn parses_levels_and_model() {
        // status high nibble 2 -> not flipped: low nibble of battery byte is left
        let a = parse(&payload(0x25, 0x98, 0x45)).unwrap();
        assert_eq!(a.model, "AirPods Pro");
        assert_eq!((a.left, a.right, a.case), (Some(80), Some(90), Some(50)));
        assert!(a.case_charging && !a.left_charging && !a.right_charging);
    }

    #[test]
    fn flip_bit_swaps_left_and_right() {
        // status high nibble 0 -> flipped: high nibble of battery byte is left
        let a = parse(&payload(0x05, 0x98, 0x03)).unwrap();
        assert_eq!((a.left, a.right), (Some(90), Some(80)));
        assert!(a.left_charging == false && a.right_charging == false); // flags nibble is 0
        let charging = parse(&payload(0x05, 0x98, 0x33)).unwrap();
        assert!(charging.left_charging && charging.right_charging);
    }

    #[test]
    fn unavailable_parts_are_omitted_from_the_event() {
        let a = parse(&payload(0x25, 0xFA, 0x0F)).unwrap(); // right=15 (n/a), left=10, case=15 (n/a)
        let e = to_event(&a);
        assert_eq!(e.parts.len(), 1);
        assert_eq!((e.parts[0].key.as_str(), e.parts[0].level), ("left", 100));
    }

    #[test]
    fn rejects_other_apple_messages() {
        assert_eq!(parse(&[0x10, 0x05, 0x01, 0x18, 0x00, 0x00, 0x00, 0x00]), None);
        assert_eq!(parse(&[0x07, 0x19]), None);
    }
}
