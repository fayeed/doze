//! Battery level for the battery guard, and process names for tools detected by process.
use windows::Win32::{
    Foundation::CloseHandle,
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
        Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS},
    },
};

/// Percent and whether Windows runs on battery; None on desktops or unknown levels.
pub fn read() -> Option<(u8, bool)> {
    let mut status = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut status) }.ok()?;
    // 128: no system battery. 255: unknown status or level.
    if status.BatteryFlag & 128 != 0 || status.BatteryFlag == 255 || status.BatteryLifePercent > 100
    {
        return None;
    }
    Some((status.BatteryLifePercent, status.ACLineStatus == 0))
}

pub fn processes() -> Vec<String> {
    let mut names = Vec::new();
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return names;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut more = Process32FirstW(snapshot, &mut entry).is_ok();
        while more {
            let length = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            names.push(String::from_utf16_lossy(&entry.szExeFile[..length]));
            more = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = CloseHandle(snapshot);
    }
    names
}

#[cfg(test)]
mod tests {
    #[test]
    fn this_process_is_listed() {
        let names = super::processes();
        let me = std::env::current_exe().unwrap();
        let me = me.file_name().unwrap().to_string_lossy();
        assert!(names.iter().any(|name| name.eq_ignore_ascii_case(&me)));
        // Desktops report no battery; laptops a level from 0 to 100.
        assert!(super::read().is_none_or(|(percent, _)| percent <= 100));
    }
}
