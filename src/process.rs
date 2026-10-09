//! Process detection utilities to identify running game and mod clients.

/// Detect any running processes that might hold locks on mod files or game WADs.
#[cfg(windows)]
pub fn detect_running_conflicting_processes() -> Vec<String> {
    use std::mem::size_of;

    type HANDLE = *mut std::ffi::c_void;
    type BOOL = i32;
    type DWORD = u32;

    const TH32CS_SNAPPROCESS: DWORD = 0x00000002;
    const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;

    #[repr(C)]
    struct PROCESSENTRY32W {
        dw_size: DWORD,
        cnt_usage: DWORD,
        th32_process_id: DWORD,
        th32_default_heap_id: usize,
        th32_module_id: DWORD,
        cnt_threads: DWORD,
        th32_parent_process_id: DWORD,
        pc_pri_class_base: i32,
        dw_flags: DWORD,
        sz_exe_file: [u16; 260],
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateToolhelp32Snapshot(dwFlags: DWORD, th32ProcessID: DWORD) -> HANDLE;
        fn Process32FirstW(hSnapshot: HANDLE, lppe: *mut PROCESSENTRY32W) -> BOOL;
        fn Process32NextW(hSnapshot: HANDLE, lppe: *mut PROCESSENTRY32W) -> BOOL;
        fn CloseHandle(hObject: HANDLE) -> BOOL;
    }

    let mut found = Vec::new();
    let targets = [
        "league of legends.exe",
    ];

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return found;
        }

        let mut entry = PROCESSENTRY32W {
            dw_size: size_of::<PROCESSENTRY32W>() as DWORD,
            cnt_usage: 0,
            th32_process_id: 0,
            th32_default_heap_id: 0,
            th32_module_id: 0,
            cnt_threads: 0,
            th32_parent_process_id: 0,
            pc_pri_class_base: 0,
            dw_flags: 0,
            sz_exe_file: [0u16; 260],
        };

        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let len = entry.sz_exe_file.iter().position(|&c| c == 0).unwrap_or(260);
                let exe_name = String::from_utf16_lossy(&entry.sz_exe_file[..len]);
                let lower = exe_name.to_ascii_lowercase();

                for target in &targets {
                    if lower == *target
                        && !found
                            .iter()
                            .any(|f: &String| f.eq_ignore_ascii_case(&exe_name))
                    {
                        found.push(exe_name.clone());
                    }
                }

                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }

        CloseHandle(snapshot);
    }

    found
}

/// Non-Windows stub.
#[cfg(not(windows))]
pub fn detect_running_conflicting_processes() -> Vec<String> {
    Vec::new()
}
