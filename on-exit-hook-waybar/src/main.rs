use anyhow::{bail, Context, Result};
use chrono::{DateTime, Local};
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
    if let Err(e) = setup_logging() {
        eprintln!("Failed to initialize logging: {}", e);
        std::process::exit(1);
    }

    if let Err(e) = run() {
        error!("{:?}", e);
        eprintln!("{:?}", e);
        std::process::exit(1);
    }
    println!("Exported to waybar.");
    info!("Export done")
}

fn get_cache_dir() -> Result<PathBuf> {
    dirs::cache_dir().context("Could not determine cache directory")
}

fn setup_logging() -> Result<()> {
    let log_file_path = get_cache_dir()?.join("waybar-task-hook.log");
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
            File::create(&log_file_path).with_context(|| {
                format!("Could not create log file at {}", log_file_path.display())
            })?,
        ),
    ])?;
    info!(
        "Logging initialized, writing to {}",
        log_file_path.display()
    );
    Ok(())
}

fn run() -> Result<()> {
    const PROCESS_NAME: &str = "waybar";
    const SIGNAL_OFFSET: i32 = 8;

    let signal_number = calculate_signal_number(SIGNAL_OFFSET)?;

    let cache_dir = get_cache_dir()?;
    let waybar_json_path = cache_dir.join("waybar-tasks.json");

    let tasks = call_task_export()?;
    let waybar_output = generate_waybar_output(&tasks);
    write_waybar_json(&waybar_output, &waybar_json_path)?;

    #[cfg(debug_assertions)]
    print_output(&waybar_output)?;

    let _ = send_sigrtmin_plus_n_to_processes_by_name(PROCESS_NAME, signal_number)
        .context("Failed to send signals")?;
    Ok(())
}

fn calculate_signal_number(sig_offset: i32) -> Result<i32> {
    if sig_offset < 1 {
        bail!(
            "Trying to use SIGRTMIN + {}, but waybar only accepts signals >= SIGRTMIN+1",
            sig_offset
        );
    }
    let sigrtmax = libc::SIGRTMAX();
    let sig_num = libc::SIGRTMIN() + sig_offset;
    if sig_num > sigrtmax {
        bail!(
            "Signal number to send ({}) is greater than SIGRTMAX {}",
            sig_num,
            sigrtmax
        );
    }
    Ok(sig_num)
}

fn get_processes_by_name(name: &str) -> Result<Vec<Process>> {
    all_processes()
        .context("Failed to list processes from /proc")?
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

fn send_signal(pid: i32, sig_num: i32) -> Result<()> {
    let result = unsafe { libc::kill(pid, sig_num) };
    if result != 0 {
        warn!(
            "Failed to send signal {} to PID {}: {}",
            sig_num,
            pid,
            std::io::Error::last_os_error()
        );
    }
    Ok(())
}

fn send_sigrtmin_plus_n_to_processes_by_name(process_name: &str, sig_num: i32) -> Result<()> {
    for process in get_processes_by_name(process_name)? {
        let pid = process.pid();
        info!("Sending signal {} to PID {}", sig_num, pid);
        send_signal(pid, sig_num)
            .with_context(|| format!("Failed to send signal to PID {}", pid))?;
    }
    Ok(())
}

fn write_waybar_json(output: &WaybarOutput, json_path: &PathBuf) -> Result<()> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&json_path)
        .with_context(|| format!("Failed to open json file at {}", json_path.display()))?;

    info!("Opened file at {}", json_path.display());

    let mut writer = BufWriter::new(file);
    let json_output = serde_json::to_string(output)?;

    writeln!(writer, "{}", json_output)
        .with_context(|| format!("Error writing to file {}", json_path.display()))?;

    info!("Json written to file");

    Ok(())
}

fn call_task_export() -> Result<Vec<Task>> {
    let output = Command::new("task")
        .arg("rc.hooks:off")
        .arg("status:pending")
        .arg("export")
        .output()
        .with_context(|| format!("Failed to execute task export"))?;

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
                parts.push(format!("Due: {}", datetime.format("%y-%m-%d %H:%M")));
            }
        }
        if let Some(urgency) = self.urgency {
            parts.push(format!("Urgency: {:.2}", urgency));
        }

        parts.join(", ")
    }
}

fn parse_due_date(due: &str) -> Result<DateTime<Local>> {
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
fn print_output(output: &WaybarOutput) -> Result<()> {
    let json_output = serde_json::to_string_pretty(output)?;
    println!("{}", json_output);
    Ok(())
}
