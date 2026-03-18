mod aws;
mod claude;
mod commands;
mod config;
mod session;

use anyhow::Result;
use config::{Config, ProfileMode};

#[tokio::main]
async fn main() -> Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    let config = Config::load()?;

    // Check for special subcommands
    if let Some(first) = args.first() {
        match first.as_str() {
            "status" => return cmd_status(&config).await,
            "config" => return cmd_config(&config),
            _ => {}
        }
    }

    // Detect profile from first arg
    let profile_name = if let Some(first) = args.first() {
        if config.profiles.contains_key(first.as_str()) {
            let name = first.clone();
            args.remove(0);
            name
        } else {
            config.default_profile.clone()
        }
    } else {
        config.default_profile.clone()
    };

    let profile = config
        .get_profile(&profile_name)
        .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found in config", profile_name))?;

    println!("[clp] Profile: {}", profile_name);

    // For Bedrock profiles: ensure credentials are valid
    if let ProfileMode::Bedrock {
        aws_profile,
        aws_region,
    } = &profile.mode
    {
        let session = aws::AwsSession::new(aws_profile.clone(), aws_region.clone());

        if !session.credentials_valid().await {
            session.sso_login().await?;
        } else {
            println!("[clp] AWS credentials valid.");
        }
    }

    // Find and exec claude
    let bin = claude::find_claude_bin()?;
    claude::exec_claude(&bin, &profile.mode, &args)?;

    Ok(())
}

async fn cmd_status(config: &Config) -> Result<()> {
    println!("claude-profiles — status\n");
    println!("Default profile: {}\n", config.default_profile);

    for (name, profile) in &config.profiles {
        let marker = if name == &config.default_profile {
            "* "
        } else {
            "  "
        };

        match &profile.mode {
            ProfileMode::Local => {
                println!("{}[{}] mode=local (Claude MAX)", marker, name);
            }
            ProfileMode::Bedrock {
                aws_profile,
                aws_region,
            } => {
                let session = aws::AwsSession::new(aws_profile.clone(), aws_region.clone());
                let valid = session.credentials_valid().await;
                let status = if valid { "✓ valid" } else { "✗ expired" };
                println!(
                    "{}[{}] mode=bedrock profile={} region={} credentials={}",
                    marker, name, aws_profile, aws_region, status
                );
            }
        }
    }

    Ok(())
}

fn cmd_config(config: &Config) -> Result<()> {
    println!("Config path: {}", Config::path().display());
    println!("\nCurrent config:\n");
    println!("{}", toml::to_string_pretty(config)?);
    Ok(())
}
