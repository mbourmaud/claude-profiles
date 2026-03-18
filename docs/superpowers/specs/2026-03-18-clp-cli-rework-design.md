# CLP CLI Rework Design

## Overview

Rework the `clp` CLI to use structured argument parsing (clap), add configurable global settings for `--dangerously-skip-permissions` and `--continue`, introduce an interactive `clp configure` command, and unify `status`/`config` into a single `clp status` command.

## CLI Structure

```
clp [OPTIONS] [-- claude-args...]
clp status
clp configure
```

### Flags

- `-p, --profile <name>` — select profile for this session (replaces positional profile selection)
- `--default <name>` — set the default profile in config and exit

### Subcommands

- `status` — unified view of config settings, profiles, and credential status
- `configure` — interactive TUI to toggle global settings

Arguments after `--` are passed through to `claude` as-is.

## Config Changes

### New Fields

Two new optional boolean fields at the top level of `config.toml`:

```toml
default_profile = "claude.max"
skip_permissions = false
auto_continue = true

[profiles."claude.max"]
mode = "local"

[profiles."claude.bedrock"]
mode = "bedrock"
aws_profile = "bedrock"
aws_region = "us-east-2"
```

- `skip_permissions` (bool, default `false`) — when true, passes `--dangerously-skip-permissions` to claude
- `auto_continue` (bool, default `false`) — when true, checks for existing session and passes `--continue` if one exists

Both use `#[serde(default)]` so existing configs remain compatible.

## Session Detection for Auto-Continue

To decide whether to pass `--continue`:

1. Derive the project key from the current working directory by replacing `/` with `-` (e.g. `/Users/fr162241/Projects/foo` becomes `-Users-fr162241-Projects-foo`)
2. Check for `~/.claude/projects/<project-key>/sessions-index.json`
3. If the file exists and is non-empty, pass `--continue` to claude

Simple file existence check. No parsing of session contents.

## `clp configure` — Interactive Setup

Uses the `dialoguer` crate for interactive prompts:

1. **Default profile** — `Select` prompt listing all profile names, pre-selecting the current default
2. **Skip permissions** — `Confirm` prompt ("Automatically skip permission checks?"), pre-filled with current value
3. **Auto-continue** — `Confirm` prompt ("Automatically continue previous session if one exists?"), pre-filled with current value

After all prompts, saves the updated config and prints a summary of changes. Ctrl+C mid-flow saves nothing.

## `clp status` — Unified View

Merges the old `status` and `config` commands. Output format:

```
claude-profiles v0.1.0

Config: ~/.config/claude-profiles/config.toml

Settings:
  skip_permissions: off
  auto_continue:    on

Profiles:
  * [claude.max]     mode=local (Claude MAX)
    [claude.bedrock]  mode=bedrock profile=bedrock region=us-east-2 credentials=✓ valid
```

The `*` marks the default profile. Bedrock profiles show credential status.

## `exec_claude` Changes

The function signature extends to accept the new flags:

```rust
pub fn exec_claude(
    bin: &str,
    mode: &ProfileMode,
    args: &[String],
    skip_permissions: bool,
    auto_continue: bool,
) -> Result<()>
```

Flags are prepended before user args:
- If `skip_permissions` is true, prepend `--dangerously-skip-permissions`
- If `auto_continue` is true and a session exists for the current directory, prepend `--continue`

Duplicate flags (user also passes them explicitly) are harmless.

## Dependencies

### Added

- `clap` with `derive` feature — structured CLI argument parsing
- `dialoguer` — interactive terminal prompts for `configure`

### Removed

None. All existing dependencies remain.

## Error Handling

- `clp --default nonexistent` — validates profile exists in config, errors with list of available profiles
- `clp -p nonexistent` — same validation
- `clp --default foo -p bar` — `--default` sets and exits, never launches claude; no conflict
- Missing config file — auto-creates with defaults (existing behavior)
- Existing configs missing new fields — serde defaults to `false` for both

## Breaking Changes

- Positional profile selection removed. `clp claude.bedrock` no longer works; use `clp -p claude.bedrock`
- `clp config` subcommand removed, replaced by `clp status`

## Files Changed

- `Cargo.toml` — add `clap` and `dialoguer` dependencies
- `src/main.rs` — replace manual arg parsing with clap, add `configure` subcommand, merge `status`/`config`
- `src/config.rs` — add `skip_permissions` and `auto_continue` fields to `Config`
- `src/claude.rs` — extend `exec_claude` to handle new flags and session detection
- `config.example.toml` — add new fields with comments
