use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::auth;
use crate::github::rest;
use crate::github::types::{RepoRef, WorkflowJob};
use crate::github::GitHubClient;
use crate::scan::{self, DiscoveredRepo};
use crate::state::{
    merge_aggregates, AppSnapshot, AuthStatus, NotificationPrefs, Project, ProjectRepoRef,
    SharedState, WatchedRepoConfig,
};
use crate::store;
use crate::tray;

#[tauri::command]
pub async fn auth_status() -> AuthStatus {
    auth::current_status().await
}

#[tauri::command]
pub fn get_state(state: State<'_, SharedState>) -> AppSnapshot {
    state.snapshot.read().clone()
}

#[tauri::command]
pub async fn get_run_jobs(
    repo: RepoRef,
    run_id: u64,
    client: State<'_, GitHubClient>,
) -> Result<Vec<WorkflowJob>, String> {
    rest::list_run_jobs(&client, &repo, run_id)
        .await
        .map_err(|e| format!("{e}"))
}

#[tauri::command]
pub async fn add_watched_repo(
    owner: String,
    name: String,
    branch_filter: Option<String>,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    {
        let mut cfg = state.config.write();
        store::upsert_watched(
            &mut cfg,
            WatchedRepoConfig {
                owner,
                name,
                branch_filter,
                excluded_workflows: Vec::new(),
                dismissed_until_run_id: None,
            },
        );
        store::save_config(&app, &cfg).map_err(|e| format!("{e}"))?;
    }
    state.wakeup.notify_one();
    Ok(())
}

#[tauri::command]
pub async fn add_watched_repos_bulk(
    repos: Vec<WatchedRepoConfig>,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<usize, String> {
    let mut added = 0usize;
    {
        let mut cfg = state.config.write();
        for r in repos {
            let exists = cfg
                .watched
                .iter()
                .any(|w| w.owner == r.owner && w.name == r.name);
            store::upsert_watched(&mut cfg, r);
            if !exists {
                added += 1;
            }
        }
        store::save_config(&app, &cfg).map_err(|e| format!("{e}"))?;
    }
    state.wakeup.notify_one();
    Ok(added)
}

#[tauri::command]
pub async fn scan_path_for_repos(path: String) -> Result<Vec<DiscoveredRepo>, String> {
    let p = PathBuf::from(shellexpand_tilde(&path));
    scan::scan(&p)
}

fn shellexpand_tilde(input: &str) -> String {
    if let Some(rest) = input.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            let mut p = PathBuf::from(home);
            p.push(rest);
            return p.display().to_string();
        }
    } else if input == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).display().to_string();
        }
    }
    input.to_string()
}

#[tauri::command]
pub async fn remove_watched_repo(
    owner: String,
    name: String,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    {
        let mut cfg = state.config.write();
        store::remove_watched(&mut cfg, &owner, &name);
        store::save_config(&app, &cfg).map_err(|e| format!("{e}"))?;
    }
    let snap = {
        let mut s = state.snapshot.write();
        s.watched.retain(|w| !(w.repo.owner == owner && w.repo.name == name));
        s.clone()
    };
    let _ = app.emit("runs:updated", &snap);
    state.wakeup.notify_one();
    Ok(())
}

#[tauri::command]
pub async fn set_notification_prefs(
    prefs: NotificationPrefs,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let mut cfg = state.config.write();
    store::set_notification_prefs(&mut cfg, prefs);
    store::save_config(&app, &cfg).map_err(|e| format!("{e}"))
}

