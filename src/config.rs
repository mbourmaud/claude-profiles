use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateCheck {
    #[default]
    Notify,
    Auto,
    Off,
}

impl std::fmt::Display for UpdateCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateCheck::Notify => write!(f, "notify"),
            UpdateCheck::Auto => write!(f, "auto"),
            UpdateCheck::Off => write!(f, "off"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum ProfileMode {
    Local,
    Bedrock {
        aws_profile: String,
        aws_region: String,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Profile {
    #[serde(flatten)]
    pub mode: ProfileMode,
    /// Default Claude model to use (e.g., "anthropic.claude-sonnet-4-6")
    #[serde(default)]
    pub default_model: Option<String>,
    /// Custom environment variables to set for this profile
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub default_profile: String,
    #[serde(default)]
    pub skip_permissions: bool,
    #[serde(default)]
    pub auto_continue: bool,
    #[serde(default)]
    pub update_check: UpdateCheck,
    pub profiles: HashMap<String, Profile>,
}

impl Config {
    pub fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("claude-profiles")
            .join("config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::path();

        if !path.exists() {
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config at {}", path.display()))?;

        toml::from_str(&content).with_context(|| format!("Invalid config at {}", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    pub fn get_profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    pub fn profile_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.profiles.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }
}

impl Default for Config {
    fn default() -> Self {
        let mut profiles = HashMap::new();

        profiles.insert(
            "claude.max".to_string(),
            Profile {
                mode: ProfileMode::Local,
                default_model: Some("claude-sonnet-4-6".to_string()),
                env: HashMap::new(),
            },
        );

        let mut bedrock_env = HashMap::new();
        bedrock_env.insert("CLAUDE_MODEL".to_string(), "anthropic.claude-sonnet-4-6".to_string());

        profiles.insert(
            "claude.bedrock".to_string(),
            Profile {
                mode: ProfileMode::Bedrock {
                    aws_profile: "bedrock".to_string(),
                    aws_region: "us-east-2".to_string(),
                },
                default_model: Some("anthropic.claude-sonnet-4-6".to_string()),
                env: bedrock_env,
            },
        );

        Self {
            default_profile: "claude.max".to_string(),
            skip_permissions: false,
            auto_continue: false,
            update_check: UpdateCheck::default(),
            profiles,
        }
    }
}
