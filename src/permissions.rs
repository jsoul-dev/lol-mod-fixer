//! Windows permissions and elevation utilities for unlocking restricted mod directories.

use std::path::Path;
use tracing::debug;

/// Check if the current process is running with elevated (Administrator) privileges on Windows.
pub fn is_elevated() -> bool {
    #[cfg(windows)]
    {
        let output = std::process::Command::new("whoami").arg("/groups").output();
        if let Ok(out) = output {
            let text = String::from_utf8_lossy(&out.stdout);
            // S-1-16-12288: High Mandatory Level (Elevated Administrator)
            // S-1-16-16384: System Mandatory Level
            return text.contains("S-1-16-12288") || text.contains("S-1-16-16384");
        }
    }
    false
}

/// Check if an error message string indicates a permission denied or file locking error.
pub fn is_lock_or_permission_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("os error 5")
        || lower.contains("access is denied")
        || lower.contains("os error 32")
        || lower.contains("used by another process")
        || lower.contains("sharing violation")
        || lower.contains("permission denied")
        || lower.contains("locked")
}

#[cfg(windows)]
fn run_command_with_timeout(
    mut cmd: std::process::Command,
    timeout: std::time::Duration,
) -> bool {
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => return false,
    };

    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) => {
                if start.elapsed() >= timeout {
                    tracing::warn!("Permission unlock command timed out after {:?}", timeout);
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

/// Automatically unlock NTFS permissions on a directory or file.
///
/// Takes ownership and grants Full Control to BUILTIN\Users and Administrators,
/// resetting inherited permissions so that restricted folders extracted by Rose
/// or other elevated processes become fully accessible.
pub fn unlock_folder_permissions(folder: &Path) -> bool {
    #[cfg(windows)]
    {
        let folder_str = folder.to_string_lossy();
        debug!(
            "Attempting to unlock NTFS permissions on '{}'...",
            folder_str
        );

        let timeout = std::time::Duration::from_secs(5);
        let is_dir = folder.is_dir();

        // Step 1: Take ownership
        let mut takeown_cmd = std::process::Command::new("takeown");
        takeown_cmd.arg("/F").arg(&*folder_str);
        if is_dir {
            takeown_cmd.args(["/R", "/D", "Y"]);
        }
        let takeown_ok = run_command_with_timeout(takeown_cmd, timeout);

        // Step 2: Grant BUILTIN\Users (*S-1-5-32-545) and Administrators (*S-1-5-32-544) Full Control
        let mut icacls_cmd = std::process::Command::new("icacls");
        icacls_cmd.arg(&*folder_str);
        icacls_cmd.args(["/grant", "*S-1-5-32-545:(OI)(CI)F"]);
        if is_dir {
            icacls_cmd.args(["/T", "/C", "/Q"]);
        } else {
            icacls_cmd.args(["/C", "/Q"]);
        }
        let grant_ok = run_command_with_timeout(icacls_cmd, timeout);

        // Step 3: Reset inheritance
        let mut reset_cmd = std::process::Command::new("icacls");
        reset_cmd.arg(&*folder_str);
        if is_dir {
            reset_cmd.args(["/reset", "/T", "/C", "/Q"]);
        } else {
            reset_cmd.args(["/reset", "/C", "/Q"]);
        }
        let _ = run_command_with_timeout(reset_cmd, timeout);

        let ok = takeown_ok || grant_ok;

        if ok {
            debug!("Successfully unlocked permissions on '{}'", folder_str);
        } else {
            debug!(
                "Could not unlock permissions on '{}' (elevation may be required)",
                folder_str
            );
        }
        ok
    }

    #[cfg(not(windows))]
    false
}

/// Attempt to re-launch the current executable with elevated Administrator privileges via UAC.
pub fn try_self_elevate() -> bool {
    #[cfg(windows)]
    {
        let current_exe = match std::env::current_exe() {
            Ok(e) => e,
            Err(_) => return false,
        };

        let mut elevated_args: Vec<String> = std::env::args()
            .skip(1)
            .filter(|a| a != "--elevate")
            .collect();

        // If no pause preference was explicitly passed, add --pause so the user sees results
        if !elevated_args.iter().any(|a| a == "--pause" || a == "--no-pause") {
            elevated_args.push("--pause".to_string());
        }

        let arg_str = elevated_args
            .iter()
            .map(|a| format!("\"{}\"", a.replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(" ");

        let powershell_cmd = format!(
            "Start-Process -FilePath '{}' -ArgumentList '{}' -Verb RunAs",
            current_exe.display(),
            arg_str
        );

        println!("\nRequesting Administrator elevation via UAC to unlock restricted folders...");
        let status = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &powershell_cmd])
            .status();

        matches!(status, Ok(s) if s.success())
    }

    #[cfg(not(windows))]
    false
}
