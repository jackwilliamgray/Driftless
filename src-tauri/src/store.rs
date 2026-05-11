use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use crate::state::{NotificationPrefs, PersistedConfig, Project, ProjectRepoRef, WatchedRepoConfig};

const STORE_FILE: &str = "driftless.config.json";
const KEY_CONFIG: &str = "config";
const KEY_LAST_RUN_CONCLUSIONS: &str = "last_run_conclusions";

pub fn load_config(app: &AppHandle) -> Result<PersistedConfig> {
    let store = app.store(STORE_FILE).context("opening store")?;
    let raw = store.get(KEY_CONFIG).unwrap_or(Value::Null);
    if raw.is_null() {
        return Ok(PersistedConfig::default());
    }
    serde_json::from_value::<PersistedConfig>(raw).context("parsing persisted config")
}

pub fn save_config(app: &AppHandle, config: &PersistedConfig) -> Result<()> {
    let store = app.store(STORE_FILE).context("opening store")?;
    store.set(KEY_CONFIG, serde_json::to_value(config)?);
    store.save().context("saving store")
}

pub fn load_last_run_conclusions(app: &AppHandle) -> HashMap<u64, String> {
    let Ok(store) = app.store(STORE_FILE) else {
        return HashMap::new();
    };
    let Some(v) = store.get(KEY_LAST_RUN_CONCLUSIONS) else {
        return HashMap::new();
    };
    serde_json::from_value::<HashMap<String, String>>(v)
        .map(|m| {
            m.into_iter()
                .filter_map(|(k, v)| k.parse::<u64>().ok().map(|id| (id, v)))
                .collect()
        })
        .unwrap_or_default()
}

pub fn save_last_run_conclusions(app: &AppHandle, map: &HashMap<u64, String>) -> Result<()> {
    let store = app.store(STORE_FILE).context("opening store")?;
    let stringified: HashMap<String, String> =
        map.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    store.set(KEY_LAST_RUN_CONCLUSIONS, serde_json::to_value(stringified)?);
    store.save().context("saving store")
}

#[allow(dead_code)]
pub fn upsert_watched(config: &mut PersistedConfig, w: WatchedRepoConfig) {
    if let Some(slot) = config
        .watched
        .iter_mut()
        .find(|x| x.owner == w.owner && x.name == w.name)
    {
        slot.branch_filter = w.branch_filter;
        // Preserve existing exclusions unless the caller provided non-empty ones.
        if !w.excluded_workflows.is_empty() {
            slot.excluded_workflows = w.excluded_workflows;
        }
        // Dismissal is a poll-driven flag; never overwritten via upsert.
    } else {
        config.watched.push(w);
    }
}

#[allow(dead_code)]
pub fn set_repo_dismissed_until(
    config: &mut PersistedConfig,
    owner: &str,
    name: &str,
    until: Option<u64>,
) -> bool {
    if let Some(slot) = config
        .watched
        .iter_mut()
        .find(|w| w.owner == owner && w.name == name)
    {
        slot.dismissed_until_run_id = until;
        true
    } else {
        false
    }
}

#[allow(dead_code)]
pub fn remove_watched(config: &mut PersistedConfig, owner: &str, name: &str) {
    config.watched.retain(|w| !(w.owner == owner && w.name == name));
}

#[allow(dead_code)]
pub fn set_notification_prefs(config: &mut PersistedConfig, prefs: NotificationPrefs) {
    config.notification_prefs = prefs;
}

#[allow(dead_code)]
pub fn set_debug_logging(config: &mut PersistedConfig, enabled: bool) {
    config.debug_logging = enabled;
}

#[allow(dead_code)]
pub fn set_global_excluded_workflows(config: &mut PersistedConfig, names: Vec<String>) {
    config.global_excluded_workflows = normalize_names(names);
}

#[allow(dead_code)]
pub fn set_repo_excluded_workflows(
    config: &mut PersistedConfig,
    owner: &str,
    name: &str,
    names: Vec<String>,
) -> bool {
    if let Some(slot) = config
        .watched
        .iter_mut()
        .find(|w| w.owner == owner && w.name == name)
    {
        slot.excluded_workflows = normalize_names(names);
        true
    } else {
        false
    }
}

#[allow(dead_code)]
pub fn upsert_project(config: &mut PersistedConfig, project: Project) {
    if let Some(slot) = config.projects.iter_mut().find(|p| p.id == project.id) {
        *slot = project;
    } else {
        config.projects.push(project);
    }
}

#[allow(dead_code)]
pub fn remove_project(config: &mut PersistedConfig, id: &str) {
    config.projects.retain(|p| p.id != id);
}

#[allow(dead_code)]
pub fn set_project_enabled(config: &mut PersistedConfig, id: &str, enabled: bool) -> bool {
    if let Some(p) = config.projects.iter_mut().find(|p| p.id == id) {
        p.enabled = enabled;
        true
    } else {
        false
    }
}

#[allow(dead_code)]
pub fn set_project_members(
    config: &mut PersistedConfig,
    id: &str,
    members: Vec<ProjectRepoRef>,
) -> bool {
    if let Some(p) = config.projects.iter_mut().find(|p| p.id == id) {
        let mut seen = std::collections::HashSet::new();
        p.member_repos = members
            .into_iter()
            .filter(|m| seen.insert((m.owner.to_lowercase(), m.name.to_lowercase())))
            .collect();
        true
    } else {
        false
    }
}

fn normalize_names(names: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for n in names {
        let trimmed = n.trim();
        if trimmed.is_empty() {
            continue;
        }
        let key = trimmed.to_lowercase();
        if seen.insert(key) {
            out.push(trimmed.to_string());
        }
    }
    out
}
