use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, PhysicalPosition, Rect};
use tracing::{debug, info, warn};

use crate::state::AggregateState;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

// Matches the configured logical width of the tray-popup window in
// tauri.conf.json. The popup is non-resizable so this stays in sync as long as
// the config does.
const POPUP_LOGICAL_WIDTH: f64 = 360.0;

const TRAY_ICON_ID: &str = "main-tray";

const ICON_IDLE: &[u8] = include_bytes!("../../icons/icon-idle.png");
const ICON_PENDING: &[u8] = include_bytes!("../../icons/icon-pending.png");
const ICON_SUCCESS: &[u8] = include_bytes!("../../icons/icon-success.png");
const ICON_FAILURE: &[u8] = include_bytes!("../../icons/icon-failed.png");
// Reuses the pending icon as the universal "working" indicator while a
// poll cycle is in flight, regardless of the last aggregate state.
const ICON_REFRESHING: &[u8] = ICON_PENDING;

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let version_label = MenuItem::with_id(
        app,
        "version",
        format!("Driftless v{APP_VERSION}"),
        false,
        None::<&str>,
    )?;
    let sep_top = PredefinedMenuItem::separator(app)?;
    let show_dashboard = MenuItem::with_id(
        app,
        "show_dashboard",
        "Open Dashboard",
        true,
        None::<&str>,
    )?;
    let refresh = MenuItem::with_id(app, "refresh", "Refresh", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Driftless", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&version_label, &sep_top, &show_dashboard, &refresh, &quit],
    )?;

    let icon = Image::from_bytes(ICON_IDLE)?;

    let _tray = TrayIconBuilder::with_id(TRAY_ICON_ID)
        .icon(icon)
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip(format!("Driftless v{APP_VERSION}"))
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
            debug!(?event, "tray event");

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
                toggle_popup(app, rect);
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

fn toggle_popup(app: &AppHandle, tray_rect: &Rect) {
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
    // Self-position rather than going through tauri-plugin-positioner.
    // The plugin's TrayCenter calls `window.current_monitor()?.unwrap()`
    // unconditionally, which panics for a hidden popup that has no monitor
    // binding yet — the exact crash hit when clicking the tray on a different
    // monitor than the popup was last shown on (especially across mixed
    // scale factors, e.g. built-in Retina vs external).
    let probe_scale = window.scale_factor().unwrap_or(1.0);
    let tray_pos = tray_rect.position.to_physical::<i32>(probe_scale);
    let tray_size = tray_rect.size.to_physical::<i32>(probe_scale);
    let tray_center_x = tray_pos.x as f64 + tray_size.width as f64 / 2.0;
    let tray_top_y = tray_pos.y as f64;

    // Look up the tray icon's monitor to get its actual scale factor — the
    // popup width is configured in logical units, and we need physical pixels
    // of the destination display for the centering math.
    let target_scale = match window.monitor_from_point(tray_center_x, tray_top_y) {
        Ok(Some(m)) => m.scale_factor(),
        Ok(None) => {
            warn!("monitor_from_point returned None; falling back to popup scale");
            probe_scale
        }
        Err(e) => {
            warn!(error = %e, "monitor_from_point failed; falling back to popup scale");
            probe_scale
        }
    };

    let popup_phys_width = (POPUP_LOGICAL_WIDTH * target_scale).round() as i32;
    let x = tray_pos.x + tray_size.width / 2 - popup_phys_width / 2;
    let y = tray_pos.y + tray_size.height;
    debug!(x, y, target_scale, "positioning popup below tray icon");

    if let Err(e) = window.set_position(PhysicalPosition::new(x, y)) {
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
