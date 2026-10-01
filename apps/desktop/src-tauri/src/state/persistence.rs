use crate::core::sessions::Settings;

/// Loads saved settings. A file that cannot be used is copied aside first: Doze then runs
/// with defaults, and the next save must not destroy preferences or agent credentials.
pub fn load(path: &std::path::Path) -> Result<Settings, String> {
    if !path.exists() {
        return Ok(Settings::default());
    }
    let parsed = std::fs::read(path)
        .map_err(|e| format!("Could not read settings: {e}."))
        .and_then(|bytes| {
            serde_json::from_slice::<Settings>(&bytes)
                .map_err(|e| format!("Could not read settings: {e}."))
        })
        .and_then(|settings| {
            settings
                .validate()
                .map_err(|e| format!("Saved settings are invalid: {e}"))?;
            Ok(settings)
        });
    parsed.map_err(|error| {
        let backup = path.with_extension("invalid.json");
        match std::fs::copy(path, &backup) {
            Ok(_) => format!(
                "{error} Using defaults; your previous file was kept at {}.",
                backup.display()
            ),
            Err(copy) => {
                format!("{error} Using defaults; the previous file could not be copied: {copy}.")
            }
        }
    })
}
pub fn persist(path: &std::path::Path, settings: &Settings) -> Result<(), String> {
    std::fs::create_dir_all(path.parent().ok_or("Invalid settings path")?)
        .map_err(|e| e.to_string())?;
    let temporary = path.with_extension("tmp");
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    file.write_all(&serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    std::fs::rename(temporary, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unusable_settings_are_kept_aside_before_defaults_apply() {
        let directory =
            std::env::temp_dir().join(format!("doze-settings-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("settings.json");
        let original = br#"{"agents":{"leaseSeconds":5,"clients":[]},"notifications":false}"#;
        std::fs::write(&path, original).unwrap();
        let error = load(&path).unwrap_err();
        let backup = directory.join("settings.invalid.json");
        assert!(error.contains(&backup.display().to_string()), "{error}");
        assert_eq!(std::fs::read(&backup).unwrap(), original);
        persist(&path, &Settings::default()).unwrap();
        assert_eq!(std::fs::read(&backup).unwrap(), original);
        assert_eq!(load(&path).unwrap(), Settings::default());
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
