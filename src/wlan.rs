//! WLAN client: enumerate interfaces, saved profiles, connect

use crate::adapter;
use std::collections::HashSet;
use std::ptr::NonNull;
use windows::core::PCWSTR;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::NetworkManagement::WiFi::{
    dot11_BSS_type_any, wlan_connection_mode_profile, wlan_interface_state_connected,
    wlan_intf_opcode_interface_state, WlanCloseHandle, WlanConnect, WlanEnumInterfaces,
    WlanFreeMemory, WlanGetAvailableNetworkList, WlanGetProfileList, WlanOpenHandle,
    WlanQueryInterface, WlanScan, WLAN_CONNECTION_PARAMETERS, WLAN_INTERFACE_STATE,
};

/// WLAN client handle wrapper
pub struct WlanClient {
    pub(crate) handle: HANDLE,
}

impl WlanClient {
    pub fn new() -> anyhow::Result<Self> {
        unsafe {
            let mut negotiated = 0u32;
            let mut handle = HANDLE::default();
            // WLAN_CLIENT_VERSION_V2 = 2
            let status = WlanOpenHandle(2, None, &mut negotiated, &mut handle);

            if status != 0 {
                anyhow::bail!("WlanOpenHandle failed: {}", status);
            }

            Ok(Self { handle })
        }
    }

    /// Connect to the given profile on the given interface
    pub fn connect_profile(
        &self,
        iface: &windows::core::GUID,
        profile: &str,
    ) -> anyhow::Result<()> {
        unsafe {
            let wide: Vec<u16> = profile.encode_utf16().chain(std::iter::once(0)).collect();
            let params = WLAN_CONNECTION_PARAMETERS {
                wlanConnectionMode: wlan_connection_mode_profile,
                strProfile: PCWSTR::from_raw(wide.as_ptr()),
                pDot11Ssid: std::ptr::null_mut(),
                pDesiredBssidList: std::ptr::null_mut(),
                dot11BssType: dot11_BSS_type_any,
                dwFlags: 0,
            };

            let status = WlanConnect(self.handle, iface, &params, None);

            if status != 0 {
                anyhow::bail!("WlanConnect({}) failed: {}", profile, status);
            }

            Ok(())
        }
    }
}

impl Drop for WlanClient {
    fn drop(&mut self) {
        unsafe {
            let _ = WlanCloseHandle(self.handle, None);
        }
    }
}

/// Get all WLAN interface GUIDs
unsafe fn get_wlan_interfaces(handle: HANDLE) -> anyhow::Result<Vec<windows::core::GUID>> {
    let mut list = std::ptr::null_mut();
    let status = WlanEnumInterfaces(handle, None, &mut list);

    if status != 0 {
        anyhow::bail!("WlanEnumInterfaces failed: {}", status);
    }

    let list =
        NonNull::new(list).ok_or_else(|| anyhow::anyhow!("WlanEnumInterfaces returned null"))?;
    let count = list.as_ref().dwNumberOfItems as usize;

    let interfaces: Vec<_> = (0..count)
        .map(|i| {
            let base = list.as_ref().InterfaceInfo.as_ptr();
            (*base.add(i)).InterfaceGuid
        })
        .collect();

    WlanFreeMemory(list.as_ptr().cast());
    Ok(interfaces)
}

/// String from [u16; 256] up to first NUL
fn wide_to_string(name: &[u16; 256]) -> String {
    let len = name.iter().position(|&c| c == 0).unwrap_or(256);
    String::from_utf16_lossy(&name[..len])
}

/// DOT11_SSID to string (SSID can be arbitrary bytes; lossy UTF-8)
fn dot11_ssid_to_string(ssid: &windows::Win32::NetworkManagement::WiFi::DOT11_SSID) -> String {
    let len = ssid.uSSIDLength.min(32) as usize;
    String::from_utf8_lossy(&ssid.ucSSID[..len]).into_owned()
}

