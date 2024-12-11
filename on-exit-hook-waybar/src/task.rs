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

#[derive(Serialize, Debug, PartialEq)]
pub struct WaybarOutput {
    text: String,
    tooltip: String,
}

#[derive(Deserialize, Debug, PartialEq)]
struct Task {
    id: u32,
    description: Option<String>,
    priority: Option<String>,
    due: Option<String>,
    urgency: Option<f64>,
}

impl Task {
    fn construct_task_output(&self) -> String {
        let parts: Vec<_> = [
            self.description.as_deref().map(String::from),
            self.priority.as_ref().map(|p| format!("Prio: {}", p)),
            self.due.as_ref().and_then(|d| {
                parse_due_date(d)
                    .ok()
                    .map(|datetime| format!("Due: {}", datetime.format("%a, %y-%m-%d %H:%M")))
            }),
            self.urgency.map(|u| format!("Urgency: {:.2}", u)),
        ]
        .into_iter()
        .flatten()
        .collect();

        [self.id.to_string(), parts.join(", ")].join(" ")
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

    sort_tasks(&mut tasks);

    Ok(tasks)
}

fn sort_tasks(tasks: &mut [Task]) -> &mut [Task] {
    tasks.sort_unstable_by(|a, b| {
        b.urgency
            .partial_cmp(&a.urgency)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| compare_optional_timestamps(a.due.as_deref(), b.due.as_deref()))
            .then_with(|| a.id.cmp(&b.id))
    });
    tasks
}

fn compare_optional_timestamps(a: Option<&str>, b: Option<&str>) -> std::cmp::Ordering {
    a.and_then(|s| parse_due_date(s).ok())
        .cmp(&b.and_then(|s| parse_due_date(s).ok()))
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

#[cfg(test)]
pub mod tests {
    use super::*;

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

    #[test]
    fn test_generate_valid_waybar_output() {
        let waybar_output = generate_waybar_output(&[
            Task {
                id: 1,
                description: Some("Test1".to_string()),
                priority: Some("H".to_string()),
                due: Some("20241206T143002Z".to_string()),
                urgency: Some(42.0),
            },
            Task {
                id: 2,
                description: Some("Test2".to_string()),
                priority: Some("M".to_string()),
                due: Some("20241206T173002Z".to_string()),
                urgency: Some(5.0),
            },
        ]);

        assert_eq!(
            waybar_output,
            WaybarOutput {
                text: "1 Test1, Prio: H, Due: Fri, 24-12-06 15:30, Urgency: 42.00".to_string(),
                tooltip: "1 Test1, Prio: H, Due: Fri, 24-12-06 15:30, Urgency: 42.00\n2 Test2, Prio: M, Due: Fri, 24-12-06 18:30, Urgency: 5.00".to_string()
            }
        );
    }

    #[test]
    fn test_generate_empty_tasks_waybar_output() {
        let waybar_output = generate_waybar_output(&[]);

        assert_eq!(
            waybar_output,
            WaybarOutput {
                text: "No tasks.".to_string(),
                tooltip: "No tasks.".to_string()
            }
        );
    }

    #[test]
    fn test_sort_tasks() {
        let mut tasks = vec![
            Task {
                id: 1,
                description: Some("First task".to_string()),
                priority: Some("H".to_string()),
                due: Some("20241206T143002Z".to_string()),
                urgency: Some(3.0),
            },
            Task {
                id: 2,
                description: Some("Second task".to_string()),
                priority: Some("M".to_string()),
                due: Some("20241205T143002Z".to_string()),
                urgency: Some(5.0),
            },
            Task {
                id: 3,
                description: Some("Third task".to_string()),
                priority: Some("L".to_string()),
                due: Some("20241207T143002Z".to_string()),
                urgency: None,
            },
            Task {
                id: 4,
                description: Some("Fourth task".to_string()),
                priority: None,
                due: None,
                urgency: Some(2.0),
            },
            Task {
                id: 5,
                description: Some("Fifth task".to_string()),
                priority: None,
                due: Some("20231205T143002Z".to_string()),
                urgency: Some(5.0),
            },
            Task {
                id: 6,
                description: Some("Sixth task".to_string()),
                priority: None,
                due: Some("20231205T143002Z".to_string()),
                urgency: Some(5.0),
            },
        ];

        sort_tasks(&mut tasks);

        let expected = vec![
            Task {
                id: 5,
                description: Some("Fifth task".to_string()),
                priority: None,
                due: Some("20231205T143002Z".to_string()),
                urgency: Some(5.0),
            },
            Task {
                id: 6,
                description: Some("Sixth task".to_string()),
                priority: None,
                due: Some("20231205T143002Z".to_string()),
                urgency: Some(5.0),
            },
            Task {
                id: 2,
                description: Some("Second task".to_string()),
                priority: Some("M".to_string()),
                due: Some("20241205T143002Z".to_string()),
                urgency: Some(5.0),
            },
            Task {
                id: 1,
                description: Some("First task".to_string()),
                priority: Some("H".to_string()),
                due: Some("20241206T143002Z".to_string()),
                urgency: Some(3.0),
            },
            Task {
                id: 4,
                description: Some("Fourth task".to_string()),
                priority: None,
                due: None,
                urgency: Some(2.0),
            },
            Task {
                id: 3,
                description: Some("Third task".to_string()),
                priority: Some("L".to_string()),
                due: Some("20241207T143002Z".to_string()),
                urgency: None,
            },
        ];

        assert_eq!(tasks, expected);
    }
}
