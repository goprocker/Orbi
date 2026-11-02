//! Generic Bluetooth earbuds and headphones: connect, disconnect and a single battery level.
//!
//! A WinRT `DeviceWatcher` on connected Bluetooth devices wakes the worker (no polling while
//! nothing is connected). The battery percentage comes from the Windows device property
//! `{104EA319-6EE2-4701-BD47-8DDBF425BBE5} 2`, read through the configuration manager. While
//! at least one device is connected the worker re-reads it every `REFRESH`, because Windows
//! sends no notification when only the battery changes.
//!
//! Windows reports one battery value per device here; separate left / right / case levels are
//! only available for AirPods-style broadcasts (see airpods.rs).

use std::collections::HashMap;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use windows::core::{HSTRING, PCWSTR};
use windows::Devices::Bluetooth::{BluetoothConnectionStatus, BluetoothDevice};
use windows::Devices::Enumeration::DeviceInformation;
use windows::Foundation::TypedEventHandler;
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Get_DevNode_PropertyW, CM_Get_Device_ID_ListW, CM_Get_Device_ID_List_SizeW, CM_Locate_DevNodeW,
    CM_GETIDLIST_FILTER_ENUMERATOR, CM_LOCATE_DEVNODE_NORMAL, CR_SUCCESS,
};
use windows::Win32::Devices::Properties::DEVPROPTYPE;
use windows::Win32::Foundation::DEVPROPKEY;

use super::{wait_for_change, Watcher};
use crate::events::{DeviceEvent, DeviceType, Metrics, Part, Status};

const QUIET: Duration = Duration::from_millis(1500);
const REFRESH: Duration = Duration::from_secs(60);

const BATTERY_KEY: DEVPROPKEY = DEVPROPKEY {
    fmtid: windows::core::GUID::from_u128(0x104ea319_6ee2_4701_bd47_8ddbf425bbe5),
    pid: 2,
};
/// Bluetooth "is connected" flag on a paired device's DEV_ node (a single boolean byte).
const IS_CONNECTED_KEY: DEVPROPKEY = DEVPROPKEY {
    fmtid: windows::core::GUID::from_u128(0x83da6326_97a6_4088_9453_a1923f573b29),
    pid: 15,
};
/// DEVPKEY_Device_FriendlyName
const FRIENDLY_NAME_KEY: DEVPROPKEY = DEVPROPKEY {
    fmtid: windows::core::GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
    pid: 14,
};

pub struct BluetoothWatcher;

