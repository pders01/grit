use std::io::Write;
use std::time::Duration;

use crate::config::{ForgeConfig, ForgeType};

/// Try to run a CLI command and capture stdout as a token
fn try_cli_token(command: &str) -> Option<String> {
    let output = std::process::Command::new("sh")
        .args(["-c", command])
        .output()
        .ok()?;

    if output.status.success() {
        let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !token.is_empty() {
            return Some(token);
        }
    }
    None
}

/// Get the forge-specific stored token path: ~/.config/grit/tokens/{forge_name}
fn forge_token_path(forge_name: &str) -> Option<std::path::PathBuf> {
    let config_dir = dirs::config_dir()?;
    Some(config_dir.join("grit").join("tokens").join(forge_name))
}

/// Load token from forge-specific storage
fn load_forge_stored_token(forge_name: &str) -> Option<String> {
    let path = forge_token_path(forge_name)?;
    let token = std::fs::read_to_string(path).ok()?;
    let token = token.trim().to_string();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

/// Save token to forge-specific storage
fn save_forge_token(forge_name: &str, token: &str) -> std::io::Result<()> {
    if let Some(path) = forge_token_path(forge_name) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, token)?;
    }
    Ok(())
}

/// Load a token for any forge, trying multiple sources:
/// 1. Forge-specific env var (from config)
/// 2. Stored token from ~/.config/grit/tokens/{forge_name}
/// 3. CLI command (from config)
/// 4. GitHub-only: OAuth device flow
pub async fn load_forge_token(forge_config: &ForgeConfig) -> Result<String, String> {
    // 1. Environment variable from config
    if let Some(env_var) = &forge_config.token_env {
        if let Ok(token) = std::env::var(env_var) {
            if !token.is_empty() {
                return Ok(token);
            }
        }
    }

    // 2. Forge-specific stored token
    if let Some(token) = load_forge_stored_token(&forge_config.name) {
        return Ok(token);
    }

    // Also check legacy stored token for backward compat with GitHub
    if forge_config.forge_type == ForgeType::GitHub {
        if let Some(token) = load_stored_token() {
            return Ok(token);
        }
    }

    // 3. CLI command from config
    if let Some(cmd) = &forge_config.token_command {
        if let Some(token) = try_cli_token(cmd) {
            let _ = save_forge_token(&forge_config.name, &token);
            return Ok(token);
        }
    }

    // 4. GitHub-only: OAuth device flow
    if forge_config.forge_type == ForgeType::GitHub {
        println!("No GitHub token found.");
        println!("Starting GitHub OAuth device flow...");

        let token = device_flow_auth(GITHUB_CLIENT_ID).await?;
        if let Err(e) = save_forge_token(&forge_config.name, &token) {
            eprintln!("Warning: could not save token: {}", e);
        }
        return Ok(token);
    }

    Err(format!(
        "No token found for forge '{}'. Set {} or configure a token_command.",
        forge_config.name,
        forge_config
            .token_env
            .as_deref()
            .unwrap_or("a token env var")
    ))
}

/// Get the legacy config file path: ~/.config/grit/token
fn token_path() -> Option<std::path::PathBuf> {
    let config_dir = dirs::config_dir()?;
    Some(config_dir.join("grit").join("token"))
}

/// Load token from disk
fn load_stored_token() -> Option<String> {
    let path = token_path()?;
    let token = std::fs::read_to_string(path).ok()?;
    let token = token.trim().to_string();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

/// GitHub OAuth device flow
/// Requires a registered OAuth App client_id (not secret)
async fn device_flow_auth(client_id: &str) -> Result<String, String> {
    let client = reqwest::Client::new();

    // Step 1: Request device code
    let resp = client
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&[("client_id", client_id), ("scope", "repo")])
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    let device_code = body["device_code"]
        .as_str()
        .ok_or("No device_code in response")?
        .to_string();
    let user_code = body["user_code"]
        .as_str()
        .ok_or("No user_code in response")?
        .to_string();
    let verification_uri = body["verification_uri"]
        .as_str()
        .ok_or("No verification_uri in response")?
        .to_string();
    let interval = body["interval"].as_u64().unwrap_or(5);

    // Step 2: Show instructions to user
    println!();
    println!("  To authenticate grit with GitHub:");
    println!("  1. Open: {}", verification_uri);
    println!("  2. Enter code: {}", user_code);
    println!();
    print!("  Waiting for authorization...");
    std::io::stdout().flush().ok();

    // Step 3: Poll for token
    loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;

        let resp = client
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .form(&[
                ("client_id", client_id),
                ("device_code", &device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

        if let Some(token) = body["access_token"].as_str() {
            println!(" done!");
            return Ok(token.to_string());
        }

        match body["error"].as_str() {
            Some("authorization_pending") => {
                print!(".");
                std::io::stdout().flush().ok();
                continue;
            }
            Some("slow_down") => {
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
            Some("expired_token") => {
                println!();
                return Err("Device code expired. Please try again.".to_string());
            }
            Some("access_denied") => {
                println!();
                return Err("Authorization denied by user.".to_string());
            }
            Some(err) => {
                println!();
                return Err(format!("OAuth error: {}", err));
            }
            None => continue,
        }
    }
}

// Note: This client_id must be from a registered GitHub OAuth App for grit.
// It is NOT secret - OAuth Apps use client_id publicly for device flow.
// Users should register their own app or this can be updated with an official one.
const GITHUB_CLIENT_ID: &str = "Ov23liYMRxFDN38Slfzr";
