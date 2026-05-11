mod auth;
mod commands;
mod github;
mod notify;
mod poller;
mod scan;
mod state;
mod store;
mod tray;

use std::sync::Arc;
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::github::GitHubClient;
use crate::state::{AppState, SharedState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("DRIFTLESS_LOG").unwrap_or_else(|_| {
                EnvFilter::new("info,driftless_lib=debug")
            }),
        )
        .with_target(false)
        .try_init();

    let shared: SharedState = Arc::new(AppState::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(shared.clone())
        .invoke_handler(tauri::generate_handler![
            commands::auth_status,
            commands::get_state,
            commands::get_run_jobs,
            commands::add_watched_repo,
            commands::add_watched_repos_bulk,
            commands::remove_watched_repo,
            commands::scan_path_for_repos,
            commands::set_notification_prefs,
            commands::set_global_excluded_workflows,
            commands::set_repo_excluded_workflows,
            commands::dismiss_watched_repo,
            commands::undismiss_watched_repo,
            commands::open_run_in_browser,
            commands::force_refresh,
            commands::show_dashboard,
            commands::hide_popup,
            commands::quit_app,
            commands::upsert_project,
            commands::remove_project,
            commands::set_project_enabled,
            commands::set_project_members,
        ])
        .setup(move |app| {
            // macOS: hide dock icon, run as menu-bar accessory.
            #[cfg(target_os = "macos")]
            {
                let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            // Hydrate persisted config & last-known run conclusions.
            match store::load_config(&app.handle()) {
                Ok(cfg) => {
                    {
                        let mut snap = shared.snapshot.write();
                        snap.global_excluded_workflows = cfg.global_excluded_workflows.clone();
                        snap.projects = cfg.projects.clone();
                    }
                    *shared.config.write() = cfg;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to load persisted config; using defaults");
                }
            }
            *shared.last_run_conclusions.write() =
                store::load_last_run_conclusions(&app.handle());

            // Build the GitHub client and stash for command access.
            let client = GitHubClient::new(shared.clone())?;
            app.manage(client.clone());

            // Tray icon + menu.
            tray::install(&app.handle())?;

            // Hide popup window if it auto-shows on launch (visible: false in
            // config means it won't, but defensive).
            if let Some(w) = app.get_webview_window("tray-popup") {
                let _ = w.hide();
            }

            // Bridge popup window blur → hide so it dismisses when clicked off.
            if let Some(w) = app.get_webview_window("tray-popup") {
                let app_handle = app.handle().clone();
                w.on_window_event(move |ev| {
                    if let tauri::WindowEvent::Focused(false) = ev {
                        if let Some(w) = app_handle.get_webview_window("tray-popup") {
                            let _ = w.hide();
                        }
                    }
                });
            }

            // Background poller.
            poller::spawn(app.handle().clone(), shared.clone(), client);

            info!("Driftless setup complete");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
