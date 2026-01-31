//! Admin check and auto-elevation via gsudo (output stays in current terminal)

use std::env;
use std::process::Command;

#[cfg(windows)]
use windows::Win32::UI::Shell::IsUserAnAdmin;

/// If not admin, re-launch this process as admin via gsudo (output in current terminal),
/// wait for it to finish, then exit. If already admin, returns normally.
///
/// Requires [gsudo](https://github.com/gerardog/gsudo) installed (e.g. `winget install gsudo`).
pub fn ensure_admin_or_elevate() -> anyhow::Result<()> {
    unsafe {
        if IsUserAnAdmin().as_bool() {
            return Ok(());
        }
    }

    let exe =
        env::current_exe().map_err(|e| anyhow::anyhow!("Failed to get current exe path: {}", e))?;
    let args: Vec<String> = env::args().skip(1).collect();

    tracing::info!("Admin required, elevating via gsudo (output in current terminal)...");

    let status = Command::new("gsudo").arg(&exe).args(&args).status();

    match status {
            Ok(s) => std::process::exit(s.code().unwrap_or(1)),
            Err(e) => anyhow::bail!(
                "gsudo not found or failed: {}. Install gsudo (e.g. winget install gsudo) and retry, or run this program as administrator.",
                e
            ),
    }
}
