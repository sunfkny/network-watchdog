//! Wi-Fi Radio control (Windows.Devices.Radios)

use windows::Devices::Radios::{Radio, RadioKind, RadioState};

/// Turn on Wi-Fi radio if currently off
pub async fn turn_on_wifi_radio() -> anyhow::Result<()> {
    tracing::info!("Getting system radio list...");
    let op = Radio::GetRadiosAsync()?;
    // WinRT IAsyncOperation.get() must run on single thread; block here
    let radios = op.get()?;
    let count = radios.Size()?;
    tracing::info!("Found {} radio(s)", count);

    let mut wifi_found = false;
    for i in 0..count {
        let radio = radios.GetAt(i)?;
        if radio.Kind()? == RadioKind::WiFi {
            wifi_found = true;
            let name = radio.Name().unwrap_or_default();
            let state = radio.State()?;
            tracing::info!("Wi-Fi radio \"{}\" state: {:?}", name, state);
            if state != RadioState::On {
                tracing::info!("Turning on Wi-Fi radio...");
                let set_op = radio.SetStateAsync(RadioState::On)?;
                let _ = set_op.get()?;
                tracing::info!("Wi-Fi radio on");
            } else {
                tracing::info!("Wi-Fi already on, skip");
            }
        }
    }

    if !wifi_found {
        tracing::warn!("No Wi-Fi radio found");
    }

    Ok(())
}
