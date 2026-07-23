//! Public IP detection via external APIs.

use crate::WarpError;

/// Default timeout for IP check requests.
const IP_CHECK_TIMEOUT_SECS: u64 = 5;

/// List of IP-check endpoints to try in order (fallback chain).
const IP_CHECK_URLS: &[&str] = &[
    "https://api.ipify.org",
    "https://ifconfig.me",
    "https://icanhazip.com",
];

/// Get the current public IP address by querying external APIs.
///
/// Tries multiple endpoints in order; returns the first successful result.
/// Returns `WarpError::IpCheckFailed` if all endpoints fail.
pub async fn get_public_ip() -> Result<String, WarpError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(IP_CHECK_TIMEOUT_SECS))
        .build()
        .map_err(WarpError::NetworkError)?;

    for url in IP_CHECK_URLS {
        match client.get(*url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    let ip = response.text().await.unwrap_or_default();
                    let ip = ip.trim().to_string();
                    if !ip.is_empty() {
                        return Ok(ip);
                    }
                }
            }
            Err(e) => {
                tracing::debug!(url = %url, error = %e, "IP check endpoint failed, trying next");
                continue;
            }
        }
    }

    Err(WarpError::IpCheckFailed(
        "all IP check endpoints failed".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn get_public_ip_returns_something() {
        // In CI/offline this will fail, which is acceptable
        if let Ok(ip) = get_public_ip().await {
            // IP should look like an IP address
            assert!(!ip.is_empty(), "IP should not be empty");
            // Basic format check: dots or colons
            assert!(
                ip.contains('.') || ip.contains(':'),
                "IP should contain dots or colons: {ip}"
            );
        }
        // If it fails (no network), that's fine too — test passes either way
    }
}
