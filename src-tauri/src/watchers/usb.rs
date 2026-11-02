//! Removable drives. Windows tells us when a volume arrives or leaves (no polling); on each
//! notification we diff the set of removable drive letters.
//!
//! Limitation: Windows reports USB hard drives and SSDs as "fixed" disks, so only removable
//! media (flash drives, SD cards) is detected for now.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;

use windows::core::{GUID, PCWSTR};
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Register_Notification, CM_NOTIFY_ACTION, CM_NOTIFY_EVENT_DATA, CM_NOTIFY_FILTER,
    CM_NOTIFY_FILTER_0, CM_NOTIFY_FILTER_0_0, CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE, CR_SUCCESS,
    HCMNOTIFICATION,
};
use windows::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
};

use super::{wait_for_change, Watcher};
use crate::events::{DeviceEvent, DeviceType, Metrics, Status};

const DRIVE_REMOVABLE: u32 = 2;
/// GUID_DEVINTERFACE_VOLUME
const VOLUME_INTERFACE: GUID = GUID::from_u128(0x53f5630d_b6bf_11d0_94f2_00a0c91efb8b);
/// A new volume needs a moment before Windows has mounted it and assigned a letter.
const QUIET: Duration = Duration::from_millis(900);

pub struct UsbWatcher;

impl Watcher for UsbWatcher {
    fn name(&self) -> &'static str {
        "usb"
    }

    fn start(self: Box<Self>, tx: Sender<DeviceEvent>) {
        let (wake_tx, wake_rx) = mpsc::channel::<()>();
        let ctx = Box::into_raw(Box::new(wake_tx)) as *const c_void;

        let filter = CM_NOTIFY_FILTER {
            cbSize: std::mem::size_of::<CM_NOTIFY_FILTER>() as u32,
            FilterType: CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE,
            u: CM_NOTIFY_FILTER_0 { DeviceInterface: CM_NOTIFY_FILTER_0_0 { ClassGuid: VOLUME_INTERFACE } },
            ..Default::default()
        };
        let mut handle = HCMNOTIFICATION::default();
        let rc = unsafe { CM_Register_Notification(&filter, Some(ctx), Some(on_volume_event), &mut handle) };
        if rc != CR_SUCCESS {
            eprintln!("[orbi] usb: CM_Register_Notification failed: {rc:?}");
            return;
        }

        thread::spawn(move || {
            let mut known = removable_drives(); // drives already plugged in do not pop up at launch
            while wait_for_change(&wake_rx, QUIET) {
                let now = removable_drives();
                for (id, event) in &now {
                    if !known.contains_key(id) {
                        let _ = tx.send(event.clone());
                    }
                }
                for (id, event) in &known {
                    if !now.contains_key(id) {
                        let mut gone = event.clone();
                        gone.status = Status::Disconnected;
                        let _ = tx.send(gone);
                    }
                }
                known = now;
            }
        });
    }
}

unsafe extern "system" fn on_volume_event(
    _notify: HCMNOTIFICATION,
    ctx: *const c_void,
    _action: CM_NOTIFY_ACTION,
    _data: *const CM_NOTIFY_EVENT_DATA,
    _size: u32,
) -> u32 {
    // SAFETY: ctx is the leaked Sender<()> created in `start`, valid for the process lifetime.
    let tx = unsafe { &*(ctx as *const Sender<()>) };
    let _ = tx.send(());
    0 // ERROR_SUCCESS
}

fn removable_drives() -> HashMap<String, DeviceEvent> {
    let mut out = HashMap::new();
    let mask = unsafe { GetLogicalDrives() };
    for i in 0..26u32 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let letter = (b'A' + i as u8) as char;
        let root: Vec<u16> = format!("{letter}:\\").encode_utf16().chain(Some(0)).collect();
        if unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) } != DRIVE_REMOVABLE {
            continue;
        }
        let id = format!("usb:{letter}");
        out.insert(id.clone(), drive_event(id, letter, &root));
    }
    out
}

fn drive_event(id: String, letter: char, root: &[u16]) -> DeviceEvent {
    let mut label = [0u16; 261];
    let has_label = unsafe {
        GetVolumeInformationW(PCWSTR(root.as_ptr()), Some(&mut label), None, None, None, None).is_ok()
    };
    let label = if has_label { from_utf16(&label) } else { String::new() };
    let name = if label.is_empty() { format!("USB drive ({letter}:)") } else { format!("{label} ({letter}:)") };

    let (mut free, mut total) = (0u64, 0u64);
    let sized = unsafe { GetDiskFreeSpaceExW(PCWSTR(root.as_ptr()), Some(&mut free), Some(&mut total), None).is_ok() };
    let metrics = if sized && total > 0 {
        Metrics {
            used_pct: Some((100 - free * 100 / total) as u8),
            free_space: Some(format!("{} free", format_size(free))),
            ..Default::default()
        }
    } else {
        Metrics::default()
    };

    DeviceEvent { id, device_type: DeviceType::Usb, name, status: Status::Connected, parts: vec![], metrics }
}

fn from_utf16(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn format_size(bytes: u64) -> String {
    const GB: f64 = 1_000_000_000.0;
    let b = bytes as f64;
    if b >= 1000.0 * GB {
        format!("{:.1} TB", b / (1000.0 * GB))
    } else if b >= GB {
        format!("{:.1} GB", b / GB)
    } else {
        format!("{:.0} MB", b / 1_000_000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_formatting() {
        assert_eq!(format_size(26_800_000_000), "26.8 GB");
        assert_eq!(format_size(512_000_000), "512 MB");
        assert_eq!(format_size(2_000_000_000_000), "2.0 TB");
    }

    #[test]
    fn removable_scan_runs_on_this_machine() {
        println!("removable drives: {:?}", removable_drives().keys().collect::<Vec<_>>());
    }
}
