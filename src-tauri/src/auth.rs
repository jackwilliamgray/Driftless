use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use tokio::process::Command;
use tracing::warn;

use crate::state::AuthStatus;

/// Locates the `gh` binary. Bundled macOS apps launched from Finder inherit
/// a minimal PATH (`/usr/bin:/bin:/usr/sbin:/sbin`) that excludes Homebrew,
/// so a bare `Command::new("gh")` fails even when the user is logged in.
fn resolve_gh() -> Option<PathBuf> {
    for candidate in [
        "/opt/homebrew/bin/gh",
        "/usr/local/bin/gh",
        "/run/current-system/sw/bin/gh",
    ] {
        let p = PathBuf::from(candidate);
        if p.exists() {
            return Some(p);
        }
    }
    which()
}

fn which() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join("gh"))
        .find(|p| p.exists())
}

/// Reads a fresh token from the GitHub CLI. Returns `Ok(None)` when `gh` is
/// not on PATH or the user is not logged in — these are expected steady-state
/// conditions, not errors.
pub async fn read_gh_token() -> Result<Option<String>> {
    let Some(gh) = resolve_gh() else {
        warn!("gh CLI not found in PATH or common install locations");
        return Ok(None);
    };
    let out = match Command::new(&gh).args(["auth", "token"]).output().await {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            warn!("gh CLI vanished between resolve and exec at {}", gh.display());
            return Ok(None);
        }
        Err(e) => return Err(e).context("running `gh auth token`"),
    };
    if !out.status.success() {
        return Ok(None);
    }
    let token = String::from_utf8(out.stdout)
        .context("gh produced non-utf8 token")?
        .trim()
        .to_string();
    if token.is_empty() {
        return Ok(None);
    }
    Ok(Some(token))
}

/// Calls `gh api user` with the active token to verify it works and pull the
/// login. We don't trust `gh auth status` because it doesn't return JSON.
pub async fn whoami(token: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .user_agent("Driftless")
        .build()?;
    let resp = client
        .get("https://api.github.com/user")
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(anyhow!(
            "GitHub /user returned {}: {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        ));
    }
    let body: serde_json::Value = resp.json().await?;
    body.get("login")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("no login in /user response"))
}

pub async fn current_status() -> AuthStatus {
    match read_gh_token().await {
        Ok(Some(tok)) => match whoami(&tok).await {
            Ok(login) => AuthStatus {
                logged_in: true,
                login: Some(login),
                error: None,
            },
            Err(e) => AuthStatus {
                logged_in: false,
                login: None,
                error: Some(format!("token rejected: {e}")),
            },
        },
        Ok(None) => AuthStatus {
            logged_in: false,
            login: None,
            error: None,
        },
        Err(e) => AuthStatus {
            logged_in: false,
            login: None,
            error: Some(format!("could not read gh token: {e}")),
        },
    }
}
