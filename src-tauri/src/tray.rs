use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, PhysicalPosition};
use tauri_plugin_positioner::{Position, WindowExt};
use tracing::{debug, info, warn};

use crate::state::AggregateState;

const TRAY_ICON_ID: &str = "main-tray";

const ICON_IDLE: &[u8] = include_bytes!("../../icons/icon-idle.png");
const ICON_PENDING: &[u8] = include_bytes!("../../icons/icon-pending.png");
const ICON_SUCCESS: &[u8] = include_bytes!("../../icons/icon-success.png");
const ICON_FAILURE: &[u8] = include_bytes!("../../icons/icon-failed.png");
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
            // Forward to positioner first so its rect cache is current before
            // we ask it to position the popup. Suspected source of intermittent
            // multi-monitor crashes — log generously so any panic from inside
            // the plugin leaves a breadcrumb in the log file.
            debug!(?event, "tray event");
            tauri_plugin_positioner::on_tray_event(app, &event);

            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                position,
                rect,
                ..
            } = &event
            {
                info!(
                    click_x = position.x,
                    click_y = position.y,
                    rect = ?rect,
                    "tray left-click"
                );
                toggle_popup(app, *position);
            }
            if let TrayIconEvent::DoubleClick { .. } = &event {
                debug!("tray double-click → opening dashboard");
                if let Some(w) = app.get_webview_window("dashboard") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

fn toggle_popup(app: &AppHandle, click_pos: PhysicalPosition<f64>) {
    let Some(window) = app.get_webview_window("tray-popup") else {
        warn!("no tray-popup window registered");
        return;
    };
    match window.is_visible() {
        Ok(true) => {
            debug!("popup visible → hiding");
            if let Err(e) = window.hide() {
                warn!(error = %e, "failed to hide popup");
            }
            return;
        }
        Ok(false) => debug!("popup hidden → positioning & showing"),
        Err(e) => warn!(error = %e, "is_visible() failed; assuming hidden"),
    }
    // Seed the popup onto the tray's monitor before asking the positioner
    // plugin to compute TrayCenter. tauri-plugin-positioner unconditionally
    // calls `window.current_monitor()?.unwrap()`, which panics for a hidden
    // window that has no monitor binding — e.g. when the tray click happens
    // on a different display than the popup was last shown on.
    let seed = PhysicalPosition::new(click_pos.x as i32, click_pos.y as i32);
    if let Err(e) = window.set_position(seed) {
        warn!(error = %e, "failed to seed popup position");
    }
    if let Err(e) = window.move_window(Position::TrayCenter) {
        warn!(error = %e, "failed to position popup");
    }
    if let Err(e) = window.show() {
        warn!(error = %e, "failed to show popup");
        return;
    }
    if let Err(e) = window.set_focus() {
        warn!(error = %e, "failed to focus popup");
    }
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
