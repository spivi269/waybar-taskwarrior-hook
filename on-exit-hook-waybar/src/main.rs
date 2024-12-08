use chrono::{DateTime, Local, Utc};
use log::{error, info, warn};
use procfs::process::{all_processes, Process};
use serde::{Deserialize, Serialize};
use simplelog::*;
use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::PathBuf,
    process::Command,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TaskHookWaybarError {
    #[error("Failed to determine cache directory")]
    SetLoggerError(#[from] log::SetLoggerError),
    #[error("File error: {0}")]
    FileError(#[from] std::io::Error),
    #[error("Error: No processes found")]
    ProcessNotFoundError,
    #[error("Process error: {0}")]
    ProcError(#[from] procfs::ProcError),
    #[error("Signal out of bounds: {0}")]
    InvalidRTSignalError(#[from] InvalidRTSignalError),
    #[error("Json processing error: {0}")]
    JsonError(#[from] serde_json::Error),
}

#[derive(Error, Debug)]
pub enum InvalidRTSignalError {
    #[error("Signal below minimum: {context}")]
    BelowMinError { context: String },
    #[error("Signal above maximum: {context}")]
    AboveMaxError { context: String },
}

#[derive(Serialize)]
struct WaybarOutput {
    text: String,
    tooltip: String,
}

#[derive(Deserialize)]
struct Task {
    description: Option<String>,
    priority: Option<String>,
    due: Option<String>,
    urgency: Option<f64>,
}

fn main() {
    let cache_dir = dirs::cache_dir().unwrap_or_else(|| {
        eprintln!("Failed to determine cache directory");
        std::process::exit(1)
    });

    if let Err(e) = setup_logging(&cache_dir.join("waybar-task-hook.log")) {
        eprintln!("Failed to initialize logging: {}", e);
        std::process::exit(1);
    }

    if let Err(e) = run(&cache_dir.join("waybar-tasks.json")) {
        error!("{:?}", e);
        eprintln!("{:?}", e);
        std::process::exit(1);
    }
    println!("Exported to waybar.");
    info!("Export done")
}

fn setup_logging(log_file_path: &PathBuf) -> Result<(), TaskHookWaybarError> {
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

fn run(waybar_json_path: &PathBuf) -> Result<(), TaskHookWaybarError> {
    const PROCESS_NAME: &str = "waybar";
    const SIGNAL_OFFSET: i32 = 8;

    let signal_number = calculate_signal_number(SIGNAL_OFFSET)?;

    let tasks = call_task_export()?;
    let waybar_output = generate_waybar_output(&tasks);
    write_waybar_json(&waybar_output, waybar_json_path)?;

    #[cfg(debug_assertions)]
    print_output(&waybar_output)?;

    send_sigrtmin_plus_n_to_processes_by_name(PROCESS_NAME, signal_number)?;
    info!("Success sending");
    Ok(())
}

fn calculate_signal_number(sig_offset: i32) -> Result<i32, InvalidRTSignalError> {
    if sig_offset < 1 {
        return Err(InvalidRTSignalError::BelowMinError {
            context: format!(
                "Signal SIGRTMIN+{} is too low. Waybar only accepts signals >= SIGRTMIN+1",
                sig_offset
            ),
        });
    }
    let sigrtmax = libc::SIGRTMAX();
    let sig_num = libc::SIGRTMIN() + sig_offset;
    if sig_num > sigrtmax {
        return Err(InvalidRTSignalError::AboveMaxError {
            context: format!(
                "Signal SIGRTMIN+{} ({} + {} = {}) is greater than SIGRTMAX ({})",
                sig_offset,
                libc::SIGRTMIN(),
                sig_offset,
                sig_num,
                sigrtmax
            ),
        });
    }
    Ok(sig_num)
}

fn get_processes_by_name(name: &str) -> Result<Vec<Process>, TaskHookWaybarError> {
    all_processes()?
        .filter_map(|process| match process {
            Ok(proc) => match proc.stat() {
                Ok(stat) if stat.comm == name => Some(Ok(proc)),
                Ok(_) => None,
                Err(e) if proc.stat().map(|s| s.comm == name).unwrap_or(false) => {
                    // Log only if the process could have matched
                    warn!(
                        "Failed to retrieve status for PID {} (matching '{}'): {}",
                        proc.pid(),
                        name,
                        e
                    );
                    Some(Err(e.into()))
                }
                Err(_) => None, // Ignore irrelevant processes
            },
            Err(_) => None, // Ignore errors unrelated to specific processes
        })
        .collect()
}

fn send_signal(pid: i32, sig_num: i32) {
    let result = unsafe { libc::kill(pid, sig_num) };
    if result != 0 {
        warn!(
            "Failed to send signal {} to PID {}: {}",
            sig_num,
            pid,
            std::io::Error::last_os_error()
        );
    }
}

fn send_sigrtmin_plus_n_to_processes_by_name(
    process_name: &str,
    sig_num: i32,
) -> Result<(), TaskHookWaybarError> {
    let processes = get_processes_by_name(process_name)?;
    let processes_len = processes.len();

    if processes_len == 0 {
        return Err(TaskHookWaybarError::ProcessNotFoundError);
    } else {
        info!(
            "Sending signal {} to {} {}",
            sig_num,
            processes_len,
            if processes_len == 1 {
                "process"
            } else {
                "processes"
            }
        );
    }

    processes
        .iter()
        .map(|process| process.pid())
        .for_each(|pid| {
            info!("Sending to PID {}", pid);
            send_signal(pid, sig_num);
        });
    Ok(())
}

fn write_waybar_json(
    output: &WaybarOutput,
    json_path: &PathBuf,
) -> Result<(), TaskHookWaybarError> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(json_path)?;

    info!("Opened file at {}", json_path.display());

    let mut writer = BufWriter::new(file);
    let json_output = serde_json::to_string(output)?;

    writeln!(writer, "{}", json_output)?;

    info!("Json written to file");

    Ok(())
}

fn call_task_export() -> Result<Vec<Task>, TaskHookWaybarError> {
    let output = Command::new("task")
        .arg("rc.hooks:off")
        .arg("status:pending")
        .arg("export")
        .output()?;

    let json_output = String::from_utf8_lossy(&output.stdout);
    let mut tasks: Vec<Task> = serde_json::from_str(&json_output)?;
    tasks.sort_by(|a, b| {
        b.urgency
            .partial_cmp(&a.urgency)
            .unwrap_or(std::cmp::Ordering::Less)
    });
    Ok(tasks)
}

fn generate_waybar_output(tasks: &[Task]) -> WaybarOutput {
    if let Some(most_urgent) = tasks.first() {
        let tooltip = tasks
            .iter()
            .map(Task::construct_task_output)
            .collect::<Vec<_>>()
            .join("\n");

        WaybarOutput {
            text: most_urgent.construct_task_output(),
            tooltip,
        }
    } else {
        WaybarOutput {
            text: "No tasks.".to_string(),
            tooltip: "No tasks.".to_string(),
        }
    }
}

impl Task {
    fn construct_task_output(&self) -> String {
        let mut parts = Vec::new();

        if let Some(description) = &self.description {
            parts.push(description.clone());
        }
        if let Some(priority) = &self.priority {
            parts.push(format!("Prio: {}", priority));
        }
        if let Some(due) = &self.due {
            if let Ok(datetime) = parse_due_date(due) {
                parts.push(format!("Due: {}", datetime.format("%a, %y-%m-%d %H:%M")));
            }
        }
        if let Some(urgency) = self.urgency {
            parts.push(format!("Urgency: {:.2}", urgency));
        }

        parts.join(", ")
    }
}

fn parse_due_date(due: &str) -> Result<DateTime<Local>, chrono::ParseError> {
    let due_formatted = format!(
        "{}-{}-{}T{}:{}:{}+00:00",
        &due[0..4],   // Year
        &due[4..6],   // Month
        &due[6..8],   // Day
        &due[9..11],  // Hour
        &due[11..13], // Minute
        &due[13..15]  // Second
    );

    let datetime = DateTime::parse_from_rfc3339(&due_formatted)?;
    Ok(datetime.with_timezone(&Local))
}

#[cfg(debug_assertions)]
fn print_output(output: &WaybarOutput) -> Result<(), serde_json::Error> {
    let json_output = serde_json::to_string_pretty(output)?;
    println!("{}", json_output);
    Ok(())
}

/**************
 * Unit tests *
 * ***********/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_signal_number_valid() {
        let result = calculate_signal_number(8);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), libc::SIGRTMIN() + 8)
    }

    #[test]
    fn test_calculate_signal_number_below_min() {
        let result = calculate_signal_number(0);
        assert!(matches!(
            result,
            Err(InvalidRTSignalError::BelowMinError { .. })
        ));
    }

    #[test]
    fn test_calculate_signal_number_negative_offset() {
        let result = calculate_signal_number(-500);
        assert!(matches!(
            result,
            Err(InvalidRTSignalError::BelowMinError { .. })
        ));
    }

    #[test]
    fn test_calculate_signal_number_above_max() {
        let result = calculate_signal_number(50);
        assert!(matches!(
            result,
            Err(InvalidRTSignalError::AboveMaxError { .. })
        ));
    }

    #[test]
    fn test_parse_due_date_valid() {
        let due = "20241206T143002Z";
        let result = parse_due_date(due);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().format("%a, %y-%m-%d %H:%M").to_string(),
            "Fri, 24-12-06 15:30"
        );
    }
}
