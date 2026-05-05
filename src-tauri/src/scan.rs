use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_DEPTH: usize = 5;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveredRepo {
    pub owner: String,
    pub name: String,
    pub path: String,
    pub remote_url: String,
}

pub fn scan(root: &Path) -> Result<Vec<DiscoveredRepo>, String> {
    if !root.exists() {
        return Err(format!("path does not exist: {}", root.display()));
    }
    if !root.is_dir() {
        return Err(format!("not a directory: {}", root.display()));
    }
    let mut out = Vec::new();
    walk(root, 0, &mut out);
    out.sort_by(|a, b| (&a.owner, &a.name).cmp(&(&b.owner, &b.name)));
    out.dedup_by(|a, b| a.owner == b.owner && a.name == b.name);
    Ok(out)
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<DiscoveredRepo>) {
    if depth > MAX_DEPTH {
        return;
    }
    let git_dir = dir.join(".git");
    if git_dir.exists() {
        if let Some(repo) = read_github_remote(dir) {
            out.push(repo);
        }
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
            if name.starts_with('.') || name == "node_modules" || name == "target" {
                continue;
            }
        }
        walk(&path, depth + 1, out);
    }
}

fn read_github_remote(repo_dir: &Path) -> Option<DiscoveredRepo> {
    let config_path = git_config_path(repo_dir)?;
    let config = fs::read_to_string(&config_path).ok()?;
    let url = first_remote_url(&config)?;
    let (owner, name) = parse_github_url(&url)?;
    Some(DiscoveredRepo {
        owner,
        name,
        path: repo_dir.display().to_string(),
        remote_url: url,
    })
}

fn git_config_path(repo_dir: &Path) -> Option<PathBuf> {
    let dot_git = repo_dir.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git.join("config"));
    }
    // Worktree or submodule: .git is a file containing `gitdir: <path>`.
    if dot_git.is_file() {
        let s = fs::read_to_string(&dot_git).ok()?;
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("gitdir:") {
                let p = Path::new(rest.trim());
                let resolved = if p.is_absolute() {
                    p.to_path_buf()
                } else {
                    repo_dir.join(p)
                };
                return Some(resolved.join("config"));
            }
        }
    }
    None
}

/// Extract the URL from the first `[remote "..."]` section. Prefer "origin"
/// when present.
fn first_remote_url(config: &str) -> Option<String> {
    let mut current_remote: Option<String> = None;
    let mut origin_url: Option<String> = None;
    let mut first_url: Option<String> = None;
    for raw in config.lines() {
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix("[remote \"") {
            if let Some(end) = rest.find("\"]") {
                current_remote = Some(rest[..end].to_string());
            } else {
                current_remote = None;
            }
        } else if line.starts_with('[') {
            current_remote = None;
        } else if let Some(value) = line.strip_prefix("url") {
            let value = value.trim_start();
            if let Some(eq) = value.strip_prefix('=') {
                let url = eq.trim().to_string();
                if current_remote.as_deref() == Some("origin") {
                    origin_url.get_or_insert(url.clone());
                }
                first_url.get_or_insert(url);
            }
        }
    }
    origin_url.or(first_url)
}

/// Accepts:
///   https://github.com/owner/repo(.git)?
///   git@github.com:owner/repo(.git)?
///   ssh://git@github.com/owner/repo(.git)?
fn parse_github_url(url: &str) -> Option<(String, String)> {
    let trimmed = url.trim();
    let after_host = if let Some(rest) = trimmed.strip_prefix("https://github.com/") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("http://github.com/") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("git@github.com:") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("ssh://git@github.com/") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("git://github.com/") {
        rest
    } else {
        return None;
    };
    let after_host = after_host.trim_end_matches('/');
    let after_host = after_host.strip_suffix(".git").unwrap_or(after_host);
    let mut parts = after_host.splitn(2, '/');
    let owner = parts.next()?.trim();
    let name = parts.next()?.trim();
    if owner.is_empty() || name.is_empty() || name.contains('/') {
        return None;
    }
    Some((owner.to_string(), name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_https() {
        assert_eq!(
            parse_github_url("https://github.com/foo/bar.git"),
            Some(("foo".into(), "bar".into()))
        );
        assert_eq!(
            parse_github_url("https://github.com/foo/bar"),
            Some(("foo".into(), "bar".into()))
        );
    }

    #[test]
    fn parses_ssh() {
        assert_eq!(
            parse_github_url("git@github.com:foo/bar.git"),
            Some(("foo".into(), "bar".into()))
        );
        assert_eq!(
            parse_github_url("ssh://git@github.com/foo/bar"),
            Some(("foo".into(), "bar".into()))
        );
    }

    #[test]
    fn rejects_non_github() {
        assert_eq!(parse_github_url("https://gitlab.com/foo/bar"), None);
        assert_eq!(parse_github_url("git@bitbucket.org:foo/bar.git"), None);
    }

    #[test]
    fn rejects_malformed() {
        assert_eq!(parse_github_url("https://github.com/foo"), None);
        assert_eq!(parse_github_url("https://github.com/"), None);
        assert_eq!(parse_github_url("https://github.com/foo/bar/baz"), None);
    }

    #[test]
    fn picks_origin_over_others() {
        let cfg = r#"
[remote "upstream"]
    url = https://github.com/up/repo.git
[remote "origin"]
    url = https://github.com/me/repo.git
"#;
        assert_eq!(
            first_remote_url(cfg).as_deref(),
            Some("https://github.com/me/repo.git")
        );
    }

    #[test]
    fn falls_back_to_first_remote_when_no_origin() {
        let cfg = r#"
[remote "upstream"]
    url = https://github.com/up/repo.git
"#;
        assert_eq!(
            first_remote_url(cfg).as_deref(),
            Some("https://github.com/up/repo.git")
        );
    }
}
