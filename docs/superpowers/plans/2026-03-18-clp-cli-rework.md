# CLP CLI Rework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rework `clp` to use clap for argument parsing, add configurable `skip_permissions` and `auto_continue` settings, interactive `clp configure`, and unified `clp status`.

**Architecture:** Replace hand-rolled arg parsing with clap derive macros. Add two boolean fields to `Config`. New `session` module handles session detection. New `commands` module holds subcommand implementations. `dialoguer` provides interactive prompts for `configure`.

**Tech Stack:** Rust, clap (derive), dialoguer, serde, toml

---

## File Structure

| File | Responsibility |
|------|---------------|
| `Cargo.toml` | Add `clap` and `dialoguer` dependencies |
| `src/main.rs` | Clap CLI definition, dispatch to subcommands or launch flow |
| `src/config.rs` | Config struct with new `skip_permissions`/`auto_continue` fields |
| `src/claude.rs` | `exec_claude` extended with flag prepending |
| `src/session.rs` | Session detection logic (new file) |
| `src/commands.rs` | `status` and `configure` subcommand implementations (new file) |
| `config.example.toml` | Updated with new fields |

---

### Task 1: Add dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add clap and dialoguer to Cargo.toml**

Add under `[dependencies]`:

```toml
# CLI
clap = { version = "4", features = ["derive"] }
dialoguer = "0.11"
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors (new deps are unused but that's fine)

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "deps: add clap and dialoguer"
```

---

### Task 2: Add new config fields

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Add `skip_permissions` and `auto_continue` fields to Config**

In the `Config` struct, add two fields with serde defaults:

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub default_profile: String,
    #[serde(default)]
    pub skip_permissions: bool,
    #[serde(default)]
    pub auto_continue: bool,
    pub profiles: HashMap<String, Profile>,
}
```

- [ ] **Step 2: Update the `Default` impl**

Add the two new fields to the `Default` implementation:

```rust
Self {
    default_profile: "claude.max".to_string(),
    skip_permissions: false,
    auto_continue: false,
    profiles,
}
```

- [ ] **Step 3: Add a helper to list profile names**

Add a method to `Config` that returns sorted profile names (needed by `configure` and error messages):

```rust
pub fn profile_names(&self) -> Vec<&str> {
    let mut names: Vec<&str> = self.profiles.keys().map(|s| s.as_str()).collect();
    names.sort();
    names
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check`
Expected: compiles (existing code still works, new fields default to false for existing configs)

- [ ] **Step 5: Commit**

```bash
git add src/config.rs
git commit -m "feat: add skip_permissions and auto_continue config fields"
```

---

### Task 3: Add session detection module

**Files:**
- Create: `src/session.rs`

- [ ] **Step 1: Create `src/session.rs` with `has_existing_session`**

```rust
use std::env;
use std::path::PathBuf;

/// Check if a Claude session exists for the current working directory.
///
/// Claude stores sessions in `~/.claude/projects/<project-key>/sessions-index.json`
/// where `<project-key>` is the absolute working directory path with `/` replaced by `-`.
pub fn has_existing_session() -> bool {
    let cwd = match env::current_dir() {
        Ok(p) => p,
        Err(_) => return false,
    };

    let project_key = cwd
        .to_string_lossy()
        .replace('/', "-");

    let session_index = claude_projects_dir().join(&project_key).join("sessions-index.json");

    match std::fs::metadata(&session_index) {
        Ok(meta) => meta.len() > 0,
        Err(_) => false,
    }
}

fn claude_projects_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("projects")
}
```

- [ ] **Step 2: Register the module in `main.rs`**

Add `mod session;` to the top of `src/main.rs` (alongside the existing `mod` declarations).

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: compiles (module registered, function unused for now)

- [ ] **Step 4: Commit**

```bash
git add src/session.rs src/main.rs
git commit -m "feat: add session detection for auto-continue"
```

---

### Task 4: Extend `exec_claude` with flag prepending

**Files:**
- Modify: `src/claude.rs`

- [ ] **Step 1: Update `exec_claude` signature and implementation**

Change `exec_claude` to accept the new flags and prepend them:

```rust
use crate::session;

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
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: error in `main.rs` because the call site still uses old signature — that's expected, we fix it in Task 6.

