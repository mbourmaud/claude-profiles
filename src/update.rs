use std::env;
use std::fs;
use std::io::Read;

use anyhow::{anyhow, Context, Result};
use flate2::read::GzDecoder;
use semver::Version;

const GITHUB_API_URL: &str =
    "https://api.github.com/repos/mbourmaud/claude-profiles/releases/latest";

pub struct ReleaseInfo {
    pub version: Version,
    pub asset_url: String,
}

/// Check GitHub for a newer release. Returns `None` if current is up-to-date.
pub async fn check_for_update() -> Option<ReleaseInfo> {
    let current = Version::parse(env!("CARGO_PKG_VERSION")).ok()?;

    let client = reqwest::Client::builder()
        .user_agent("clp-updater")
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    let resp: serde_json::Value = client
        .get(GITHUB_API_URL)
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let tag = resp.get("tag_name")?.as_str()?;
    let latest = Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok()?;

    if latest <= current {
        return None;
    }

    let asset_name = platform_asset_name()?;
    let assets = resp.get("assets")?.as_array()?;
    let asset_url = assets.iter().find_map(|a| {
        let name = a.get("name")?.as_str()?;
        if name == asset_name {
            a.get("browser_download_url")?.as_str().map(|s| s.to_string())
        } else {
            None
        }
    })?;

    Some(ReleaseInfo {
        version: latest,
        asset_url,
    })
}

/// Download the release tarball and replace the current binary.
pub async fn self_update(release: &ReleaseInfo) -> Result<()> {
    let client = reqwest::Client::builder()
        .user_agent("clp-updater")
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let bytes = client
        .get(&release.asset_url)
        .send()
        .await
        .context("Failed to download update")?
        .bytes()
        .await
        .context("Failed to read update body")?;

    let gz = GzDecoder::new(&bytes[..]);
    let mut archive = tar::Archive::new(gz);

    let mut new_binary: Option<Vec<u8>> = None;
    for entry in archive.entries().context("Failed to read tarball")? {
        let mut entry = entry?;
        let path = entry.path()?;
        if path.file_name().and_then(|n| n.to_str()) == Some("clp") {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            new_binary = Some(buf);
            break;
        }
    }

    let new_binary = new_binary.ok_or_else(|| anyhow!("'clp' binary not found in tarball"))?;

    let current_exe = env::current_exe().context("Cannot determine current executable path")?;

    // Write to a temp file next to the current binary, then rename (atomic on same filesystem)
    let tmp_path = current_exe.with_extension("update-tmp");
    fs::write(&tmp_path, &new_binary).context("Failed to write temporary update file")?;

    // Preserve executable permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o755))?;
    }

    fs::rename(&tmp_path, &current_exe).context("Failed to replace binary")?;

    Ok(())
}

fn platform_asset_name() -> Option<&'static str> {
    match (env::consts::OS, env::consts::ARCH) {
        ("macos", "aarch64") => Some("clp-macos-aarch64.tar.gz"),
        ("macos", "x86_64") => Some("clp-macos-x86_64.tar.gz"),
        ("linux", "aarch64") => Some("clp-linux-aarch64.tar.gz"),
        ("linux", "x86_64") => Some("clp-linux-x86_64.tar.gz"),
        _ => None,
    }
}