#[tauri::command]
pub async fn set_global_excluded_workflows(
    names: Vec<String>,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<Vec<String>, String> {
    let normalized = {
        let mut cfg = state.config.write();
        store::set_global_excluded_workflows(&mut cfg, names);
        store::save_config(&app, &cfg).map_err(|e| format!("{e}"))?;
        cfg.global_excluded_workflows.clone()
    };
    state.snapshot.write().global_excluded_workflows = normalized.clone();
    state.wakeup.notify_one();
    Ok(normalized)
}

#[tauri::command]
pub async fn set_repo_excluded_workflows(
    owner: String,
    name: String,
    names: Vec<String>,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<Vec<String>, String> {
    let updated = {
        let mut cfg = state.config.write();
        if !store::set_repo_excluded_workflows(&mut cfg, &owner, &name, names) {
            return Err(format!("repo {owner}/{name} is not in the watched list"));
        }
        store::save_config(&app, &cfg).map_err(|e| format!("{e}"))?;
        cfg.watched
            .iter()
            .find(|w| w.owner == owner && w.name == name)
            .map(|w| w.excluded_workflows.clone())
            .unwrap_or_default()
    };
    {
        let mut snap = state.snapshot.write();
        if let Some(slot) = snap
            .watched
            .iter_mut()
            .find(|w| w.repo.owner == owner && w.repo.name == name)
        {
            slot.excluded_workflows = updated.clone();
        }
    }
    state.wakeup.notify_one();
    Ok(updated)
}

#[tauri::command]
pub async fn dismiss_watched_repo(
    owner: String,
    name: String,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    // Use the latest known run id from the snapshot as the high-water mark.
    // Anything with id <= this is considered acknowledged; the next poll that
    // surfaces a higher id clears the dismissal.
    let max_id = {
        let snap = state.snapshot.read();
        snap.watched
            .iter()
            .find(|w| w.repo.owner == owner && w.repo.name == name)
            .map(|w| w.recent_runs.iter().map(|r| r.id).max().unwrap_or(0))
            .unwrap_or(0)
    };
    {
        let mut cfg = state.config.write();
        if !store::set_repo_dismissed_until(&mut cfg, &owner, &name, Some(max_id)) {
            return Err(format!("repo {owner}/{name} is not in the watched list"));
        }
        store::save_config(&app, &cfg).map_err(|e| format!("{e}"))?;
    }
    update_dismissed_and_emit(&app, &state, &owner, &name, true);
    Ok(())
}

#[tauri::command]
pub async fn undismiss_watched_repo(
    owner: String,
    name: String,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    {
        let mut cfg = state.config.write();
        if !store::set_repo_dismissed_until(&mut cfg, &owner, &name, None) {
            return Err(format!("repo {owner}/{name} is not in the watched list"));
        }
        store::save_config(&app, &cfg).map_err(|e| format!("{e}"))?;
    }
    update_dismissed_and_emit(&app, &state, &owner, &name, false);
    Ok(())
}

fn update_dismissed_and_emit(
    app: &AppHandle,
    state: &SharedState,
    owner: &str,
    name: &str,
    dismissed: bool,
) {
    let snap = {
        let mut s = state.snapshot.write();
        if let Some(slot) = s
            .watched
            .iter_mut()
            .find(|w| w.repo.owner == owner && w.repo.name == name)
        {
            slot.dismissed = dismissed;
        }
        // Recompute the overall aggregate so the tray icon reflects the change
        // immediately, without waiting for the next poll cycle.
        let pr_aggs = s.prs.iter().map(|p| p.aggregate).collect::<Vec<_>>();
        let watched_aggs = s
            .watched
            .iter()
            .filter(|w| !w.dismissed)
            .map(|w| w.aggregate)
            .collect::<Vec<_>>();
        s.aggregate = merge_aggregates(pr_aggs.into_iter().chain(watched_aggs));
        s.clone()
    };
    let _ = app.emit("runs:updated", &snap);
    tray::update_icon(app, snap.aggregate);
}

#[tauri::command]
pub async fn open_run_in_browser(url: String, app: AppHandle) -> Result<(), String> {
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| format!("{e}"))
}

#[tauri::command]
pub fn force_refresh(state: State<'_, SharedState>) {
    state.wakeup.notify_one();
}

#[tauri::command]
pub fn show_dashboard(app: AppHandle) {
    if let Some(w) = app.get_webview_window("dashboard") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[tauri::command]
pub fn hide_popup(app: AppHandle) {
    if let Some(w) = app.get_webview_window("tray-popup") {
        let _ = w.hide();
    }
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

fn snapshot_after_projects_change(state: &SharedState) -> AppSnapshot {
    let projects = state.config.read().projects.clone();
    let mut s = state.snapshot.write();
    s.projects = projects;
    s.clone()
}

#[tauri::command]
pub async fn upsert_project(
    project: Project,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<Vec<Project>, String> {
    if project.id.trim().is_empty() {
        return Err("project id is required".into());
    }
    if project.name.trim().is_empty() {
        return Err("project name is required".into());
    }
    {
        let mut cfg = state.config.write();
        store::upsert_project(&mut cfg, project);
        store::save_config(&app, &cfg).map_err(|e| format!("{e}"))?;
    }
    let snap = snapshot_after_projects_change(&state);
    let _ = app.emit("runs:updated", &snap);
    Ok(snap.projects)
}

#[tauri::command]
pub async fn remove_project(
    id: String,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<Vec<Project>, String> {
    {
        let mut cfg = state.config.write();
        store::remove_project(&mut cfg, &id);
        store::save_config(&app, &cfg).map_err(|e| format!("{e}"))?;
    }
    let snap = snapshot_after_projects_change(&state);
    let _ = app.emit("runs:updated", &snap);
    Ok(snap.projects)
}

#[tauri::command]
pub async fn set_project_enabled(
    id: String,
    enabled: bool,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<Vec<Project>, String> {
    {
        let mut cfg = state.config.write();
        if !store::set_project_enabled(&mut cfg, &id, enabled) {
            return Err(format!("project {id} not found"));
        }
        store::save_config(&app, &cfg).map_err(|e| format!("{e}"))?;
    }
    let snap = snapshot_after_projects_change(&state);
    let _ = app.emit("runs:updated", &snap);
    Ok(snap.projects)
}

#[tauri::command]
pub async fn set_project_members(
    id: String,
    members: Vec<ProjectRepoRef>,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<Vec<Project>, String> {
    {
        let mut cfg = state.config.write();
        if !store::set_project_members(&mut cfg, &id, members) {
            return Err(format!("project {id} not found"));
        }
        store::save_config(&app, &cfg).map_err(|e| format!("{e}"))?;
    }
    let snap = snapshot_after_projects_change(&state);
    let _ = app.emit("runs:updated", &snap);
    Ok(snap.projects)
}
