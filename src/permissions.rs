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

/// Automatically unlock NTFS permissions on a directory.
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

        // Step 1: Take ownership recursively
        let takeown_res = std::process::Command::new("takeown")
            .args(["/F", &folder_str, "/R", "/D", "Y"])
            .output();

        // Step 2: Grant BUILTIN\Users (*S-1-5-32-545) and Administrators (*S-1-5-32-544) Full Control
        let icacls_grant = std::process::Command::new("icacls")
            .args([
                &folder_str,
                "/grant",
                "*S-1-5-32-545:(OI)(CI)F",
                "/T",
                "/C",
                "/Q",
            ])
            .output();

        // Step 3: Reset inheritance
        let _icacls_reset = std::process::Command::new("icacls")
            .args([&folder_str, "/reset", "/T", "/C", "/Q"])
            .output();

        let takeown_ok = matches!(takeown_res, Ok(ref o) if o.status.success());
        let grant_ok = match &icacls_grant {
            Ok(o) => {
                let out = String::from_utf8_lossy(&o.stdout);
                let err = String::from_utf8_lossy(&o.stderr);
                o.status.success()
                    && !out.contains("Failed processing")
                    && !err.contains("Access is denied")
            }
            Err(_) => false,
        };

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
