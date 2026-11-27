#![warn(missing_docs, clippy::missing_docs_in_private_items)]
#![doc = include_str!("../README.md")]
use std::{env, process::ExitCode};

use kitty_media::task_queue::TaskQueueDatabase;

/// Exit code for configuration errors.
pub const EXIT_CODE_CONFIG_ERROR: u8 = 1;
/// Exit code for database errors.
pub const EXIT_CODE_DATABASE_ERROR: u8 = 2;

#[cfg(feature = "logger")]
/// Initializes the logger using the [`tracing`] crate.
fn init_logger() {
    use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::try_from_env("KITTY_MEDIA_LOG").unwrap_or(EnvFilter::new(
                if cfg!(debug_assertions) {
                    "kitty_media=trace"
                } else {
                    "kitty_media=info"
                },
            )),
        )
        .init();
}

/// The main entry point of the application.
#[tokio::main]
async fn main() -> ExitCode {
    #[cfg(feature = "logger")]
    init_logger();

    let connection_string = match env::var("KITTY_MEDIA_DATABASE_URL") {
        Ok(url) => url,
        Err(e) => {
            tracing::error!("Failed to read KITTY_MEDIA_DATABASE_URL: {e}");
            return ExitCode::from(EXIT_CODE_CONFIG_ERROR);
        }
    };

    let machine_id = match env::var("KITTY_MEDIA_MACHINE_ID").map(|v| v.parse::<u16>()) {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            tracing::error!("Failed to parse KITTY_MEDIA_MACHINE_ID: {e}");
            return ExitCode::from(EXIT_CODE_CONFIG_ERROR);
        }
        Err(_) => {
            tracing::warn!("KITTY_MEDIA_MACHINE_ID not provided, defaulting to 0");
            0
        }
    };

    let max_attempts = match env::var("KITTY_MEDIA_MAX_ATTEMPTS").map(|v| v.parse::<i8>()) {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            tracing::error!("Failed to parse KITTY_MEDIA_MAX_ATTEMPTS: {e}");
            return ExitCode::from(EXIT_CODE_CONFIG_ERROR);
        }
        Err(_) => 3,
    };

    let _ = match TaskQueueDatabase::new(&connection_string, machine_id, max_attempts).await {
        Ok(db) => db,
        Err(e) => {
            tracing::error!("Failed to initialize task queue database: {e}");
            return ExitCode::from(EXIT_CODE_DATABASE_ERROR);
        }
    };

    ExitCode::SUCCESS
}
