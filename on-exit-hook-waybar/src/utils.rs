use crate::errors::TaskHookWaybarError;
use chrono::{Local, Utc};
use log::info;
use simplelog::*;
use std::fs::File;
use std::path::PathBuf;

pub fn setup_logging(log_file_path: &PathBuf) -> Result<(), TaskHookWaybarError> {
    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Error,
            Config::default(),
            TerminalMode::Stderr,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Info,
            Config::default(),
            File::create(log_file_path)?,
        ),
    ])?;

    let time_zone = if Utc::now().timestamp() == Local::now().timestamp() {
        "UTC"
    } else {
        "Local Time"
    };

    info!(
        "Logging initialized, writing to {}",
        log_file_path.display()
    );
    info!("Log file time zone: {}", time_zone);
    Ok(())
}
