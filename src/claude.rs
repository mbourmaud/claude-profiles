use std::env;
use std::os::unix::process::CommandExt;
use std::process::Command;

use anyhow::{anyhow, Result};

use crate::config::ProfileMode;
use crate::session;

/// Find the real `claude` binary, skipping ourselves to avoid infinite loops.
pub fn find_claude_bin() -> Result<String> {
    let self_path = env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(|s| s.to_string()))
        .unwrap_or_default();

    for dir in env::var("PATH").unwrap_or_default().split(':') {
        let candidate = format!("{}/claude", dir);
        if candidate == self_path {
            continue;
        }
        if std::path::Path::new(&candidate).exists() {
            return Ok(candidate);
        }
    }

    Err(anyhow!(
        "Could not find 'claude' binary in PATH (excluding self)"
    ))
}

/// Apply environment variables for the given profile mode, then exec claude.
pub fn exec_claude(
    bin: &str,
    mode: &ProfileMode,
    args: &[String],
    skip_permissions: bool,
    auto_continue: bool,
) -> Result<()> {
    let mut claude_args: Vec<String> = Vec::new();

    if skip_permissions {
        claude_args.push("--dangerously-skip-permissions".to_string());
    }

    if auto_continue && session::has_existing_session() {
        claude_args.push("--continue".to_string());
    }

    claude_args.extend_from_slice(args);

    let mut cmd = Command::new(bin);
    cmd.args(&claude_args);

    // Clear Bedrock env vars first, then set if needed
    cmd.env_remove("CLAUDE_CODE_USE_BEDROCK");
    cmd.env_remove("AWS_PROFILE");
    cmd.env_remove("AWS_REGION");

    match mode {
        ProfileMode::Local => {
            // Nothing extra — use personal Claude MAX subscription
        }
        ProfileMode::Bedrock {
            aws_profile,
            aws_region,
        } => {
            cmd.env("CLAUDE_CODE_USE_BEDROCK", "1");
            cmd.env("AWS_PROFILE", aws_profile);
            cmd.env("AWS_REGION", aws_region);
        }
    }

    // exec() replaces the current process (Unix only)
    let err = cmd.exec();
    Err(anyhow!("Failed to exec '{}': {}", bin, err))
}
