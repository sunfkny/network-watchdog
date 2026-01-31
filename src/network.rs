//! NCSI network probe

/// Default NCSI URL (Windows NCSI endpoint)
pub const DEFAULT_NCSI_URL: &str = "http://www.msftconnecttest.com/connecttest.txt";

/// Default NCSI request timeout in seconds
pub const DEFAULT_NCSI_TIMEOUT_SECS: u64 = 5;

/// Probe network reachability using the given NCSI endpoint
pub async fn test_network(url: &str, timeout_secs: u64) -> bool {
    tracing::debug!("Requesting NCSI: {} (timeout {} s)", url, timeout_secs);
    let client = reqwest::Client::new();
    let result = client
        .get(url)
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);
    if result {
        tracing::debug!("NCSI probe: OK");
    } else {
        tracing::debug!("NCSI probe: failed or timeout");
    }
    result
}
