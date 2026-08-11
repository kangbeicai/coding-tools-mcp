use std::mem;
use std::path::Path;

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, TerminateProcess, WaitForSingleObject,
    PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
};

use crate::error::{AppError, AppResult};

pub fn is_process_alive(pid: u32) -> bool {
    unsafe {
        let handle = OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            false,
            pid,
        )
        .or_else(|_| OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid));
        let Ok(handle) = handle else {
            return false;
        };
        if handle == INVALID_HANDLE_VALUE {
            return false;
        }
        let still_running = WaitForSingleObject(handle, 0) != WAIT_OBJECT_0;
        let _ = CloseHandle(handle);
        still_running
    }
}

pub fn process_image_path(pid: u32) -> AppResult<Option<String>> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .map_err(|err| AppError::Message(format!("OpenProcess failed: {err}")))?;
        if handle == INVALID_HANDLE_VALUE {
            return Ok(None);
        }

        let mut size = 32_768u32;
        let mut buffer = vec![0u16; size as usize];
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(handle);
        if result.is_err() {
            return Ok(None);
        }
        buffer.truncate(size as usize);
        Ok(Some(String::from_utf16_lossy(&buffer)))
    }
}

pub fn terminate_process_tree(root_pid: u32) -> AppResult<()> {
    let children = collect_child_pids(root_pid)?;
    for pid in children.into_iter().rev() {
        terminate_pid(pid)?;
    }
    terminate_pid(root_pid)
}

pub fn terminate_processes_by_image_path(image_path: &Path) -> AppResult<usize> {
    let expected = normalize_image_path(image_path);
    let mut matched = Vec::new();
    for pid in process_ids()? {
        if let Ok(Some(actual)) = process_image_path(pid) {
            if normalize_image_path(Path::new(&actual)) == expected {
                matched.push(pid);
            }
        }
    }

    let mut terminated = 0;
    for pid in matched {
        if terminate_process_tree(pid).is_ok() {
            terminated += 1;
        }
    }
    Ok(terminated)
}

fn normalize_image_path(path: &Path) -> String {
    let normalized = std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .replace('/', "\\")
        .trim_matches('"')
        .to_ascii_lowercase();
    normalized
        .strip_prefix("\\\\?\\")
        .unwrap_or(&normalized)
        .to_string()
}

fn process_ids() -> AppResult<Vec<u32>> {
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let mut pids = Vec::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .map_err(|err| AppError::Message(format!("CreateToolhelp32Snapshot failed: {err}")))?;
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(AppError::Message("invalid process snapshot".into()));
        }
        let mut entry = PROCESSENTRY32W {
            dwSize: mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                pids.push(entry.th32ProcessID);
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
    Ok(pids)
}

fn collect_child_pids(root_pid: u32) -> AppResult<Vec<u32>> {
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let mut parent_by_pid = Vec::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .map_err(|err| AppError::Message(format!("CreateToolhelp32Snapshot failed: {err}")))?;
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(AppError::Message("invalid process snapshot".into()));
        }
        let mut entry = PROCESSENTRY32W {
            dwSize: mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                parent_by_pid.push((entry.th32ProcessID, entry.th32ParentProcessID));
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }

    let mut pending = vec![root_pid];
    let mut seen = std::collections::HashSet::from([root_pid]);
    let mut ordered = Vec::new();
    let mut index = 0usize;
    while index < pending.len() {
        let parent = pending[index];
        for &(pid, candidate_parent) in &parent_by_pid {
            if candidate_parent == parent && seen.insert(pid) {
                ordered.push(pid);
                pending.push(pid);
            }
        }
        index += 1;
    }
    Ok(ordered)
}

fn terminate_pid(pid: u32) -> AppResult<()> {
    unsafe {
        let handle: HANDLE = OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, false, pid)
            .or_else(|_| OpenProcess(PROCESS_TERMINATE, false, pid))
            .map_err(|err| AppError::Message(format!("OpenProcess terminate failed: {err}")))?;
        if handle == INVALID_HANDLE_VALUE {
            return Ok(());
        }
        if WaitForSingleObject(handle, 0) == WAIT_OBJECT_0 {
            let _ = CloseHandle(handle);
            return Ok(());
        }
        let result = TerminateProcess(handle, 1);
        let _ = WaitForSingleObject(handle, 1_000);
        let _ = CloseHandle(handle);
        result.map_err(|err| AppError::Message(format!("TerminateProcess failed: {err}")))?;
        Ok(())
    }
}
