use anyhow::Result;
use dialoguer::{Confirm, Input, Select};
use crate::aws;
use crate::config::{Config, ProfileMode, UpdateCheck};

/// Check if an environment variable name might contain sensitive information
fn is_sensitive_env_var(key: &str) -> bool {
    let key_lower = key.to_lowercase();
    key_lower.contains("key") ||
    key_lower.contains("secret") ||
    key_lower.contains("token") ||
    key_lower.contains("password") ||
    key_lower.contains("pass") ||
    key_lower.contains("credential") ||
    key_lower.contains("auth")
}

pub async fn cmd_status(config: &Config) -> Result<()> {
    println!("claude-profiles v{}\n", env!("CARGO_PKG_VERSION"));
    println!("Config: {}\n", Config::path().display());

    println!("Settings:");
    println!(
        "  skip_permissions: {}",
        if config.skip_permissions { "on" } else { "off" }
    );
    println!(
        "  auto_continue:    {}",
        if config.auto_continue { "on" } else { "off" }
    );
    println!("  update_check:     {}", config.update_check);

    println!("\nProfiles:");
    let names = config.profile_names();

    for name in &names {
        let profile = config.get_profile(name).unwrap();
        let marker = if *name == config.default_profile {
            "*"
        } else {
            " "
        };

        let model_info = profile
            .default_model
            .as_ref()
            .map(|m| format!(" model={}", m))
            .unwrap_or_default();

        match &profile.mode {
            ProfileMode::Local => {
                println!("  {} [{}]  mode=local (Claude MAX){}", marker, name, model_info);
            }
            ProfileMode::Bedrock {
                aws_profile,
                aws_region,
            } => {
                let session = aws::AwsSession::new(aws_profile.clone(), aws_region.clone());
                let valid = session.credentials_valid().await;
                let status = if valid { "✓ valid" } else { "✗ expired" };
                println!(
                    "  {} [{}]  mode=bedrock profile={} region={} credentials={}{}",
                    marker, name, aws_profile, aws_region, status, model_info
                );
            }
        }

        // Show environment variables (non-sensitive config)
        if !profile.env.is_empty() {
            println!("           env:");
            let mut env_vars: Vec<_> = profile.env.iter().collect();
            env_vars.sort_by_key(|(k, _)| *k);

            for (key, value) in env_vars {
                // Skip displaying values that might be sensitive
                let display_value = if is_sensitive_env_var(key) {
                    "[hidden]".to_string()
                } else {
                    value.clone()
                };
                println!("             {}={}", key, display_value);
            }
        }
    }

    Ok(())
}

pub fn cmd_configure(config: &mut Config) -> Result<()> {
    let names = config.profile_names().into_iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let current_default_idx = names
        .iter()
        .position(|n| n == &config.default_profile)
        .unwrap_or(0);

    let default_idx = Select::new()
        .with_prompt("Default profile")
        .items(&names)
        .default(current_default_idx)
        .interact_opt()?;

    // If user hit Ctrl+C / Esc, abort without saving
    let Some(default_idx) = default_idx else {
        println!("Cancelled. No changes saved.");
        return Ok(());
    };

    let skip_permissions = Confirm::new()
        .with_prompt("Automatically skip permission checks?")
        .default(config.skip_permissions)
        .interact_opt()?;

    let Some(skip_permissions) = skip_permissions else {
        println!("Cancelled. No changes saved.");
        return Ok(());
    };

    let auto_continue = Confirm::new()
        .with_prompt("Automatically continue previous session if one exists?")
        .default(config.auto_continue)
        .interact_opt()?;

    let Some(auto_continue) = auto_continue else {
        println!("Cancelled. No changes saved.");
        return Ok(());
    };

    let update_options = ["notify", "auto", "off"];
    let current_update_idx = match config.update_check {
        UpdateCheck::Notify => 0,
        UpdateCheck::Auto => 1,
        UpdateCheck::Off => 2,
    };

    let update_idx = Select::new()
        .with_prompt("Check for updates (notify = show message, auto = self-update, off = skip)")
        .items(&update_options)
        .default(current_update_idx)
        .interact_opt()?;

    let Some(update_idx) = update_idx else {
        println!("Cancelled. No changes saved.");
        return Ok(());
    };

    let update_check = match update_idx {
        0 => UpdateCheck::Notify,
        1 => UpdateCheck::Auto,
        _ => UpdateCheck::Off,
    };

    // Configure default model for the selected profile
    let profile_name = &names[default_idx];
    let profile = config.profiles.get_mut(profile_name).unwrap();

    let current_model = profile.default_model.clone().unwrap_or_default();
    let new_model: String = Input::new()
        .with_prompt("Default Claude model (leave empty for Claude Code default, or use 'anthropic.claude-sonnet-4-6')")
        .default(current_model)
        .allow_empty(true)
        .interact_text()?;

    profile.default_model = if new_model.is_empty() {
        None
    } else {
        Some(new_model)
    };

    config.default_profile = names[default_idx].clone();
    config.skip_permissions = skip_permissions;
    config.auto_continue = auto_continue;
    config.update_check = update_check;
    config.save()?;

    println!("\nSettings saved:");
    println!("  default_profile:  {}", config.default_profile);
    println!(
        "  skip_permissions: {}",
        if config.skip_permissions { "on" } else { "off" }
    );
    println!(
        "  auto_continue:    {}",
        if config.auto_continue { "on" } else { "off" }
    );
    println!("  update_check:     {}", config.update_check);

    // Show the model configuration for the default profile
    if let Some(profile) = config.get_profile(&config.default_profile) {
        if let Some(model) = &profile.default_model {
            println!("  default_model:    {}", model);
        } else {
            println!("  default_model:    (using Claude Code default)");
        }
    }

    Ok(())
}
