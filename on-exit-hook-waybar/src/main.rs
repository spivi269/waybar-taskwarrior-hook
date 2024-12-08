mod errors;
mod signals;
mod task;
mod utils;

use crate::errors::TaskHookWaybarError;
use crate::signals::*;
use crate::task::write_waybar_json;
use crate::utils::setup_logging;
use log::{error, info};
use std::path::PathBuf;
use task::generate_waybar_output_from_task_export;

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

fn run(waybar_json_path: &PathBuf) -> Result<(), TaskHookWaybarError> {
    const PROCESS_NAME: &str = "waybar";
    const SIGNAL_OFFSET: i32 = 8;

    let waybar_output = generate_waybar_output_from_task_export()?;
    write_waybar_json(&waybar_output, waybar_json_path)?;

    #[cfg(debug_assertions)]
    crate::task::debug::print_output(&waybar_output)?;

    send_offset_signal_to_process_by_name(PROCESS_NAME, SIGNAL_OFFSET)?;
    info!("Success sending");
    Ok(())
}

//
//     #[test]
//     fn test_parse_due_date_valid() {
//         let due = "20241206T143002Z";
//         let result = parse_due_date(due);
//         assert!(result.is_ok());
//         assert_eq!(
//             result.unwrap().format("%a, %y-%m-%d %H:%M").to_string(),
//             "Fri, 24-12-06 15:30"
//         );
//     }
// }
