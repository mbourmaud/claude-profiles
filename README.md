# claude-profiles

Switch between Claude Code backends (MAX, Bedrock) with a single command.

`clp` is a lightweight wrapper around `claude` that manages profiles and AWS SSO authentication. Run `clp` instead of `claude` — it picks the right backend, handles credentials, and launches Claude Code.

## Quick start

Paste this into Claude Code and let it do everything for you:

```
Install claude-profiles (clp) from https://github.com/mbourmaud/claude-profiles — run the install script, then help me configure it with my AWS SSO profile for Bedrock and a local profile for Claude MAX.
```

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/mbourmaud/claude-profiles/main/install.sh | bash
```

This downloads the latest release binary for your platform and puts it in `~/.local/bin`.

## Usage

```bash
# Launch with default profile
clp

# Launch with a specific profile
clp -p claude.bedrock

# Set a new default profile
clp --default claude.bedrock

# Show config and credential status
clp status

# Interactively configure settings
clp configure
```

## Configuration

Config lives at `~/.config/claude-profiles/config.toml` (auto-created on first run).

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
```

### Top-level options

| Option | Default | Description |
|--------|---------|-------------|
| `default_profile` | — | Profile used when none is specified |
| `skip_permissions` | `false` | Skip Claude Code permission prompts (`--dangerously-skip-permissions`) |
| `auto_continue` | `false` | Automatically continue after tool use (`--continue`) |

### Profile modes

| Mode | Description |
|------|-------------|
| `local` | Uses your Claude MAX subscription (no extra config) |
| `bedrock` | Uses AWS Bedrock via SSO. Set `aws_profile` and `aws_region`. |

For Bedrock profiles, your `~/.aws/config` must have a matching SSO profile:

```ini
[profile bedrock]
sso_start_url = https://your-org.awsapps.com/start
sso_region = us-east-1
sso_account_id = 123456789012
sso_role_name = YourRole
region = us-east-2
```

## How it works

1. Reads your config to determine which profile to use
2. For Bedrock profiles: checks if AWS credentials are valid, runs SSO login if expired
3. Sets the right environment variables (`CLAUDE_CODE_USE_BEDROCK`, `AWS_PROFILE`, `AWS_REGION`)
4. Execs `claude` with your arguments

## Uninstall

```bash
rm ~/.local/bin/clp
rm -rf ~/.config/claude-profiles
```

## License

MIT
