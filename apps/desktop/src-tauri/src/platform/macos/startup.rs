//! Launch at login through a per-user LaunchAgent, matching the Windows Run key: the current
//! executable starts with `--startup` when the user logs in. macOS lists it under Login Items.
use std::{
    fs, io,
    path::{Path, PathBuf},
};

const LABEL: &str = "app.getdoze.desktop";

fn agent_path(home: &Path) -> PathBuf {
    home.join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn property_list(executable: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>--startup</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>LimitLoadToSessionType</key>
    <string>Aqua</string>
    <key>ProcessType</key>
    <string>Interactive</string>
</dict>
</plist>
"#,
        escape(&executable.to_string_lossy())
    )
}

fn set_at(home: &Path, executable: &Path, enabled: bool) -> io::Result<()> {
    let path = agent_path(home);
    if !enabled {
        return match fs::remove_file(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        };
    }
    let folder = path
        .parent()
        .ok_or_else(|| io::Error::other("LaunchAgents folder unavailable"))?;
    fs::create_dir_all(folder)?;
    let staging = folder.join(format!(".{LABEL}.{}.tmp", std::process::id()));
    fs::write(&staging, property_list(executable))?;
    fs::rename(&staging, &path).inspect_err(|_| {
        let _ = fs::remove_file(&staging);
    })
}

pub fn set_enabled(enabled: bool) -> Result<(), String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
        .ok_or("Home folder unavailable.")?;
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    set_at(&home, &executable, enabled)
        .map_err(|error| format!("Could not update launch at login: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_item_launches_this_executable_in_the_menu_bar_and_can_be_removed() {
        let home = std::env::temp_dir().join(format!("doze-startup-{}", uuid::Uuid::new_v4()));
        let executable = Path::new("/Applications/Doze & Co.app/Contents/MacOS/doze");
        set_at(&home, executable, true).unwrap();
        let written = fs::read_to_string(agent_path(&home)).unwrap();
        assert!(written
            .contains("<string>/Applications/Doze &amp; Co.app/Contents/MacOS/doze</string>"));
        assert!(written.contains("<string>--startup</string>"));
        set_at(&home, executable, false).unwrap();
        assert!(!agent_path(&home).exists());
        // Disabling twice is not an error.
        set_at(&home, executable, false).unwrap();
        fs::remove_dir_all(home).unwrap();
    }
}