/// Get set of currently visible (in-range) network names: SSID strings + existing profile names.
/// Optionally trigger a scan first to refresh the list.
unsafe fn get_available_network_names(
    handle: HANDLE,
    iface: &windows::core::GUID,
    trigger_scan: bool,
) -> anyhow::Result<HashSet<String>> {
    if trigger_scan {
        let _ = WlanScan(handle, iface, None, None, None);
        // Caller decides whether to sleep
    }
    let mut list = std::ptr::null_mut();
    // dwflags 0 = default
    let status = WlanGetAvailableNetworkList(handle, iface, 0, None, &mut list);
    if status != 0 {
        anyhow::bail!("WlanGetAvailableNetworkList failed: {}", status);
    }
    let list = NonNull::new(list)
        .ok_or_else(|| anyhow::anyhow!("WlanGetAvailableNetworkList returned null"))?;
    let count = list.as_ref().dwNumberOfItems as usize;
    let mut names = HashSet::new();
    for i in 0..count {
        let base = list.as_ref().Network.as_ptr();
        let net = &*base.add(i);
        let profile_name = wide_to_string(&net.strProfileName);
        if !profile_name.is_empty() {
            names.insert(profile_name);
        }
        let ssid_str = dot11_ssid_to_string(&net.dot11Ssid);
        if !ssid_str.is_empty() {
            names.insert(ssid_str);
        }
    }
    WlanFreeMemory(list.as_ptr().cast());
    Ok(names)
}

/// Connect strategy: visible only / all saved / explicit list
#[derive(Clone, Debug)]
pub enum ConnectStrategy {
    /// Only try saved profiles that match currently visible networks
    ScanOnly,
    /// Try all saved profiles (no visibility filter)
    All,
    /// Only try these profile names (CLI-specified)
    Explicit(Vec<String>),
}

/// Get all saved profile names for the given interface
unsafe fn get_saved_profiles(
    handle: HANDLE,
    iface: &windows::core::GUID,
) -> anyhow::Result<Vec<String>> {
    let mut list = std::ptr::null_mut();
    let status = WlanGetProfileList(handle, iface, None, &mut list);

    if status != 0 {
        anyhow::bail!("WlanGetProfileList failed: {}", status);
    }

    let list =
        NonNull::new(list).ok_or_else(|| anyhow::anyhow!("WlanGetProfileList returned null"))?;
    let count = list.as_ref().dwNumberOfItems as usize;

    let profiles: Vec<String> = (0..count)
        .map(|i| {
            let base = list.as_ref().ProfileInfo.as_ptr();
            wide_to_string(&(*base.add(i)).strProfileName)
        })
        .collect();

    WlanFreeMemory(list.as_ptr().cast());
    Ok(profiles)
}

/// Query current WLAN interface state (connected / associating / disconnected etc.)
unsafe fn get_wlan_interface_state(
    handle: HANDLE,
    iface: &windows::core::GUID,
) -> Option<WLAN_INTERFACE_STATE> {
    let mut size = 0u32;
    let mut pdata = std::ptr::null_mut();
    let status = WlanQueryInterface(
        handle,
        iface,
        wlan_intf_opcode_interface_state,
        None,
        &mut size,
        &mut pdata,
        None,
    );
    if status != 0 || pdata.is_null() {
        return None;
    }
    let state = *pdata.cast::<WLAN_INTERFACE_STATE>();
    WlanFreeMemory(pdata.cast());
    Some(state)
}

/// Poll WLAN interface connection state until \"connected\" or timeout. Uses connection state, not NCSI.
async fn poll_wlan_connection_state(
    handle: HANDLE,
    iface: &windows::core::GUID,
    max_wait_secs: u64,
    interval_secs: u64,
) -> bool {
    let rounds = (max_wait_secs / interval_secs).max(1);
    for round in 1..=rounds {
        tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs)).await;
        let state = unsafe { get_wlan_interface_state(handle, iface) };
        tracing::info!(
            "WLAN state poll #{}/{} ({}s/{}s): {:?}",
            round,
            rounds,
            round * interval_secs,
            max_wait_secs,
            state
        );
        if state == Some(wlan_interface_state_connected) {
            return true;
        }
    }
    false
}

