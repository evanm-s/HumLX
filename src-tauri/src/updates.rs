//! When the updater says only "Could not fetch a valid release JSON", this asks
//! the update endpoint again so the app can say what actually went wrong.

use std::time::Duration;

const ENDPOINT: &str = "https://github.com/evanm-s/HumLX/releases/latest/download/latest.json";

/// Fetches the update manifest and describes the outcome in a few words.
#[tauri::command]
pub async fn probe_update_endpoint() -> String {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    let client = match reqwest::Client::builder()
        .user_agent(concat!("HumLX/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(20))
        .build()
    {
        Ok(client) => client,
        Err(error) => return format!("couldn't start a request ({error})"),
    };
    match client.get(ENDPOINT).send().await {
        Ok(response) => {
            let status = response.status();
            let host = response.url().host_str().unwrap_or("?").to_string();
            if !status.is_success() {
                return format!("{host} answered {status}");
            }
            match response.text().await {
                Ok(body) if serde_json::from_str::<serde_json::Value>(&body).is_ok() => {
                    "GitHub answered fine on a second try".to_string()
                }
                Ok(_) => format!("{host} sent something that isn't update info"),
                Err(error) => format!("download from {host} broke off ({error})"),
            }
        }
        Err(error) => {
            let mut text = error.to_string();
            let mut source = std::error::Error::source(&error);
            while let Some(inner) = source {
                text = format!("{text}: {inner}");
                source = inner.source();
            }
            format!("no answer from GitHub ({text})")
        }
    }
}
