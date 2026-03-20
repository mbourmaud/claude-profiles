use anyhow::Result;
use dialoguer::{Confirm, Select};
use crate::aws;
use crate::config::{Config, ProfileMode, UpdateCheck};

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

        match &profile.mode {
            ProfileMode::Local => {
                println!("  {} [{}]  mode=local (Claude MAX)", marker, name);
            }
            ProfileMode::Bedrock {
                aws_profile,
                aws_region,
            } => {
                let session = aws::AwsSession::new(aws_profile.clone(), aws_region.clone());
                let valid = session.credentials_valid().await;
                let status = if valid { "✓ valid" } else { "✗ expired" };
                println!(
                    "  {} [{}]  mode=bedrock profile={} region={} credentials={}",
                    marker, name, aws_profile, aws_region, status
                );
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

    Ok(())
}
