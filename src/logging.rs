use std::sync::LazyLock;

use color_eyre::Result;
use tracing_subscriber::{filter, fmt, prelude::*, EnvFilter};

use crate::config;
use crate::logger_widget::{init_tui_logger, TuiTracingSubscriber};

static LOG_FILE: LazyLock<String> = LazyLock::new(|| format!("{}.log", env!("CARGO_PKG_NAME")));

pub fn init() -> Result<()> {
    let directory = config::get_data_dir();
    std::fs::create_dir_all(directory.clone())?;

    let log_path = directory.join(LOG_FILE.clone());
    let log_file = std::fs::File::create(log_path)?;

    init_tui_logger(50);

    let file_subscriber = fmt::layer()
        .with_file(true)
        .with_line_number(true)
        .with_writer(log_file)
        .with_target(false)
        .with_ansi(false)
        .with_filter(EnvFilter::from_default_env());

    // Filter out logs from dependencies
    let filter = filter::EnvFilter::builder()
        .with_default_directive(filter::LevelFilter::DEBUG.into())
        .from_env()?
        .add_directive("panic=error".parse()?);

    tracing_subscriber::registry()
        .with(file_subscriber)
        .with(TuiTracingSubscriber)
        .with(filter)
        .try_init()?;

    Ok(())
}
