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
    profile: &crate::config::Profile,
    args: &[String],
    skip_permissions: bool,
    auto_continue: bool,
    verbose: bool,
) -> Result<()> {
    let mut claude_args: Vec<String> = Vec::new();

    if skip_permissions {
        claude_args.push("--dangerously-skip-permissions".to_string());
    }

    if auto_continue && session::has_existing_session() {
        claude_args.push("--continue".to_string());
    }

    // Pass model via --model so it overrides any default model from
    // ~/.claude/settings.json. CLAUDE_MODEL env is unreliable — claude code
    // has its own default that wins unless --model is given.
    // Skip if the user already supplied --model in args.
    let user_set_model = args.iter().any(|a| a == "--model" || a.starts_with("--model="));
    if !user_set_model
        && let Some(model) = &profile.default_model
    {
        claude_args.push("--model".to_string());
        claude_args.push(model.clone());
    }

    claude_args.extend_from_slice(args);

    let mut cmd = Command::new(bin);
    cmd.args(&claude_args);

    // Always clear the Bedrock trigger vars — set back below if mode == Bedrock.
    cmd.env_remove("CLAUDE_CODE_USE_BEDROCK");
    cmd.env_remove("AWS_PROFILE");
    cmd.env_remove("AWS_REGION");
    cmd.env_remove("CLAUDE_MODEL");

    // For Local (MAX) mode, aggressively scrub any inherited provider state
    // that could divert `claude` away from the Claude MAX subscription.
    // Running from Claude Desktop injects a bunch of these — they need to go.
    if matches!(profile.mode, ProfileMode::Local) {
        let prefixes = ["AWS_", "ANTHROPIC_", "CLAUDE_CODE_PROVIDER_"];
        let inherited: Vec<String> = std::env::vars()
            .map(|(k, _)| k)
            .filter(|k| prefixes.iter().any(|p| k.starts_with(p)))
            .collect();
        for key in inherited {
            // Preserve OAuth — it's what authenticates MAX.
            if key == "CLAUDE_CODE_OAUTH_TOKEN" {
                continue;
            }
            cmd.env_remove(key);
        }
    }

    // Set custom environment variables from profile
    for (key, value) in &profile.env {
        cmd.env(key, value);
    }

    match &profile.mode {
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

    if verbose {
        // Mirror the final env that `claude` will see by computing it from the
        // parent env plus our modifications tracked in `cmd`.
        eprintln!("[clp] exec: {} {:?}", bin, claude_args);
        eprintln!("[clp] env (AWS_/ANTHROPIC_/CLAUDE_* only):");
        let mut final_env: std::collections::BTreeMap<String, Option<String>> =
            std::env::vars().map(|(k, v)| (k, Some(v))).collect();
        for (k, v) in cmd.get_envs() {
            let key = k.to_string_lossy().into_owned();
            match v {
                Some(val) => {
                    final_env.insert(key, Some(val.to_string_lossy().into_owned()));
                }
                None => {
                    final_env.insert(key, None);
                }
            }
        }
        for (k, v) in &final_env {
            if !(k.starts_with("AWS_")
                || k.starts_with("ANTHROPIC_")
                || k.starts_with("CLAUDE_"))
            {
                continue;
            }
            match v {
                Some(val) => {
                    // Truncate long tokens for readability
                    let display = if val.len() > 40 {
                        format!("{}…({} chars)", &val[..20], val.len())
                    } else {
                        val.clone()
                    };
                    eprintln!("  {}={}", k, display);
                }
                None => eprintln!("  {}=<removed>", k),
            }
        }
    }

    // exec() replaces the current process (Unix only)
    let err = cmd.exec();
    Err(anyhow!("Failed to exec '{}': {}", bin, err))
}
