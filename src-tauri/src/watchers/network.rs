//! Wi-Fi and Ethernet. Windows tells us when any IP interface changes (no polling); on each
//! change we re-read the interface table and report what connected or disconnected.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::{HANDLE, WIN32_ERROR};
use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetIfTable2, NotifyIpInterfaceChange, MIB_IF_TABLE2, MIB_IPINTERFACE_ROW,
    MIB_NOTIFICATION_TYPE,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::NetworkManagement::WiFi::{
    wlan_intf_opcode_current_connection, wlan_interface_state_connected, WlanCloseHandle,
    WlanEnumInterfaces, WlanFreeMemory, WlanOpenHandle, WlanQueryInterface,
    WLAN_CONNECTION_ATTRIBUTES, WLAN_INTERFACE_INFO_LIST,
};
use windows::Win32::Networking::WinSock::AF_UNSPEC;

use super::{wait_for_change, Watcher};
use crate::events::{DeviceEvent, DeviceType, Metrics, Status};

const IF_TYPE_ETHERNET: u32 = 6;
const IF_TYPE_WIFI: u32 = 71;
const QUIET: Duration = Duration::from_millis(600);

pub struct NetworkWatcher;

impl Watcher for NetworkWatcher {
    fn name(&self) -> &'static str {
        "network"
    }

    fn start(self: Box<Self>, tx: Sender<DeviceEvent>) {
        let (wake_tx, wake_rx) = mpsc::channel::<()>();
        // Leaked on purpose: the registration lives for the whole process.
        let ctx = Box::into_raw(Box::new(wake_tx)) as *const c_void;
        let mut handle = HANDLE::default();
        let err = unsafe { NotifyIpInterfaceChange(AF_UNSPEC, Some(on_change), Some(ctx), false, &mut handle) };
        if err != WIN32_ERROR(0) {
            eprintln!("[orbi] network: NotifyIpInterfaceChange failed: {err:?}");
            return;
        }

        thread::spawn(move || {
            let mut known = snapshot();
            for event in known.values() {
                let _ = tx.send(event.clone());
            }
            while wait_for_change(&wake_rx, QUIET) {
                let now = snapshot();
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

unsafe extern "system" fn on_change(ctx: *const c_void, _row: *const MIB_IPINTERFACE_ROW, _kind: MIB_NOTIFICATION_TYPE) {
    // SAFETY: ctx is the leaked Sender<()> created in `start`, valid for the process lifetime.
    let tx = unsafe { &*(ctx as *const Sender<()>) };
    let _ = tx.send(());
}

struct WifiInfo {
    ssid: String,
    signal: u8,
    rx_mbps: u32,
}

/// Currently connected networks keyed by a stable id ("wifi:<ssid>", "ethernet:<alias>").
fn snapshot() -> HashMap<String, DeviceEvent> {
    let mut out = HashMap::new();
    let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
    if unsafe { GetIfTable2(&mut table) } != WIN32_ERROR(0) || table.is_null() {
        return out;
    }
    let rows = unsafe { std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize) };
    let wifi = wifi_connection();

    for row in rows {
        if row.OperStatus != IfOperStatusUp {
            continue;
        }
        // bit 0 of the flags is HardwareInterface: skips Hyper-V, VPN and other virtual adapters
        let hardware = row.InterfaceAndOperStatusFlags._bitfield & 1 == 1;
        if !hardware {
            continue;
        }
        let alias = utf16(&row.Alias);
        match row.Type {
            IF_TYPE_ETHERNET => {
                let id = format!("ethernet:{alias}");
                out.insert(id.clone(), event(id, DeviceType::Ethernet, "Ethernet".into(), Metrics {
                    speed: Some(format_speed(row.TransmitLinkSpeed)),
                    ..Default::default()
                }));
            }
            IF_TYPE_WIFI => {
                if let Some(w) = &wifi {
                    let id = format!("wifi:{}", w.ssid);
                    out.insert(id.clone(), event(id, DeviceType::Wifi, w.ssid.clone(), Metrics {
                        ssid: Some(w.ssid.clone()),
                        signal: Some(w.signal),
                        speed: Some(format!("{} Mbps", w.rx_mbps)),
                        ..Default::default()
                    }));
                }
            }
            _ => {}
        }
    }
    unsafe { FreeMibTable(table as *const c_void) };
    out
}

fn event(id: String, device_type: DeviceType, name: String, metrics: Metrics) -> DeviceEvent {
    DeviceEvent { id, device_type, name, status: Status::Connected, parts: vec![], metrics }
}

fn utf16(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn format_speed(bps: u64) -> String {
    if bps >= 1_000_000_000 {
        format!("{} Gbps", bps / 1_000_000_000)
    } else {
        format!("{} Mbps", bps / 1_000_000)
    }
}

/// SSID, signal quality (0-100) and receive rate of the connected Wi-Fi network, if any.
fn wifi_connection() -> Option<WifiInfo> {
    unsafe {
        let mut negotiated = 0u32;
        let mut client = HANDLE::default();
        if WlanOpenHandle(2, None, &mut negotiated, &mut client) != 0 {
            return None;
        }
        let mut result = None;
        let mut list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
        if WlanEnumInterfaces(client, None, &mut list) == 0 && !list.is_null() {
            let infos = std::slice::from_raw_parts((*list).InterfaceInfo.as_ptr(), (*list).dwNumberOfItems as usize);
            for info in infos {
                if info.isState != wlan_interface_state_connected {
                    continue;
                }
                let mut size = 0u32;
                let mut data: *mut c_void = std::ptr::null_mut();
                let rc = WlanQueryInterface(
                    client,
                    &info.InterfaceGuid,
                    wlan_intf_opcode_current_connection,
                    None,
                    &mut size,
                    &mut data,
                    None,
                );
                if rc == 0 && !data.is_null() {
                    let attrs = &*(data as *const WLAN_CONNECTION_ATTRIBUTES);
                    let assoc = &attrs.wlanAssociationAttributes;
                    let len = (assoc.dot11Ssid.uSSIDLength as usize).min(32);
                    result = Some(WifiInfo {
                        ssid: String::from_utf8_lossy(&assoc.dot11Ssid.ucSSID[..len]).into_owned(),
                        signal: assoc.wlanSignalQuality.min(100) as u8,
                        rx_mbps: assoc.ulRxRate / 1000,
                    });
                    WlanFreeMemory(data);
                    break;
                }
            }
            WlanFreeMemory(list as *const c_void);
        }
        WlanCloseHandle(client, None);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_formatting() {
        assert_eq!(format_speed(1_000_000_000), "1 Gbps");
        assert_eq!(format_speed(866_000_000), "866 Mbps");
    }

    #[test]
    fn utf16_stops_at_nul() {
        let buf: Vec<u16> = "Wi-Fi\0junk".encode_utf16().collect();
        assert_eq!(utf16(&buf), "Wi-Fi");
    }

    /// Reads the real adapters of the machine running the tests; prints what it saw.
    #[test]
    fn snapshot_runs_on_this_machine() {
        for (id, e) in snapshot() {
            println!("{id}: {} {:?}", e.name, e.metrics);
        }
    }
}