impl Watcher for BluetoothWatcher {
    fn name(&self) -> &'static str {
        "bluetooth"
    }

    fn start(self: Box<Self>, tx: Sender<DeviceEvent>) {
        let (wake_tx, wake_rx) = mpsc::channel::<()>();
        if let Err(e) = watch_connections(wake_tx) {
            eprintln!("[orbi] bluetooth: device watcher failed: {e}");
            return;
        }

        thread::spawn(move || {
            let mut known = snapshot();
            // report what is already connected; the pipeline folds these into one launch popup
            for event in known.values() {
                let _ = tx.send(event.clone());
            }
            loop {
                let alive = if known.is_empty() {
                    wait_for_change(&wake_rx, QUIET)
                } else {
                    match wake_rx.recv_timeout(REFRESH) {
                        Ok(()) => {
                            while wake_rx.recv_timeout(QUIET).is_ok() {}
                            true
                        }
                        Err(RecvTimeoutError::Timeout) => true,
                        Err(RecvTimeoutError::Disconnected) => false,
                    }
                };
                if !alive {
                    return;
                }
                let now = snapshot();
                for (id, event) in &now {
                    match known.get(id) {
                        None => {
                            let _ = tx.send(event.clone());
                        }
                        Some(prev) if prev != event => {
                            let mut update = event.clone();
                            update.status = Status::Updated;
                            let _ = tx.send(update);
                        }
                        _ => {}
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

/// Wake the worker whenever a Bluetooth device connects, disconnects or changes.
fn watch_connections(wake: Sender<()>) -> windows::core::Result<()> {
    let selector = BluetoothDevice::GetDeviceSelectorFromConnectionStatus(BluetoothConnectionStatus::Connected)?;
    let watcher = DeviceInformation::CreateWatcherAqsFilter(&selector)?;

    let (a, r, u) = (wake.clone(), wake.clone(), wake);
    watcher.Added(&TypedEventHandler::new(move |_, _| {
        let _ = a.send(());
        Ok(())
    }))?;
    watcher.Removed(&TypedEventHandler::new(move |_, _| {
        let _ = r.send(());
        Ok(())
    }))?;
    watcher.Updated(&TypedEventHandler::new(move |_, _| {
        let _ = u.send(());
        Ok(())
    }))?;
    watcher.Start()?;
    // The watcher must outlive this function; it runs for the life of the process.
    std::mem::forget(watcher);
    Ok(())
}

/// Bluetooth devices that currently report a battery level, keyed by device instance id.
fn snapshot() -> HashMap<String, DeviceEvent> {
    let mut out = HashMap::new();
    let instances = bluetooth_instance_ids();
    // Windows keeps the last battery reading of paired devices that are switched off, so only
    // trust nodes that belong to a device Windows says is connected right now.
    let connected = connected_addresses(&instances);
    for instance in instances {
        if !belongs_to_connected(&instance, &connected) {
            continue;
        }
        let Some(devnode) = locate(&instance) else { continue };
        let Some(level) = read_property::<1>(devnode, &BATTERY_KEY).map(|b| b[0]) else { continue };
        if level > 100 {
            continue;
        }
        let name = read_string(devnode, &FRIENDLY_NAME_KEY).map(|n| clean_name(&n)).unwrap_or_else(|| "Bluetooth device".into());
        let id = format!("bluetooth:{name}"); // HFP, A2DP and AVRCP nodes of one device share a name
        out.insert(
            id.clone(),
            DeviceEvent {
                id,
                device_type: DeviceType::Bluetooth,
                name,
                status: Status::Connected,
                parts: vec![Part { key: "main".into(), level, charging: false }],
                metrics: Metrics::default(),
            },
        );
    }
    out
}

/// Hardware addresses (12 hex digits) of paired devices that are currently connected.
fn connected_addresses(instances: &[String]) -> Vec<String> {
    instances
        .iter()
        .filter_map(|id| {
            let address = device_address(id)?;
            let devnode = locate(id)?;
            let flag = read_property::<1>(devnode, &IS_CONNECTED_KEY)?;
            (flag[0] != 0).then_some(address)
        })
        .collect()
}

/// The address in a paired device's own node id, e.g. `BTHENUM\DEV_4142FCB0FABE\...`.
fn device_address(instance: &str) -> Option<String> {
    let rest = instance.strip_prefix("BTHENUM\\DEV_").or_else(|| instance.strip_prefix("BTHLE\\DEV_"))?;
    let address = rest.split('\\').next()?;
    (address.len() == 12 && address.chars().all(|c| c.is_ascii_hexdigit())).then(|| address.to_ascii_uppercase())
}

/// Profile nodes (hands-free, A2DP, ...) carry the device address inside their own id.
fn belongs_to_connected(instance: &str, connected: &[String]) -> bool {
    let id = instance.to_ascii_uppercase();
    connected.iter().any(|address| id.contains(address.as_str()))
}

fn bluetooth_instance_ids() -> Vec<String> {
    let mut ids = Vec::new();
    for enumerator in ["BTHENUM", "BTHLE"] {
        let filter: Vec<u16> = enumerator.encode_utf16().chain(Some(0)).collect();
        let mut len = 0u32;
        if unsafe { CM_Get_Device_ID_List_SizeW(&mut len, PCWSTR(filter.as_ptr()), CM_GETIDLIST_FILTER_ENUMERATOR) } != CR_SUCCESS || len < 2 {
            continue;
        }
        let mut buf = vec![0u16; len as usize];
        if unsafe { CM_Get_Device_ID_ListW(PCWSTR(filter.as_ptr()), &mut buf, CM_GETIDLIST_FILTER_ENUMERATOR) } != CR_SUCCESS {
            continue;
        }
        // double-NUL terminated list of NUL-separated strings
        ids.extend(buf.split(|&c| c == 0).filter(|s| !s.is_empty()).map(String::from_utf16_lossy));
    }
    ids
}

fn locate(instance: &str) -> Option<u32> {
    let id = HSTRING::from(instance);
    let mut devnode = 0u32;
    // NORMAL = only devices that are currently present
    (unsafe { CM_Locate_DevNodeW(&mut devnode, PCWSTR(id.as_ptr()), CM_LOCATE_DEVNODE_NORMAL) } == CR_SUCCESS).then_some(devnode)
}

fn read_property<const N: usize>(devnode: u32, key: &DEVPROPKEY) -> Option<[u8; N]> {
    let mut kind = DEVPROPTYPE::default();
    let mut buf = [0u8; N];
    let mut size = N as u32;
    let rc = unsafe { CM_Get_DevNode_PropertyW(devnode, key, &mut kind, Some(buf.as_mut_ptr()), &mut size, 0) };
    (rc == CR_SUCCESS).then_some(buf)
}

fn read_string(devnode: u32, key: &DEVPROPKEY) -> Option<String> {
    let mut kind = DEVPROPTYPE::default();
    let mut buf = [0u8; 512];
    let mut size = buf.len() as u32;
    let rc = unsafe { CM_Get_DevNode_PropertyW(devnode, key, &mut kind, Some(buf.as_mut_ptr()), &mut size, 0) };
    if rc != CR_SUCCESS {
        return None;
    }
    let wide: Vec<u16> = buf[..size as usize].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    Some(String::from_utf16_lossy(&wide[..end]))
}

/// Windows names each Bluetooth profile node ("Buds Hands-Free AG"); keep just the device name.
fn clean_name(raw: &str) -> String {
    const SUFFIXES: [&str; 5] = [" Hands-Free AG Audio", " Hands-Free AG", " Hands-Free", " Stereo", " Avrcp Transport"];
    let mut name = raw.trim();
    for suffix in SUFFIXES {
        if let Some(stripped) = name.strip_suffix(suffix) {
            name = stripped;
        }
    }
    name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_suffixes_are_stripped() {
        assert_eq!(clean_name("WH-1000XM4 Hands-Free AG Audio"), "WH-1000XM4");
        assert_eq!(clean_name("Orbi Buds Hands-Free"), "Orbi Buds");
        assert_eq!(clean_name("Orbi Buds"), "Orbi Buds");
    }

    #[test]
    fn extracts_the_device_address() {
        assert_eq!(device_address("BTHENUM\\DEV_4142FCB0FABE\\7&9574CD4&0&BLUETOOTHDEVICE_4142FCB0FABE").as_deref(), Some("4142FCB0FABE"));
        assert_eq!(device_address("BTHENUM\\{0000111E-0000-1000-8000-00805F9B34FB}_LOCALMFG&0002\\7&9574CD4&0&4142FCB0FABE_C00000000"), None);
    }

    #[test]
    fn profile_nodes_match_only_connected_devices() {
        let hfp = "BTHENUM\\{0000111E-0000-1000-8000-00805F9B34FB}_LOCALMFG&0002\\7&9574CD4&0&4142FCB0FABE_C00000000";
        assert!(belongs_to_connected(hfp, &["4142FCB0FABE".into()]));
        assert!(!belongs_to_connected(hfp, &["B1897FF7CD77".into()]));
        assert!(!belongs_to_connected(hfp, &[]));
    }

    /// Runs the real enumeration; on a machine with no battery-reporting earbuds it is empty.
    #[test]
    fn snapshot_runs_on_this_machine() {
        println!("bluetooth devices with battery: {:?}", snapshot().keys().collect::<Vec<_>>());
    }
}
