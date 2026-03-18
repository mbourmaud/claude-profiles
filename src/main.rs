mod aws;
mod claude;
mod commands;
mod config;
mod session;

use anyhow::Result;
use clap::{Parser, Subcommand};
use config::Config;

#[derive(Parser)]
#[command(name = "clp", about = "Claude profile manager", version)]
struct Cli {
    /// Select a profile for this session
    #[arg(short, long)]
    profile: Option<String>,

    /// Set the default profile and exit
    #[arg(long)]
    default: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,

    /// Extra arguments passed to claude (after --)
    #[arg(last = true)]
    claude_args: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show config, settings, and credential status
    Status,
    /// Interactively configure global settings
    Configure,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut config = Config::load()?;

    // Handle subcommands
    if let Some(command) = cli.command {
        return match command {
            Commands::Status => commands::cmd_status(&config).await,
            Commands::Configure => commands::cmd_configure(&mut config),
        };
    }

    // Handle --default: set and exit
    if let Some(ref name) = cli.default {
        if config.get_profile(name).is_none() {
            let available = config.profile_names().join(", ");
            anyhow::bail!(
                "Profile '{}' not found. Available profiles: {}",
                name,
                available
            );
        }
        config.default_profile = name.clone();
        config.save()?;
        println!("[clp] Default profile set to '{}'", name);
        return Ok(());
    }

    // Determine which profile to use
    let profile_name = cli.profile.unwrap_or_else(|| config.default_profile.clone());

    let profile = config.get_profile(&profile_name).ok_or_else(|| {
        let available = config.profile_names().join(", ");
        anyhow::anyhow!(
            "Profile '{}' not found. Available profiles: {}",
            profile_name,
            available
        )
    })?;

    println!("[clp] Profile: {}", profile_name);

    // For Bedrock profiles: ensure credentials are valid
    if let config::ProfileMode::Bedrock {
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
    claude::exec_claude(
        &bin,
        &profile.mode,
        &cli.claude_args,
        config.skip_permissions,
        config.auto_continue,
    )?;

    Ok(())
}
