//! Network Watchdog: auto-recover by connecting to saved Wi-Fi when network is down.

mod adapter;
mod admin;
mod network;
mod radio;
mod wlan;

use std::sync::Arc;

use clap::Parser;
use tokio::time::{sleep, Duration};
use wlan::ConnectStrategy;

#[derive(Parser, Debug)]
#[command(
    name = "network-watchdog",
    about = "Auto-recover network by connecting to saved Wi-Fi when down",
    long_about = "Periodically checks network (NCSI). If unreachable, turns on Wi-Fi radio and tries saved Wi-Fi profiles until restored or all tried."
)]
struct Cli {
    /// Run once: check network once, try recovery once if down, then exit (no loop)
    #[arg(long, short = '1', alias = "single")]
    pub once: bool,

    /// Check interval in seconds
    #[arg(long, default_value_t = 60)]
    pub interval: u64,

    /// NCSI probe URL
    #[arg(long, default_value = network::DEFAULT_NCSI_URL)]
    pub ncsi_url: String,

    /// NCSI request timeout in seconds
    #[arg(long, default_value_t = network::DEFAULT_NCSI_TIMEOUT_SECS)]
    pub ncsi_timeout: u64,

    /// Try all saved Wi-Fi profiles (no \"visible only\" filter; default is scan-only)
    #[arg(long)]
    pub all: bool,

    /// Only try these saved profile names; multiple or comma-separated
    /// e.g. --profiles Home --profiles Office or --profiles "Home,Office"
    #[arg(long, value_delimiter(','), num_args = 1..)]
    pub profiles: Option<Vec<String>>,
}

impl Cli {
    fn connect_strategy(&self) -> ConnectStrategy {
        if let Some(ref names) = self.profiles {
            if !names.is_empty() {
                return ConnectStrategy::Explicit(names.clone());
            }
        }
        if self.all {
            return ConnectStrategy::All;
        }
        ConnectStrategy::ScanOnly
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    admin::ensure_admin_or_elevate()?;
    let strategy = cli.connect_strategy();

    tracing::info!(
        "Network Watchdog started, strategy: {:?}, mode: {}",
        strategy,
        if cli.once { "single run" } else { "loop" }
    );
    if !cli.once {
        tracing::info!("Checking network every {} s", cli.interval);
    }

    let ncsi_url: Arc<str> = Arc::from(cli.ncsi_url.as_str());
    let ncsi_timeout = cli.ncsi_timeout;
    let check_interval = cli.interval;

    loop {
        tracing::info!("Checking network...");
        if network::test_network(&ncsi_url, ncsi_timeout).await {
            tracing::info!("Network OK");
            if cli.once {
                tracing::info!("--once mode, exiting");
                return Ok(());
            }
            tracing::info!("Sleeping {} s...", check_interval);
            sleep(Duration::from_secs(check_interval)).await;
            continue;
        }

        tracing::warn!("Network unreachable, attempting Wi-Fi recovery");

        tracing::info!("Step 1/2: Turn on Wi-Fi radio");
        if let Err(e) = radio::turn_on_wifi_radio().await {
            tracing::warn!(
                "Failed to turn on Wi-Fi radio: {} (continuing with saved profiles)",
                e
            );
        } else {
            tracing::info!("Wi-Fi radio ready");
        }

        tracing::info!(
            "Step 2/2: Enumerate and connect saved Wi-Fi profiles (filtered by strategy)"
        );
        let url = Arc::clone(&ncsi_url);
        let timeout = ncsi_timeout;
        let result = wlan::connect_any_saved_wifi(
            move || {
                let u = Arc::clone(&url);
                Box::pin(async move { network::test_network(&u, timeout).await })
            },
            strategy.clone(),
        )
        .await;

        match result {
            Ok(()) => {
                tracing::info!("Network restored");
            }
            Err(e) => {
                tracing::warn!("Recovery failed this round: {}", e);
            }
        }

        if cli.once {
            tracing::info!("--once mode, exiting after one run");
            return Ok(());
        }
        tracing::info!("Sleeping {} s...", check_interval);
        sleep(Duration::from_secs(check_interval)).await;
    }
}
