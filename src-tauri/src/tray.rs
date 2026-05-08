use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tauri_plugin_positioner::{Position, WindowExt};
use tracing::{debug, warn};

use crate::state::AggregateState;

const TRAY_ICON_ID: &str = "main-tray";

const ICON_IDLE: &[u8] = include_bytes!("../icons/tray-idle.png");
const ICON_PENDING: &[u8] = include_bytes!("../icons/tray-pending.png");
const ICON_SUCCESS: &[u8] = include_bytes!("../icons/tray-success.png");
const ICON_FAILURE: &[u8] = include_bytes!("../icons/tray-failure.png");
// Reuses the pending icon as the universal "working" indicator while a
// poll cycle is in flight, regardless of the last aggregate state.
const ICON_REFRESHING: &[u8] = ICON_PENDING;

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let show_dashboard = MenuItem::with_id(
        app,
        "show_dashboard",
        "Open Dashboard",
        true,
        None::<&str>,
    )?;
    let refresh = MenuItem::with_id(app, "refresh", "Refresh", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Driftless", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_dashboard, &refresh, &quit])?;

    let icon = Image::from_bytes(ICON_IDLE)?;

    let _tray = TrayIconBuilder::with_id(TRAY_ICON_ID)
        .icon(icon)
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Driftless")
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show_dashboard" => {
                if let Some(w) = app.get_webview_window("dashboard") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            "refresh" => {
                let state = app.state::<crate::state::SharedState>();
                state.wakeup.notify_one();
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            let app = tray.app_handle();
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_popup(app);
            }
            if let TrayIconEvent::DoubleClick { .. } = event {
                if let Some(w) = app.get_webview_window("dashboard") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            // Forward to positioner so it can track the icon location.
            tauri_plugin_positioner::on_tray_event(app, &event);
        })
        .build(app)?;

    Ok(())
}

fn toggle_popup(app: &AppHandle) {
    let Some(window) = app.get_webview_window("tray-popup") else {
        warn!("no tray-popup window registered");
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }
    if let Err(e) = window.move_window(Position::TrayCenter) {
        warn!(error = %e, "failed to position popup");
    }
    let _ = window.show();
    let _ = window.set_focus();
}

pub fn update_icon(app: &AppHandle, aggregate: AggregateState) {
    let bytes: &[u8] = match aggregate {
        AggregateState::Idle => ICON_IDLE,
        AggregateState::Pending => ICON_PENDING,
        AggregateState::Success => ICON_SUCCESS,
        AggregateState::Failure => ICON_FAILURE,
    };
    set_icon_bytes(app, bytes, format!("{aggregate:?}"));
}

pub fn set_refreshing(app: &AppHandle) {
    set_icon_bytes(app, ICON_REFRESHING, "refreshing".into());
}

fn set_icon_bytes(app: &AppHandle, bytes: &[u8], label: String) {
    let icon = match Image::from_bytes(bytes) {
        Ok(i) => i,
        Err(e) => {
            warn!(error = %e, "failed to decode tray icon");
            return;
        }
    };
    if let Some(tray) = app.tray_by_id(TRAY_ICON_ID) {
        if let Err(e) = tray.set_icon(Some(icon)) {
            warn!(error = %e, "failed to update tray icon");
        } else {
            debug!(state = %label, "tray icon updated");
        }
    }
}
