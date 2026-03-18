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
