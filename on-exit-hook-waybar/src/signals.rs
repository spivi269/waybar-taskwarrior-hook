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
    Ok(all_processes()?
        .filter_map(Result::ok)
        .filter(|p| p.stat().is_ok_and(|s| s.comm == name))
        .collect())
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

/**************
 * Unit tests *
 **************/

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
    fn test_retrieve_valid_processes() {
        let procs = get_processes_by_name("cargo");
        assert!(procs.is_ok());
        let procs = procs.unwrap();
        assert!(!procs.is_empty());
    }
}
