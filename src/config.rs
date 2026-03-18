use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

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
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub default_profile: String,
    #[serde(default)]
    pub skip_permissions: bool,
    #[serde(default)]
    pub auto_continue: bool,
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
            },
        );

        profiles.insert(
            "claude.bedrock".to_string(),
            Profile {
                mode: ProfileMode::Bedrock {
                    aws_profile: "bedrock".to_string(),
                    aws_region: "us-east-2".to_string(),
                },
            },
        );

        Self {
            default_profile: "claude.max".to_string(),
            skip_permissions: false,
            auto_continue: false,
            profiles,
        }
    }
}
