use reqwest::header::{ACCEPT, AUTHORIZATION, ETAG, IF_NONE_MATCH, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::db::models::SyncSnapshot;

pub const SYNC_GIST_FILENAME: &str = "abdullah_masar_v6_sync.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct PullResult {
    pub not_modified: bool,
    pub etag: Option<String>,
    pub snapshot: Option<SyncSnapshot>,
}

/// Pulls Private GitHub Gist using ETag / If-None-Match conditional request.
pub async fn pull_gist(
    pat: &str,
    gist_id: &str,
    last_etag: Option<&str>,
) -> Result<PullResult, String> {
    let client = reqwest::Client::new();
    let url = format!("https://api.github.com/gists/{}", gist_id.trim());

    let mut req = client
        .get(&url)
        .header(AUTHORIZATION, format!("Bearer {}", pat.trim()))
        .header(ACCEPT, "application/vnd.github+json")
        .header(USER_AGENT, "abdullah-masar-v6/6.0");

    if let Some(etag) = last_etag {
        if !etag.is_empty() {
            req = req.header(IF_NONE_MATCH, etag);
        }
    }

    let resp = req.send().await.map_err(|e| format!("Network error: {}", e))?;
    if resp.status().as_u16() == 304 {
        return Ok(PullResult {
            not_modified: true,
            etag: last_etag.map(|s| s.to_string()),
            snapshot: None,
        });
    }
    if !resp.status().is_success() {
        return Err(format!("GitHub Gist GET failed: HTTP {}", resp.status()));
    }

    let new_etag = resp
        .headers()
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let content = body
        .get("files")
        .and_then(|f| f.get(SYNC_GIST_FILENAME))
        .and_then(|f| f.get("content"))
        .and_then(|c| c.as_str());

    let snapshot = match content {
        Some(raw) if !raw.trim().is_empty() => {
            Some(serde_json::from_str::<SyncSnapshot>(raw).map_err(|e| e.to_string())?)
        }
        _ => None,
    };

    Ok(PullResult {
        not_modified: false,
        etag: new_etag,
        snapshot,
    })
}

/// Pushes merged canonical V6 SyncSnapshot back to Private GitHub Gist.
pub async fn push_gist(
    pat: &str,
    gist_id: &str,
    snapshot: &SyncSnapshot,
) -> Result<Option<String>, String> {
    let client = reqwest::Client::new();
    let url = format!("https://api.github.com/gists/{}", gist_id.trim());
    let serialized = serde_json::to_string_pretty(snapshot).map_err(|e| e.to_string())?;

    let payload = json!({
        "files": {
            SYNC_GIST_FILENAME: {
                "content": serialized
            }
        }
    });

    let resp = client
        .patch(&url)
        .header(AUTHORIZATION, format!("Bearer {}", pat.trim()))
        .header(ACCEPT, "application/vnd.github+json")
        .header(USER_AGENT, "abdullah-masar-v6/6.0")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub Gist PATCH failed: HTTP {}", resp.status()));
    }

    Ok(resp
        .headers()
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string()))
}
