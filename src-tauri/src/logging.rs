//! Logging to a daily rotating file in `%APPDATA%\Deckless\logs` plus stderr in debug builds.

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{Builder, Rotation};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::paths;

/// Initializes the global subscriber. Keep the returned guard alive for the whole process.
pub fn init() -> Option<WorkerGuard> {
    let _ = paths::ensure_dirs();
    let filter = EnvFilter::try_from_env("DECKLESS_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info,tao=warn,wry=warn,tungstenite=warn"));

    let appender = Builder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix("deckless")
        .filename_suffix("log")
        .max_log_files(7)
        .build(paths::logs_dir());

    let (file_layer, guard) = match appender {
        Ok(appender) => {
            let (writer, guard) = tracing_appender::non_blocking(appender);
            (
                Some(fmt::layer().with_ansi(false).with_writer(writer)),
                Some(guard),
            )
        }
        Err(_) => (None, None),
    };

    let stderr_layer = cfg!(debug_assertions).then(|| fmt::layer().with_writer(std::io::stderr));

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stderr_layer)
        .try_init();

    install_panic_hook();
    guard
}

/// Logs panics from any thread (SPEC 10.3). Threads that must survive panics
/// (input hooks) additionally wrap their work in `catch_unwind`.
fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("<unnamed>");
        let bt = std::backtrace::Backtrace::force_capture();
        tracing::error!(thread = name, "panic: {info}\n{bt}");
        default(info);
    }));
}
