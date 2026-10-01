//! Offline skill distribution. Only private native UI operations invoke installation.
use serde_json::{json, Value};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

const FILES: [(&str, &str); 2] = [
    (
        "SKILL.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../skills/doze/SKILL.md"
        )),
    ),
    (
        "agents/openai.yaml",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../skills/doze/agents/openai.yaml"
        )),
    ),
];

fn home() -> Result<PathBuf, String> {
    let variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    let path = std::env::var_os(variable)
        .map(PathBuf::from)
        .ok_or("User home directory unavailable.")?;
    if !path.is_absolute() {
        return Err("User home directory must be absolute.".into());
    }
    Ok(path)
}

fn destination(home: &Path, client: &str) -> Result<PathBuf, String> {
    let parent = match client {
        "Codex" => ".agents",
        "Claude Code" => ".claude",
        _ => return Err("Automatic skill installation supports Codex and Claude Code.".into()),
    };
    Ok(home.join(parent).join("skills/doze"))
}

// Only components below the trusted root (home or Doze's data directory) are checked. System
// locations above it may be links, such as macOS `/var` → `/private/var`.
fn check_directories(root: &Path, path: &Path) -> io::Result<()> {
    if !path.starts_with(root) {
        return Err(io::Error::other("Skill path is outside its trusted root."));
    }
    for ancestor in path.ancestors().take_while(|ancestor| *ancestor != root) {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(io::Error::other(
                    "Skill path contains a link or a non-directory. Install manually instead.",
                ))
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn status(root: &Path, path: &Path) -> &'static str {
    if check_directories(root, path).is_err() {
        return "blocked";
    }
    if !path.exists() {
        return "not_installed";
    }
    if FILES.iter().all(|(name, content)| {
        let file = path.join(name);
        fs::symlink_metadata(&file).is_ok_and(|m| {
            m.is_file() && !m.file_type().is_symlink() && m.len() == content.len() as u64
        }) && fs::read(&file).is_ok_and(|bytes| bytes == content.as_bytes())
    }) {
        "installed"
    } else {
        "update_available"
    }
}

pub fn states() -> Value {
    json!(["Codex", "Claude Code"].map(|name| {
        match home().and_then(|home| Ok((destination(&home, name)?, home))) {
            Ok((path, home)) => {
                json!({"name": name, "path": path, "status": status(&home, &path)})
            }
            Err(error) => json!({"name": name, "status": "blocked", "error": error}),
        }
    }))
}

// Copy preserves extra files, but never follows links. Limits keep this a small UI operation.
fn copy_tree(
    source: &Path,
    target: &Path,
    depth: usize,
    budget: &mut (usize, u64),
) -> io::Result<()> {
    if depth > 8 {
        return Err(io::Error::other(
            "Skill folder is too deeply nested. Update manually.",
        ));
    }
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        budget.0 += 1;
        budget.1 = budget.1.saturating_add(metadata.len());
        if budget.0 > 256 || budget.1 > 8 * 1024 * 1024 {
            return Err(io::Error::other(
                "Skill folder is too large. Update manually.",
            ));
        }
        let target = target.join(entry.file_name());
        if metadata.file_type().is_symlink() {
            return Err(io::Error::other(
                "Skill folder contains a link. Update manually.",
            ));
        } else if metadata.is_dir() {
            copy_tree(&entry.path(), &target, depth + 1, budget)?;
        } else if metadata.is_file() {
            fs::copy(entry.path(), target)?;
        } else {
            return Err(io::Error::other(
                "Skill folder contains an unsupported file.",
            ));
        }
    }
    Ok(())
}

fn write_bundle(path: &Path) -> io::Result<()> {
    for (name, content) in FILES {
        let file = path.join(name);
        fs::create_dir_all(
            file.parent()
                .ok_or_else(|| io::Error::other("Missing skill parent"))?,
        )?;
        fs::write(file, content)?;
    }
    Ok(())
}

