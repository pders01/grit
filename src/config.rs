use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ForgeType {
    GitHub,
    GitLab,
    Gitea,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ForgeConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub forge_type: ForgeType,
    pub host: String,
    pub token_env: Option<String>,
    pub token_command: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[allow(dead_code)]
pub struct GeneralConfig {
    pub default_forge: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    #[allow(dead_code)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub forges: Vec<ForgeConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            forges: vec![ForgeConfig {
                name: "github".to_string(),
                forge_type: ForgeType::GitHub,
                host: "github.com".to_string(),
                token_env: Some("GITHUB_TOKEN".to_string()),
                token_command: Some("gh auth token".to_string()),
            }],
        }
    }
}

fn config_path() -> Option<PathBuf> {
    let config_dir = dirs::config_dir()?;
    Some(config_dir.join("grit").join("config.toml"))
}

impl Config {
    pub fn load() -> Self {
        let Some(path) = config_path() else {
            return Config::default();
        };

        let Ok(content) = std::fs::read_to_string(&path) else {
            return Config::default();
        };

        match toml::from_str::<Config>(&content) {
            Ok(config) => {
                if config.forges.is_empty() {
                    Config::default()
                } else {
                    config
                }
            }
            Err(_) => Config::default(),
        }
    }
}

/// Detect which forge to use based on the current git remote origin.
/// Returns the matching ForgeConfig, or None if no match.
pub fn detect_forge(config: &Config) -> Option<&ForgeConfig> {
    let output = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let host = extract_host(&url)?;

    config.forges.iter().find(|f| f.host == host)
}

/// Extract hostname from SSH (git@host:...) or HTTPS (https://host/...) URLs
fn extract_host(url: &str) -> Option<String> {
    if let Some(rest) = url.strip_prefix("git@") {
        // SSH: git@host:owner/repo.git
        let host = rest.split(':').next()?;
        Some(host.to_string())
    } else if url.starts_with("https://") || url.starts_with("http://") {
        // HTTPS: https://host/owner/repo.git
        let without_scheme = url.split("://").nth(1)?;
        let host = without_scheme.split('/').next()?;
        Some(host.to_string())
    } else if url.starts_with("ssh://") {
        // SSH: ssh://git@host/owner/repo.git
        let without_scheme = url.split("://").nth(1)?;
        let after_at = without_scheme.split('@').next_back()?;
        let host = after_at.split('/').next()?;
        // Strip port if present
        let host = host.split(':').next()?;
        Some(host.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_config() {
        let toml_str = r#"
[general]
default_forge = "github"

[[forges]]
name = "github"
type = "github"
host = "github.com"
token_env = "GITHUB_TOKEN"
token_command = "gh auth token"

[[forges]]
name = "work-gitlab"
type = "gitlab"
host = "gitlab.company.com"
token_env = "GITLAB_TOKEN"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.forges.len(), 2);
        assert_eq!(config.forges[0].forge_type, ForgeType::GitHub);
        assert_eq!(config.forges[1].forge_type, ForgeType::GitLab);
        assert_eq!(config.forges[1].host, "gitlab.company.com");
    }

    #[test]
    fn parse_empty_config_uses_default() {
        let config = Config::load(); // will use default since file likely doesn't exist in test
        assert!(!config.forges.is_empty());
        assert_eq!(config.forges[0].forge_type, ForgeType::GitHub);
    }

    #[test]
    fn extract_host_ssh() {
        assert_eq!(
            extract_host("git@github.com:owner/repo.git"),
            Some("github.com".to_string())
        );
    }

    #[test]
    fn extract_host_https() {
        assert_eq!(
            extract_host("https://github.com/owner/repo.git"),
            Some("github.com".to_string())
        );
    }

    #[test]
    fn extract_host_http() {
        assert_eq!(
            extract_host("http://gitea.local/owner/repo.git"),
            Some("gitea.local".to_string())
        );
    }

    #[test]
    fn extract_host_ssh_scheme() {
        assert_eq!(
            extract_host("ssh://git@gitlab.com/owner/repo.git"),
            Some("gitlab.com".to_string())
        );
    }

    #[test]
    fn extract_host_ssh_scheme_with_port() {
        assert_eq!(
            extract_host("ssh://git@gitlab.com:2222/owner/repo.git"),
            Some("gitlab.com".to_string())
        );
    }

    #[test]
    fn extract_host_invalid() {
        assert_eq!(extract_host("not-a-url"), None);
    }

    #[test]
    fn detect_forge_matches_config() {
        let config = Config {
            general: GeneralConfig::default(),
            forges: vec![
                ForgeConfig {
                    name: "github".to_string(),
                    forge_type: ForgeType::GitHub,
                    host: "github.com".to_string(),
                    token_env: None,
                    token_command: None,
                },
                ForgeConfig {
                    name: "gitlab".to_string(),
                    forge_type: ForgeType::GitLab,
                    host: "gitlab.company.com".to_string(),
                    token_env: None,
                    token_command: None,
                },
            ],
        };

        // detect_forge will run `git remote get-url origin` — we can't control that in tests,
        // but we can test extract_host + manual matching
        let host = extract_host("git@gitlab.company.com:team/project.git").unwrap();
        let matched = config.forges.iter().find(|f| f.host == host);
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().forge_type, ForgeType::GitLab);
    }
}
