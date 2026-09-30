use super::cli_bridge::{cli_bridge_status, publish_cli_bridge, CliBridgeHealth, CliBridgeStatus};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::Path;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CliManifest {
    pub version: String,
    pub source_commit: String,
    pub target: String,
    pub profile: String,
    pub asset_name: String,
    pub size: u64,
    pub sha256: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CliPreparationPhase {
    Preparing,
    Downloading,
    Verifying,
    Installing,
    Configuring,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliPreparationProgress {
    pub operation_id: String,
    pub phase: CliPreparationPhase,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
}
pub fn embedded_cli_manifest() -> Result<CliManifest> {
    let manifest: CliManifest =
        serde_json::from_str(option_env!("SKILLS_HUB_CLI_MANIFEST").unwrap_or(""))
            .map_err(|_| anyhow::anyhow!("CLI_MANIFEST_INVALID"))?;
    if manifest.version != env!("CARGO_PKG_VERSION")
        || manifest.profile
            != if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
    {
        bail!("CLI_MANIFEST_INVALID");
    }
    Ok(manifest)
}
pub fn prepare_cli(
    manifest: &CliManifest,
    destination: &Path,
    proxy_url: Option<&str>,
    on_progress: &dyn Fn(CliPreparationPhase, u64, Option<u64>),
) -> Result<CliBridgeStatus> {
    on_progress(CliPreparationPhase::Preparing, 0, Some(manifest.size));
    let current = cli_bridge_status(destination, &manifest.version, &manifest.sha256);
    if current.status == CliBridgeHealth::Valid {
        return Ok(current);
    }
    if cfg!(debug_assertions) {
        if manifest.profile != "debug" {
            bail!("CLI_MANIFEST_INVALID");
        }
        let source = option_env!("SKILLS_HUB_BUNDLED_CLI_SOURCE_PATH")
            .ok_or_else(|| anyhow::anyhow!("CLI_UNAVAILABLE"))?;
        on_progress(
            CliPreparationPhase::Installing,
            manifest.size,
            Some(manifest.size),
        );
        return publish_cli_bridge(
            Path::new(source),
            destination,
            &manifest.version,
            &manifest.sha256,
        );
    }
    if manifest.profile != "release" {
        bail!("CLI_MANIFEST_INVALID");
    }
    let url = format!(
        "https://github.com/qufei1993/skills-hub-cli/releases/download/v{}/{}",
        manifest.version, manifest.asset_name
    );
    let client = super::network_proxy::app_download_client(proxy_url, 120)
        .map_err(|_| anyhow::anyhow!("CLI_DOWNLOAD_FAILED"))?;
    download_and_publish(manifest, destination, &url, &client, on_progress)
}
fn download_and_publish(
    manifest: &CliManifest,
    destination: &Path,
    url: &str,
    client: &reqwest::blocking::Client,
    on_progress: &dyn Fn(CliPreparationPhase, u64, Option<u64>),
) -> Result<CliBridgeStatus> {
    let current = cli_bridge_status(destination, &manifest.version, &manifest.sha256);
    if current.status == CliBridgeHealth::Valid {
        return Ok(current);
    }
    let mut response = client
        .get(url)
        .send()
        .map_err(|_| anyhow::anyhow!("CLI_DOWNLOAD_FAILED"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        bail!("CLI_DOWNLOAD_UNAVAILABLE");
    }
    if !response.status().is_success() {
        bail!("CLI_DOWNLOAD_FAILED");
    }
    if response
        .content_length()
        .is_some_and(|length| length != manifest.size)
    {
        bail!("CLI_INTEGRITY_FAILED");
    }
    let temporary = tempfile::Builder::new()
        .prefix("skills-hub-cli-")
        .tempdir()
        .map_err(|_| anyhow::anyhow!("CLI_UNAVAILABLE"))?;
    let source = temporary.path().join("download");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&source)
        .map_err(|_| anyhow::anyhow!("CLI_UNAVAILABLE"))?;
    let mut hash = Sha256::new();
    let mut count = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    on_progress(CliPreparationPhase::Downloading, 0, Some(manifest.size));
    loop {
        let length = response
            .read(&mut buffer)
            .map_err(|_| anyhow::anyhow!("CLI_DOWNLOAD_FAILED"))?;
        if length == 0 {
            break;
        }
        count = count
            .checked_add(length as u64)
            .ok_or_else(|| anyhow::anyhow!("CLI_INTEGRITY_FAILED"))?;
        if count > manifest.size {
            bail!("CLI_INTEGRITY_FAILED");
        }
        file.write_all(&buffer[..length])
            .map_err(|_| anyhow::anyhow!("CLI_UNAVAILABLE"))?;
        hash.update(&buffer[..length]);
        on_progress(CliPreparationPhase::Downloading, count, Some(manifest.size));
    }
    on_progress(CliPreparationPhase::Verifying, count, Some(manifest.size));
    if count != manifest.size || hex::encode(hash.finalize()) != manifest.sha256 {
        bail!("CLI_INTEGRITY_FAILED");
    }
    file.sync_all()
        .map_err(|_| anyhow::anyhow!("CLI_UNAVAILABLE"))?;
    drop(file);
    on_progress(CliPreparationPhase::Installing, count, Some(manifest.size));
    publish_cli_bridge(&source, destination, &manifest.version, &manifest.sha256)
}
#[cfg(test)]
#[path = "tests/cli_distribution.rs"]
mod tests;