fn install_at(root: &Path, path: &Path, update: bool) -> Result<Option<PathBuf>, String> {
    let install = || -> io::Result<Option<PathBuf>> {
        check_directories(root, path)?;
        if status(root, path) == "installed" {
            return Ok(None);
        }
        if path.exists() && !update {
            return Err(io::Error::other(
                "Skill already exists. Confirm Update to preserve a backup before replacing it.",
            ));
        }
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("Missing skill directory"))?;
        fs::create_dir_all(parent)?;
        let staging = parent.join(format!(".doze-install-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&staging)?;
        let staged = (|| {
            if path.exists() {
                copy_tree(path, &staging, 0, &mut (0, 0))?;
            }
            write_bundle(&staging)
        })();
        if let Err(error) = staged {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
        // Recheck the fixed destination before swapping. Retain the original as a user-visible backup.
        check_directories(root, path)?;
        let backup = if path.exists() {
            if !update {
                let _ = fs::remove_dir_all(&staging);
                return Err(io::Error::other(
                    "Skill appeared during installation; nothing was replaced.",
                ));
            }
            // Backups live outside the client's scanned skills directory to avoid duplicate skills.
            let backup_parent = parent
                .parent()
                .ok_or_else(|| io::Error::other("Missing backup parent"))?
                .join("doze-skill-backups");
            check_directories(root, &backup_parent)?;
            fs::create_dir_all(&backup_parent)?;
            let backup = backup_parent.join(uuid::Uuid::new_v4().to_string());
            fs::rename(path, &backup)?;
            Some(backup)
        } else {
            None
        };
        if let Err(error) = fs::rename(&staging, path) {
            if let Some(backup) = &backup {
                fs::rename(backup, path).map_err(|restore| io::Error::other(format!("Install failed: {error}. Restore failed: {restore}. Your original remains at {}", backup.display())))?;
            }
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
        Ok(backup)
    };
    install().map_err(|error| format!("Could not install skill: {error}"))
}

pub fn install(client: &str, update: bool) -> Result<String, String> {
    let home = home()?;
    let path = destination(&home, client)?;
    let backup = install_at(&home, &path, update)?;
    Ok(match backup {
        Some(backup) => format!(
            "Doze skill updated at {}. Original saved at {}",
            path.display(),
            backup.display()
        ),
        None => format!(
            "Doze skill installed at {}. Reload your client if needed.",
            path.display()
        ),
    })
}

pub fn open_folder(settings_path: &Path) -> Result<(), String> {
    let root = settings_path
        .parent()
        .ok_or("Doze data directory unavailable.")?;
    let folder = root.join("bundled-skills/doze");
    install_at(root, &folder, true)?;
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("explorer.exe");
        command.creation_flags(0x0800_0000);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    command
        .arg(folder)
        .spawn()
        .map_err(|error| format!("Could not open skill folder: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> PathBuf {
        let path = std::env::temp_dir().join(format!("doze-skill-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }
    #[test]
    fn installs_complete_bundle_offline_and_detects_it() {
        let root = fixture();
        let path = destination(&root, "Codex").unwrap();
        install_at(&root, &path, false).unwrap();
        assert_eq!(status(&root, &path), "installed");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn existing_local_edits_require_explicit_update() {
        let root = fixture();
        let path = root.join("skills/doze");
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("SKILL.md"), "local edit").unwrap();
        assert!(install_at(&root, &path, false).is_err());
        assert_eq!(
            fs::read_to_string(path.join("SKILL.md")).unwrap(),
            "local edit"
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn confirmed_update_keeps_original_and_extra_files() {
        let root = fixture();
        let path = root.join("skills/doze");
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("SKILL.md"), "local edit").unwrap();
        fs::write(path.join("notes.txt"), "notes").unwrap();
        let backup = install_at(&root, &path, true).unwrap().unwrap();
        assert_eq!(
            fs::read_to_string(backup.join("SKILL.md")).unwrap(),
            "local edit"
        );
        assert_eq!(fs::read_to_string(path.join("notes.txt")).unwrap(), "notes");
        assert_eq!(status(&root, &path), "installed");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failed_staging_does_not_move_existing_skill() {
        let root = fixture();
        let path = root.join("skills/doze");
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("SKILL.md"), "original").unwrap();
        fs::write(path.join("agents"), "not a directory").unwrap();
        assert!(install_at(&root, &path, true).is_err());
        assert_eq!(
            fs::read_to_string(path.join("SKILL.md")).unwrap(),
            "original"
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn links_below_the_trusted_root_are_refused() {
        let root = fixture();
        fs::create_dir_all(root.join("real")).unwrap();
        std::os::unix::fs::symlink(root.join("real"), root.join(".claude")).unwrap();
        let path = destination(&root, "Claude Code").unwrap();
        assert_eq!(status(&root, &path), "blocked");
        assert!(install_at(&root, &path, false).is_err());
        assert!(!root.join("real/skills").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unsupported_clients_cannot_choose_install_paths() {
        assert!(destination(Path::new("/home/example"), "../anything").is_err());
    }
}