/// Filter profiles by strategy: only those we should try
fn filter_profiles_by_strategy(
    saved: &[String],
    strategy: &ConnectStrategy,
    available_names: Option<&HashSet<String>>,
) -> Vec<String> {
    match strategy {
        ConnectStrategy::ScanOnly => {
            let avail = match available_names {
                Some(s) => s,
                None => return Vec::new(),
            };
            saved
                .iter()
                .filter(|p| avail.contains(*p))
                .cloned()
                .collect()
        }
        ConnectStrategy::All => saved.to_vec(),
        ConnectStrategy::Explicit(names) => {
            let set: HashSet<_> = names.iter().map(String::as_str).collect();
            saved
                .iter()
                .filter(|p| set.contains(p.as_str()))
                .cloned()
                .collect()
        }
    }
}

/// Enumerate saved profiles, filter by strategy, try connecting until NCSI passes
pub async fn connect_any_saved_wifi(
    test_network: impl Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>,
    strategy: ConnectStrategy,
) -> anyhow::Result<()> {
    tracing::info!("Initializing WLAN client...");
    let client = WlanClient::new()?;
    tracing::info!("WLAN client ready");

    let mut ifaces = unsafe { get_wlan_interfaces(client.handle)? };
    tracing::info!("Found {} WLAN interface(s)", ifaces.len());

    if ifaces.is_empty() {
        tracing::warn!("No WLAN interface; adapter may be disabled, trying to enable...");
        if adapter::try_enable_wlan_adapter() {
            tracing::info!("Waiting 3s then re-enumerating WLAN interfaces...");
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            ifaces = unsafe { get_wlan_interfaces(client.handle)? };
            tracing::info!("Re-enum: {} WLAN interface(s)", ifaces.len());
        }
    }

    if ifaces.is_empty() {
        anyhow::bail!("No WLAN interface (tried enabling common adapters)");
    }

    let mut tried = 0u32;
    for (idx, iface) in ifaces.iter().enumerate() {
        let saved = match unsafe { get_saved_profiles(client.handle, iface) } {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    "Interface #{}: get profile list failed: {}, skip",
                    idx + 1,
                    e
                );
                continue;
            }
        };
        tracing::info!("Interface #{}: {} saved profile(s)", idx + 1, saved.len());

        let available_names = match &strategy {
            ConnectStrategy::ScanOnly => {
                tracing::info!("Scanning visible networks (connect only in-range)...");
                unsafe {
                    let _ = WlanScan(client.handle, iface, None, None, None);
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                match unsafe { get_available_network_names(client.handle, iface, false) } {
                    Ok(n) => {
                        tracing::debug!("{} visible network(s): {:?}", n.len(), n);
                        Some(n)
                    }
                    Err(e) => {
                        tracing::warn!("Get visible list failed: {}, skip interface", e);
                        continue;
                    }
                }
            }
            _ => None,
        };

        let profiles = filter_profiles_by_strategy(&saved, &strategy, available_names.as_ref());
        if profiles.is_empty() {
            tracing::info!("No profiles to try after filter (strategy: {:?})", strategy);
            continue;
        }
        let profiles_count = profiles.len();
        tracing::debug!(
            "{} profile(s) to try on this interface: {:?}",
            profiles_count,
            profiles
        );

        for profile in profiles {
            tried += 1;
            tracing::info!("[{}/{}] Connecting: \"{}\"", tried, profiles_count, profile);

            if let Err(e) = client.connect_profile(iface, &profile) {
                tracing::info!("Connect \"{}\" failed: {}", profile, e);
                continue;
            }

            tracing::info!("Connect requested, polling WLAN state (every 2s, up to 30s)...");
            if !poll_wlan_connection_state(client.handle, iface, 30, 2).await {
                tracing::info!(
                    "\"{}\" timed out (never reached connected), try next",
                    profile
                );
                continue;
            }
            tracing::info!("WLAN connected, checking network...");
            if test_network().await {
                tracing::info!("Network restored via \"{}\"", profile);
                return Ok(());
            }
            tracing::info!("\"{}\" connected but NCSI failed, try next", profile);
        }
    }

    tracing::warn!("Tried {} profile(s), none restored network", tried);
    anyhow::bail!("No saved Wi-Fi profile could establish network");
}