- [ ] **Step 3: Commit**

```bash
git add src/claude.rs
git commit -m "feat: extend exec_claude with skip_permissions and auto_continue"
```

---

### Task 5: Add commands module (status + configure)

**Files:**
- Create: `src/commands.rs`

- [ ] **Step 1: Create `src/commands.rs` with `cmd_status`**

```rust
use anyhow::Result;
use crate::aws;
use crate::config::{Config, ProfileMode};

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

    println!("\nProfiles:");
    let mut names = config.profile_names();
    // Ensure deterministic ordering
    names.sort();

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
```

- [ ] **Step 2: Add `cmd_configure` to the same file**

```rust
use dialoguer::{Confirm, Select};

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

    config.default_profile = names[default_idx].clone();
    config.skip_permissions = skip_permissions;
    config.auto_continue = auto_continue;
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

    Ok(())
}
```

- [ ] **Step 3: Register the module in `main.rs`**

Add `mod commands;` to the top of `src/main.rs`.

- [ ] **Step 4: Verify it compiles**

Run: `cargo check`
Expected: compiles (functions unused for now)

- [ ] **Step 5: Commit**

```bash
git add src/commands.rs src/main.rs
git commit -m "feat: add status and configure commands"
```

---

### Task 6: Rewrite `main.rs` with clap

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Replace the entire contents of `main.rs`**

```rust
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
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

- [ ] **Step 3: Test basic CLI behavior**

Run these commands and verify output:

```bash
cargo run -- --help
# Expected: shows help with --profile, --default, status, configure

cargo run -- status
# Expected: shows version, config path, settings, profiles

cargo run -- --default claude.max
# Expected: "Default profile set to 'claude.max'"

cargo run -- --default nonexistent
# Expected: error with available profile names
```

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: rewrite main.rs with clap CLI structure"
```

---

### Task 7: Update config.example.toml

**Files:**
- Modify: `config.example.toml`

- [ ] **Step 1: Update the example config**

```toml
default_profile = "claude.max"
skip_permissions = false
auto_continue = false

[profiles."claude.max"]
mode = "local"

[profiles."claude.bedrock"]
mode = "bedrock"
aws_profile = "bedrock"
aws_region = "us-east-2"

# Example: additional Bedrock profiles
# [profiles."claude.bedrock-prod"]
# mode = "bedrock"
# aws_profile = "my-sso-profile"
# aws_region = "eu-west-1"
```

- [ ] **Step 2: Commit**

```bash
git add config.example.toml
git commit -m "docs: update example config with new settings"
```

---

### Task 8: Update README.md

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Read current README**

Read `README.md` to understand current structure.

- [ ] **Step 2: Update usage section**

Update the usage section to reflect the new CLI interface:

- Replace positional profile usage (`clp profile.name`) with `clp -p profile.name`
- Add `--default` usage
- Replace `clp status` / `clp config` with unified `clp status`
- Add `clp configure` documentation
- Document `skip_permissions` and `auto_continue` config options

- [ ] **Step 3: Verify no stale references to old commands**

Search for `clp config` or positional profile references and remove/update them.

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: update README for new CLI interface"
```

---

### Task 9: Final integration test

- [ ] **Step 1: Build release binary**

Run: `cargo build --release`
Expected: compiles with no errors or warnings

- [ ] **Step 2: Test all commands end-to-end**

```bash
# Help
./target/release/clp --help

# Status
./target/release/clp status

# Configure (interactive — walk through prompts)
./target/release/clp configure

# Set default
./target/release/clp --default claude.max

# Error case
./target/release/clp -p nonexistent
```

Verify each command works as expected.

- [ ] **Step 3: Commit any fixes**

If any issues found, fix and commit.
