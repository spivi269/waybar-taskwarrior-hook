use crate::errors::{InvalidRTSignalError, TaskHookWaybarError};
use log::{info, warn};
use procfs::process::{all_processes, Process};

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

pub fn send_offset_signal_to_process_by_name(
    process_name: &str,
    offset_from_sigrtmin: i32,
) -> Result<(), TaskHookWaybarError> {
    send_signal_to_processes_by_name(process_name, calculate_signal_number(offset_from_sigrtmin)?)
}

pub fn send_signal_to_processes_by_name(
    process_name: &str,
    sig_num: i32,
) -> Result<(), TaskHookWaybarError> {
    let processes = get_processes_by_name(process_name)?;
    let processes_len = processes.len();

    if processes_len == 0 {
        return Err(TaskHookWaybarError::ProcessNotFound);
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
