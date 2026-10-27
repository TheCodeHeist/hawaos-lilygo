//! The Wi-Fi networks around, for setup, from the WLAN API: 2.4 GHz only
//! (the ESP32-S3 has no 5 GHz radio), strongest first.

use std::collections::HashSet;
use std::ptr;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::NetworkManagement::WiFi::{
    dot11_BSS_type_any, WlanCloseHandle, WlanEnumInterfaces, WlanFreeMemory, WlanGetAvailableNetworkList,
    WlanGetNetworkBssList, WlanOpenHandle, DOT11_SSID, WLAN_AVAILABLE_NETWORK_CONNECTED,
    WLAN_AVAILABLE_NETWORK_LIST, WLAN_BSS_LIST, WLAN_INTERFACE_INFO_LIST,
};

use crate::config::DiscoveredWifi;

fn ssid(s: &DOT11_SSID) -> String {
    let len = (s.uSSIDLength as usize).min(s.ucSSID.len());
    String::from_utf8_lossy(&s.ucSSID[..len]).into_owned()
}

pub fn scan_wifi_networks() -> Vec<DiscoveredWifi> {
    let mut list = Vec::new();
    unsafe {
        let mut version = 0u32;
        let mut handle = HANDLE::default();
        if WlanOpenHandle(2, None, &mut version, &mut handle) != 0 {
            return list;
        }
        let mut ifaces: *mut WLAN_INTERFACE_INFO_LIST = ptr::null_mut();
        if WlanEnumInterfaces(handle, None, &mut ifaces) == 0 && !ifaces.is_null() {
            let count = (*ifaces).dwNumberOfItems as usize;
            let infos = std::slice::from_raw_parts((*ifaces).InterfaceInfo.as_ptr(), count);
            let mut seen = HashSet::new();
            for info in infos {
                let guid = &info.InterfaceGuid;

                // Networks with an access point on 2.4 GHz (centre frequency
                // in kHz).
                let mut on_24ghz = HashSet::new();
                let mut bss: *mut WLAN_BSS_LIST = ptr::null_mut();
                if WlanGetNetworkBssList(handle, guid, None, dot11_BSS_type_any, false, None, &mut bss) == 0
                    && !bss.is_null()
                {
                    let n = (*bss).dwNumberOfItems as usize;
                    for e in std::slice::from_raw_parts((*bss).wlanBssEntries.as_ptr(), n) {
                        if e.ulChCenterFrequency < 3_000_000 {
                            on_24ghz.insert(ssid(&e.dot11Ssid));
                        }
                    }
                    WlanFreeMemory(bss as *const _);
                }

                let mut nets: *mut WLAN_AVAILABLE_NETWORK_LIST = ptr::null_mut();
                if WlanGetAvailableNetworkList(handle, guid, 0, None, &mut nets) == 0 && !nets.is_null() {
                    let n = (*nets).dwNumberOfItems as usize;
                    for net in std::slice::from_raw_parts((*nets).Network.as_ptr(), n) {
                        let name = ssid(&net.dot11Ssid);
                        if name.is_empty() || !on_24ghz.contains(&name) || !seen.insert(name.clone()) {
                            continue;
                        }
                        list.push(DiscoveredWifi {
                            ssid: name,
                            signal: net.wlanSignalQuality.min(100) as u8,
                            security: if net.bSecurityEnabled.as_bool() { "WPA".into() } else { String::new() },
                            in_use: net.dwFlags & WLAN_AVAILABLE_NETWORK_CONNECTED != 0,
                        });
                    }
                    WlanFreeMemory(nets as *const _);
                }
            }
            WlanFreeMemory(ifaces as *const _);
        }
        WlanCloseHandle(handle, None);
    }
    list.sort_by(|a, b| b.signal.cmp(&a.signal));
    list.truncate(10);
    list
}
