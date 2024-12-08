use crate::errors::TaskHookWaybarError;
use chrono::{DateTime, Local};
use log::info;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::{
    fs::OpenOptions,
    io::{BufWriter, Write},
    path::PathBuf,
};

#[derive(Serialize)]
pub struct WaybarOutput {
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

pub fn generate_waybar_output_from_task_export() -> Result<WaybarOutput, TaskHookWaybarError> {
    Ok(generate_waybar_output(&call_task_export()?))
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

pub fn write_waybar_json(
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

#[cfg(debug_assertions)]
pub mod debug {
    use super::WaybarOutput;
    pub fn print_output(output: &WaybarOutput) -> Result<(), serde_json::Error> {
        let json_output = serde_json::to_string_pretty(output)?;
        println!("{}", json_output);
        Ok(())
    }
}
