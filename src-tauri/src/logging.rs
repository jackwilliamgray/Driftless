use std::backtrace::Backtrace;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::{AppHandle, Manager};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

const LOG_FILE_NAME: &str = "driftless.log";

pub fn log_dir(app: &AppHandle) -> tauri::Result<PathBuf> {
    app.path().app_log_dir()
}

/// Initialize the global tracing subscriber. If `debug_logging` is enabled, a
/// file appender at `${app_log_dir}/driftless.log` is added alongside stderr
/// and the level is bumped to `debug`. A panic hook is always installed so
/// crashes get captured to the same file when debug logging is on.
pub fn init(log_dir: &Path, debug_logging: bool) {
    let env_default = if debug_logging {
        "debug,driftless_lib=debug"
    } else {
        "info,driftless_lib=info"
    };
    let env_filter = EnvFilter::try_from_env("DRIFTLESS_LOG")
        .unwrap_or_else(|_| EnvFilter::new(env_default));

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_writer(std::io::stderr);

    let file_path = log_dir.join(LOG_FILE_NAME);
    let file_writer = if debug_logging {
        if let Err(e) = std::fs::create_dir_all(log_dir) {
            eprintln!("failed to create log dir {log_dir:?}: {e}");
        }
        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
        {
            Ok(f) => Some(Mutex::new(f)),
            Err(e) => {
                eprintln!("failed to open log file {file_path:?}: {e}");
                None
            }
        }
    } else {
        None
    };

    let init_result = if let Some(file_writer) = file_writer {
        let file_layer = tracing_subscriber::fmt::layer()
            .with_target(false)
            .with_ansi(false)
            .with_writer(file_writer);
        tracing_subscriber::registry()
            .with(env_filter)
            .with(stderr_layer)
            .with(file_layer)
            .try_init()
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(stderr_layer)
            .try_init()
    };

    if let Err(e) = init_result {
        eprintln!("tracing subscriber already initialised: {e}");
    }

    install_panic_hook(file_path, debug_logging);
}

fn install_panic_hook(log_path: PathBuf, debug_logging: bool) {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_hook(info);

        if !debug_logging {
            return;
        }

        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let message = info
            .payload()
            .downcast_ref::<&'static str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<non-string panic payload>".to_string());

        let backtrace = Backtrace::force_capture();
        let now = chrono::Utc::now().to_rfc3339();

        let mut file = match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            Ok(f) => f,
            Err(e) => {
                eprintln!("panic hook: failed to open log file {log_path:?}: {e}");
                return;
            }
        };

        let _ = writeln!(
            file,
            "[{now}] PANIC at {location}\n  message: {message}\n  thread: {}\n  backtrace:\n{backtrace}",
            std::thread::current().name().unwrap_or("<unnamed>")
        );
        let _ = file.sync_all();
    }));
}
