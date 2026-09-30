use crate::core::sessions::Settings;

pub fn load(path: &std::path::Path) -> Result<Settings, String> {
    if !path.exists() {
        return Ok(Settings::default());
    }
    let settings: Settings =
        serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Could not read settings: {e}"))?;
    settings.validate()?;
    Ok(settings)
}
pub(super) fn persist(path: &std::path::Path, settings: &Settings) -> Result<(), String> {
    std::fs::create_dir_all(path.parent().ok_or("Invalid settings path")?)
        .map_err(|e| e.to_string())?;
    let temporary = path.with_extension("tmp");
    use std::io::Write;
    let mut file = std::fs::File::create(&temporary).map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    std::fs::rename(temporary, path).map_err(|e| e.to_string())
}
