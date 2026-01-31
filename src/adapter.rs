//! WLAN adapter enable: PowerShell (InterfaceType = Wireless80211) then netsh fallback.

use std::process::Command;

/// NetworkInterfaceType.Wireless80211 (IEEE 802.11)
/// https://learn.microsoft.com/en-us/dotnet/api/system.net.networkinformation.networkinterfacetype
const INTERFACE_TYPE_WIRELESS_80211: i32 = 71;

/// Try to enable WLAN adapter via PowerShell: Get-NetAdapter | Where-Object InterfaceType -eq 71 | Enable-NetAdapter.
/// Then fallback to netsh with common interface names.
/// When WlanEnumInterfaces returns 0, adapter is often disabled; call this then retry enum.
pub fn try_enable_wlan_adapter() -> bool {
    // 1. PowerShell: enable adapters by InterfaceType = Wireless80211 (71)
    let ps = format!(
        "Get-NetAdapter -ErrorAction SilentlyContinue | Where-Object {{ $_.InterfaceType -eq {} }} | Enable-NetAdapter -Confirm:$false -ErrorAction SilentlyContinue",
        INTERFACE_TYPE_WIRELESS_80211
    );
    tracing::info!("Trying to enable WLAN adapter via PowerShell (InterfaceType = Wireless80211)");
    match Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps])
        .output()
    {
        Ok(out) if out.status.success() => {
            tracing::info!("PowerShell Enable-NetAdapter (InterfaceType=71) succeeded");
            return true;
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let stdout = String::from_utf8_lossy(&out.stdout);
            if !stderr.is_empty() {
                tracing::info!("PowerShell Enable-NetAdapter failed: {}", stderr.trim());
            }
            if !stdout.is_empty() && !out.status.success() {
                tracing::info!("PowerShell output: {}", stdout.trim());
            }
        }
        Err(e) => {
            tracing::info!("Failed to run PowerShell: {}", e);
        }
    }

    // 2. Fallback: netsh with common WLAN interface names
    const WLAN_INTERFACE_NAMES: &[&str] =
        &["Wi-Fi", "WLAN", "Wireless", "Wireless Network Connection"];
    tracing::info!("PowerShell did not enable WLAN, trying netsh fallback");
    for name in WLAN_INTERFACE_NAMES {
        tracing::info!("Trying to enable interface: \"{}\"", name);
        let status = Command::new("netsh")
            .args([
                "interface",
                "set",
                "interface",
                &format!("name=\"{}\"", name),
                "admin=enable",
            ])
            .output();

        match status {
            Ok(out) if out.status.success() => {
                tracing::info!("Enabled interface: \"{}\"", name);
                return true;
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let stdout = String::from_utf8_lossy(&out.stdout);
                if !stderr.is_empty() {
                    tracing::info!("netsh enable \"{}\" failed: {}", name, stderr.trim());
                }
                if !stdout.is_empty() && !out.status.success() {
                    tracing::info!("netsh output: {}", stdout.trim());
                }
            }
            Err(e) => {
                tracing::info!("Failed to run netsh: {}", e);
            }
        }
    }

    tracing::warn!("No WLAN interface could be enabled (PowerShell + netsh fallback)");
    false
}
