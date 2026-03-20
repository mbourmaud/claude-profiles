use std::io::{self, Write};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use aws_config::BehaviorVersion;
use aws_sdk_ssooidc::Client as OidcClient;

pub struct AwsSession {
    pub aws_profile: String,
    pub aws_region: String,
}

impl AwsSession {
    pub fn new(aws_profile: String, aws_region: String) -> Self {
        Self {
            aws_profile,
            aws_region,
        }
    }

    /// Check if credentials for the profile are valid via STS get-caller-identity.
    pub async fn credentials_valid(&self) -> bool {
        let config = aws_config::defaults(BehaviorVersion::latest())
            .profile_name(&self.aws_profile)
            .region(aws_config::Region::new(self.aws_region.clone()))
            .load()
            .await;

        let sts = aws_sdk_sts::Client::new(&config);
        sts.get_caller_identity().send().await.is_ok()
    }

    /// SSO login via OIDC device authorization grant.
    /// Writes credentials to ~/.aws/credentials under the profile.
    pub async fn sso_login(&self) -> Result<()> {
        println!(
            "[clp] Credentials expired for profile '{}'. Starting SSO login...",
            self.aws_profile
        );

        let profile_config = self.load_profile_sso_config()?;

        let oidc_config = aws_config::defaults(BehaviorVersion::latest())
            .region(aws_config::Region::new(profile_config.sso_region.clone()))
            .no_credentials()
            .load()
            .await;

        let oidc_client = OidcClient::new(&oidc_config);

        // Register client
        let registration = oidc_client
            .register_client()
            .client_name("claude-profiles")
            .client_type("public")
            .send()
            .await
            .context("Failed to register OIDC client")?;

        let client_id = registration
            .client_id()
            .ok_or_else(|| anyhow!("No client_id in registration"))?
            .to_string();
        let client_secret = registration
            .client_secret()
            .ok_or_else(|| anyhow!("No client_secret in registration"))?
            .to_string();

        // Start device authorization
        let auth = oidc_client
            .start_device_authorization()
            .client_id(&client_id)
            .client_secret(&client_secret)
            .start_url(&profile_config.sso_start_url)
            .send()
            .await
            .context("Failed to start device authorization")?;

        let device_code = auth
            .device_code()
            .ok_or_else(|| anyhow!("No device_code"))?
            .to_string();
        let user_code = auth.user_code().unwrap_or("").to_string();
        let verification_url = auth
            .verification_uri_complete()
            .or(auth.verification_uri())
            .unwrap_or("")
            .to_string();
        let interval = auth.interval().max(5) as u64;
        let expires_in = auth.expires_in() as u64;

        println!(
            "\n[clp] Open this URL in your browser:\n  {}\n",
            verification_url
        );
        if !user_code.is_empty() {
            println!(
                "[clp] Or go to {} and enter code: {}\n",
                auth.verification_uri().unwrap_or(""),
                user_code
            );
        }
        print!("[clp] Waiting for authorization");
        io::stdout().flush()?;

        // Poll for token
        let deadline = tokio::time::Instant::now() + Duration::from_secs(expires_in);

        let token = loop {
            if tokio::time::Instant::now() > deadline {
                return Err(anyhow!("SSO login timed out"));
            }

            tokio::time::sleep(Duration::from_secs(interval)).await;
            print!(".");
            io::stdout().flush()?;

            match oidc_client
                .create_token()
                .client_id(&client_id)
                .client_secret(&client_secret)
                .grant_type("urn:ietf:params:oauth:grant-type:device_code")
                .device_code(&device_code)
                .send()
                .await
            {
                Ok(resp) => break resp,
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("AuthorizationPending") || msg.contains("SlowDown") {
                        continue;
                    }
                    return Err(anyhow!("SSO token error: {}", msg));
                }
            }
        };

        println!("\n[clp] Authorized! Writing credentials...");

        let access_token = token
            .access_token()
            .ok_or_else(|| anyhow!("No access_token"))?
            .to_string();

        // Fetch role credentials via SSO
        let sso_config = aws_config::defaults(BehaviorVersion::latest())
            .region(aws_config::Region::new(profile_config.sso_region.clone()))
            .no_credentials()
            .load()
            .await;
        let sso_client = aws_sdk_sso::Client::new(&sso_config);

        let creds = sso_client
            .get_role_credentials()
            .account_id(&profile_config.sso_account_id)
            .role_name(&profile_config.sso_role_name)
            .access_token(&access_token)
            .send()
            .await
            .context("Failed to get role credentials")?;

        let role_creds = creds
            .role_credentials()
            .ok_or_else(|| anyhow!("No role_credentials in response"))?;

        self.write_credentials(
            role_creds.access_key_id().unwrap_or(""),
            role_creds.secret_access_key().unwrap_or(""),
            role_creds.session_token().unwrap_or(""),
        )?;

        println!(
            "[clp] SSO login successful for profile '{}'.",
            self.aws_profile
        );
        Ok(())
    }

    fn write_credentials(
        &self,
        access_key_id: &str,
        secret_access_key: &str,
        session_token: &str,
    ) -> Result<()> {
        let creds_path = dirs::home_dir()
            .unwrap_or_default()
            .join(".aws")
            .join("credentials");

        let mut content = if creds_path.exists() {
            std::fs::read_to_string(&creds_path)?
        } else {
            String::new()
        };

        let header = format!("[{}]", self.aws_profile);
        let new_section = format!(
            "{}\naws_access_key_id = {}\naws_secret_access_key = {}\naws_session_token = {}\n",
            header, access_key_id, secret_access_key, session_token,
        );

        if let Some(start) = content.find(&header) {
            // Replace existing section up to the next [profile] or EOF
            let section_start = start;
            let section_end = content[start + 1..]
                .find("\n[")
                .map(|p| start + 1 + p + 1)
                .unwrap_or(content.len());
            content.replace_range(section_start..section_end, &new_section);
        } else {
            if !content.ends_with('\n') && !content.is_empty() {
                content.push('\n');
            }
            content.push_str(&new_section);
        }

        if let Some(parent) = creds_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&creds_path, content)?;
        Ok(())
    }

    fn load_profile_sso_config(&self) -> Result<SsoProfileConfig> {
        let config_path = dirs::home_dir()
            .unwrap_or_default()
            .join(".aws")
            .join("config");

        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Cannot read {}", config_path.display()))?;

        let section_header = format!("[profile {}]", self.aws_profile);
        let start = content
            .find(&section_header)
            .ok_or_else(|| anyhow!("Profile '{}' not found in ~/.aws/config", self.aws_profile))?;

        let rest = &content[start + section_header.len()..];
        let end = rest.find("\n[").unwrap_or(rest.len());
        let section = &rest[..end];

        let get = |key: &str| -> Result<String> {
            section
                .lines()
                .find_map(|line| {
                    let line = line.trim();
                    line.strip_prefix(key)
                        .map(|rest| rest.trim_start())
                        .and_then(|rest| rest.strip_prefix('='))
                        .map(|v| v.trim().to_string())
                })
                .ok_or_else(|| anyhow!("Missing '{}' in [profile {}]", key, self.aws_profile))
        };

        Ok(SsoProfileConfig {
            sso_start_url: get("sso_start_url")?,
            sso_region: get("sso_region")?,
            sso_account_id: get("sso_account_id")?,
            sso_role_name: get("sso_role_name")?,
        })
    }
}

struct SsoProfileConfig {
    sso_start_url: String,
    sso_region: String,
    sso_account_id: String,
    sso_role_name: String,
}
