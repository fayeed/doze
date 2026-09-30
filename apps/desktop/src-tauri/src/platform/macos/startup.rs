pub fn set_enabled(enabled: bool) -> Result<(), String> {
    if enabled {
        Err("macOS launch-at-login is not implemented yet.".into())
    } else {
        Ok(())
    }
}
