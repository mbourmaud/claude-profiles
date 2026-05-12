use std::env;
use std::path::PathBuf;

/// Check if a Claude session exists for the current working directory.
///
/// Claude 4.x stores sessions as `<uuid>.jsonl` files in
/// `~/.claude/projects/<project-key>/` where `<project-key>` is the absolute
/// working directory path with `/` replaced by `-`.
pub fn has_existing_session() -> bool {
    let cwd = match env::current_dir() {
        Ok(p) => p,
        Err(_) => return false,
    };

    let project_key = cwd
        .to_string_lossy()
        .replace('/', "-");

    let project_dir = claude_projects_dir().join(&project_key);

    match std::fs::read_dir(&project_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .any(|e| {
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    == Some("jsonl")
            }),
        Err(_) => false,
    }
}

fn claude_projects_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("projects")
}
