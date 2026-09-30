use windows::{
    core::{w, PCWSTR},
    Win32::{Foundation::ERROR_FILE_NOT_FOUND, System::Registry::*},
};
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    unsafe {
        let mut key = HKEY::default();
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            0,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
        .ok()
        .map_err(|e| e.to_string())?;
        let result = if enabled {
            let path = std::env::current_exe().map_err(|e| e.to_string());
            match path {
                Ok(path) => {
                    let value: Vec<u16> = format!("\"{}\" --startup", path.display())
                        .encode_utf16()
                        .chain(Some(0))
                        .collect();
                    let bytes: Vec<u8> = value.iter().flat_map(|v| v.to_le_bytes()).collect();
                    RegSetValueExW(key, w!("Doze"), 0, REG_SZ, Some(&bytes))
                        .ok()
                        .map_err(|e| e.to_string())
                }
                Err(e) => Err(e),
            }
        } else {
            let status = RegDeleteValueW(key, w!("Doze"));
            if status == ERROR_FILE_NOT_FOUND {
                Ok(())
            } else {
                status.ok().map_err(|e| e.to_string())
            }
        };
        let _ = RegCloseKey(key);
        result
    }
}
