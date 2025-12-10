//! Process management utilities.

use std::process::Command;

/// Kill any previously running omniclip instances.
///
/// This prevents port conflicts when restarting the service.
pub fn kill_previous_instances() {
    let my_pid = std::process::id();

    // Use pkill to kill other omniclip processes
    let _ = Command::new("pkill")
        .args(["-9", "-f", "target/.*omniclip"])
        .output();

    // Kill by process name, excluding our PID
    if let Ok(output) = Command::new("pgrep").args(["-f", "omniclip"]).output() {
        let pids = String::from_utf8_lossy(&output.stdout);
        for pid_str in pids.lines() {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                if pid != my_pid {
                    let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
                }
            }
        }
    }

    // Brief pause to let the OS release the port
    std::thread::sleep(std::time::Duration::from_millis(100));
}
