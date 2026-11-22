use std::{env, process::ExitCode};

use kitty_media::task_queue::TaskQueueDatabase;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub const EXIT_CODE_CONFIG_ERROR: u8 = 1;
pub const EXIT_CODE_DATABASE_ERROR: u8 = 2;

#[tokio::main]
async fn main() -> ExitCode {
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

    let connection_string = match env::var("KITTY_MEDIA_DATABASE_URL") {
        Ok(url) => url,
        Err(e) => {
            tracing::error!("Failed to read KITTY_MEDIA_DATABASE_URL: {}", e);
            return ExitCode::from(EXIT_CODE_CONFIG_ERROR);
        }
    };

    let _ = match TaskQueueDatabase::new(&connection_string).await {
        Ok(db) => db,
        Err(e) => {
            tracing::error!("Failed to connect to task queue database: {}", e);
            return ExitCode::from(EXIT_CODE_DATABASE_ERROR);
        }
    };

    ExitCode::SUCCESS
}
