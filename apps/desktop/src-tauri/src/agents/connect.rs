//! Connect and Remove in Settings › Agents. Each edit is previewed as a diff, confirmed by the
//! user, preceded by a timestamped backup, and limited to Doze's own entries: anything else
//! in the file is left as it was, and Remove takes out exactly what Connect added.
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

/// Agents with a lifecycle API Doze can connect to.
pub const AGENTS: [&str; 4] = ["claude-code", "codex", "opencode", "gemini-cli"];

struct Target {
    /// The tool's own folder; it exists once the tool has been run.
    folder: PathBuf,
    file: PathBuf,
    method: &'static str,
    events: &'static [&'static str],
    note: Option<&'static str>,
}

fn target(agent: &str, home: &Path) -> Result<Target, String> {
    Ok(match agent {
        // https://docs.claude.com/en/docs/claude-code/hooks
        "claude-code" => Target {
            folder: home.join(".claude"),
            file: home.join(".claude/settings.json"),
            method: "Hooks",
            events: &[
                "SessionStart",
                "UserPromptSubmit",
                "PostToolUse",
                "Notification",
                "Stop",
                "SessionEnd",
            ],
            note: None,
        },
        // Codex reads Claude-style hooks from ~/.codex/hooks.json and runs new hooks only
        // once the user trusts them.
        "codex" => Target {
            folder: home.join(".codex"),
            file: home.join(".codex/hooks.json"),
            method: "Hooks",
            events: &[
                "SessionStart",
                "UserPromptSubmit",
                "PostToolUse",
                "Stop",
                "SessionEnd",
            ],
            note: Some("Codex runs new hooks only after you trust them: open Codex, run /hooks and trust Doze's entries."),
        },
        // https://geminicli.com/docs/hooks/
        "gemini-cli" => Target {
            folder: home.join(".gemini"),
            file: home.join(".gemini/settings.json"),
            method: "Hooks",
            events: &[
                "SessionStart",
                "BeforeAgent",
                "AfterTool",
                "Notification",
                "AfterAgent",
                "SessionEnd",
            ],
            note: None,
        },
        // https://opencode.ai/docs/plugins/ — a plugin file of Doze's own.
        "opencode" => Target {
            folder: home.join(".config/opencode"),
            file: home.join(".config/opencode/plugins/doze.js"),
            method: "Plugin",
            events: &[],
            note: Some("OpenCode loads the plugin the next time it starts."),
        },
        _ => return Err("Doze cannot connect this agent.".into()),
    })
}

pub fn home() -> Result<PathBuf, String> {
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME");
    home.map(PathBuf::from)
        .ok_or_else(|| "Could not find your home folder.".into())
}

/// The command agents run: the console front end on Windows, the app binary on macOS.
pub fn executable() -> String {
    let current = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("doze"));
    #[cfg(windows)]
    let current = {
        let cli = current.with_file_name("doze-cli.exe");
        if cli.is_file() {
            cli
        } else {
            current
        }
    };
    // Forward slashes work in cmd, PowerShell and the POSIX shells agents use on Windows.
    current.to_string_lossy().replace('\\', "/")
}

fn command(executable: &str, agent: &str, event: &str) -> String {
    format!("\"{executable}\" hook {agent} {event}")
}

/// Whether a hook command is one Doze wrote for this agent.
fn ours(command: &str, agent: &str) -> bool {
    command.contains(&format!(" hook {agent} ")) && command.to_ascii_lowercase().contains("doze")
}

fn entry(agent: &str, event: &str, command: String) -> Value {
    match agent {
        // Gemini CLI timeouts are milliseconds and entries carry a name.
        "gemini-cli" => {
            json!({"name": "doze", "type": "command", "command": command, "timeout": 5000})
        }
        // Codex caps SessionEnd hooks at 3 seconds and warns about longer ones in every session.
        "codex" if event == "SessionEnd" => {
            json!({"type": "command", "command": command, "timeout": 3})
        }
        _ => json!({"type": "command", "command": command, "timeout": 10}),
    }
}

/// Removes this agent's Doze hooks. Groups and event lists Doze emptied are removed too.
fn strip(document: &mut Value, agent: &str) {
    let Some(hooks) = document.get_mut("hooks").and_then(Value::as_object_mut) else {
        return;
    };
    let mut emptied = Vec::new();
    for (event, groups) in hooks.iter_mut() {
        let Some(groups) = groups.as_array_mut() else {
            continue;
        };
        let before = groups.len();
        groups.retain_mut(|group| {
            let Some(entries) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
                return true;
            };
            let count = entries.len();
            entries.retain(|entry| {
                !entry["command"]
                    .as_str()
                    .is_some_and(|command| ours(command, agent))
            });
            !(entries.is_empty() && count > 0)
        });
        if groups.is_empty() && before > 0 {
            emptied.push(event.clone());
        }
    }
    for event in emptied {
        hooks.remove(&event);
    }
    if hooks.is_empty() {
        if let Some(object) = document.as_object_mut() {
            object.remove("hooks");
        }
    }
}

fn connected_in(document: &Value, agent: &str) -> bool {
    document["hooks"].as_object().is_some_and(|hooks| {
        hooks
            .values()
            .flat_map(|g| g.as_array().into_iter().flatten())
            .any(|group| {
                group["hooks"].as_array().is_some_and(|entries| {
                    entries.iter().any(|entry| {
                        entry["command"]
                            .as_str()
                            .is_some_and(|command| ours(command, agent))
                    })
                })
            })
    })
}

fn opencode_plugin(executable: &str) -> String {
    let exe = Value::String(executable.into());
    format!(
        r#"// Doze: keeps this computer awake while OpenCode works.
// Added by Doze › Settings › Agents. Remove it there, or delete this file.
const DOZE = {exe};

export const DozePlugin = async ({{ directory }}) => {{
  const send = (event, sessionID, extra = {{}}) => {{
    try {{
      const proc = Bun.spawn([DOZE, "hook", "opencode", event], {{
        stdin: "pipe",
        stdout: "ignore",
        stderr: "ignore",
      }});
      proc.stdin.write(JSON.stringify({{ session_id: sessionID, cwd: directory, ...extra }}));
      proc.stdin.end();
    }} catch {{}}
  }};
  const text = (output) =>
    (output?.parts ?? [])
      .filter((part) => part?.type === "text")
      .map((part) => part.text)
      .join(" ");
  return {{
    "chat.message": async (input, output) => {{
      send("chat.message", input?.sessionID, {{ prompt: text(output) }});
    }},
    "tool.execute.after": async (input) => {{
      send("tool.execute.after", input?.sessionID);
    }},
    event: async ({{ event }}) => {{
      const id = event?.properties?.sessionID ?? event?.properties?.info?.id;
      switch (event?.type) {{
        case "session.created":
        case "session.idle":
        case "session.deleted":
        case "session.error":
        case "permission.asked":
        case "permission.updated":
          send(event.type, id);
          break;
        case "session.status":
          send(`session.status:${{event.properties?.status?.type}}`, id);
          break;
      }}
    }},
  }};
}};
"#
    )
}

/// One agent's row in Settings › Agents.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub id: String,
    pub name: String,
    pub monogram: String,
    pub method: String,
    pub installed: bool,
    pub connected: bool,
    pub path: PathBuf,
}

pub fn connections(home: &Path) -> Vec<Connection> {
    AGENTS
        .iter()
        .filter_map(|agent| {
            let target = target(agent, home).ok()?;
            let connected = match *agent {
                "opencode" => std::fs::read_to_string(&target.file)
                    .is_ok_and(|text| text.contains("hook\", \"opencode\"")),
                _ => std::fs::read(&target.file)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                    .is_some_and(|document| connected_in(&document, agent)),
            };
            let kind = super::AgentKind::parse(agent).ok()?;
            Some(Connection {
                id: (*agent).into(),
                name: kind.label(),
                monogram: kind.monogram(),
                method: target.method.into(),
                installed: target.folder.is_dir(),
                connected,
                path: target.file,
            })
        })
        .collect()
}

/// A proposed edit, shown to the user before anything is written.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub agent: String,
    pub remove: bool,
    pub path: PathBuf,
    pub before: Option<String>,
    /// None deletes the file (only ever Doze's own plugin file).
    pub after: Option<String>,
    pub diff: String,
    pub note: Option<String>,
    /// Confirms that the file the user reviewed is the file Doze writes.
    pub token: String,
}

pub fn preview(agent: &str, remove: bool, home: &Path, executable: &str) -> Result<Change, String> {
    let target = target(agent, home)?;
    let before = match std::fs::read_to_string(&target.file) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("Could not read {}: {error}", target.file.display())),
    };
    let after = if agent == "opencode" {
        (!remove).then(|| opencode_plugin(executable))
    } else {
        let mut document = match before.as_deref().map(str::trim) {
            None | Some("") => Value::Object(Map::new()),
            Some(text) => serde_json::from_str::<Value>(text).map_err(|_| {
                format!(
                    "{} is not plain JSON (it may contain comments), so Doze will not rewrite it. Add the hooks yourself or remove the comments.",
                    target.file.display()
                )
            })?,
        };
        if !document.is_object() {
            return Err(format!("{} is not a JSON object.", target.file.display()));
        }
        strip(&mut document, agent);
        if !remove {
            let hooks = document
                .as_object_mut()
                .ok_or("Invalid settings file.")?
                .entry("hooks")
                .or_insert_with(|| Value::Object(Map::new()));
            let hooks = hooks
                .as_object_mut()
                .ok_or("The file's \"hooks\" entry is not an object.")?;
            for event in target.events {
                let groups = hooks
                    .entry(*event)
                    .or_insert_with(|| Value::Array(Vec::new()));
                groups
                    .as_array_mut()
                    .ok_or(format!("The file's \"{event}\" hooks are not a list."))?
                    .push(
                        json!({"hooks": [entry(agent, event, command(executable, agent, event))]}),
                    );
            }
        }
        let mut text = serde_json::to_string_pretty(&document).map_err(|e| e.to_string())?;
        text.push('\n');
        Some(text)
    };
    if before == after {
        return Err(if remove {
            "Doze is not connected to this agent.".into()
        } else {
            "Already connected.".into()
        });
    }
    let diff = diff(
        before.as_deref().unwrap_or(""),
        after.as_deref().unwrap_or(""),
    );
    let token = format!(
        "{:016x}",
        fingerprint(before.as_deref().unwrap_or("\u{0}missing"))
            ^ fingerprint(after.as_deref().unwrap_or("\u{0}deleted")).rotate_left(1)
    );
    Ok(Change {
        agent: agent.into(),
        remove,
        path: target.file,
        before,
        after,
        diff,
        note: target.note.map(String::from),
        token,
    })
}

/// Writes a change the user confirmed. The file must still be exactly what was reviewed.
pub fn apply(
    agent: &str,
    remove: bool,
    token: &str,
    home: &Path,
    executable: &str,
) -> Result<String, String> {
    let change = preview(agent, remove, home, executable)?;
    if change.token != token {
        return Err("The file changed since you reviewed it. Review the change again.".into());
    }
    let path = &change.path;
    if change.before.is_some() {
        let backup = backup_path(path, std::time::SystemTime::now());
        std::fs::copy(path, &backup)
            .map_err(|e| format!("Could not back up {}: {e}", path.display()))?;
    }
    match &change.after {
        Some(text) => {
            std::fs::create_dir_all(path.parent().ok_or("Invalid path.")?)
                .map_err(|e| e.to_string())?;
            let temporary = path.with_extension("doze-tmp");
            std::fs::write(&temporary, text).map_err(|e| e.to_string())?;
            std::fs::rename(&temporary, path).map_err(|e| e.to_string())?;
        }
        None => std::fs::remove_file(path).map_err(|e| e.to_string())?,
    }
    Ok(change.note.unwrap_or_else(|| {
        if remove {
            "Removed. A backup of the previous file was kept beside it.".into()
        } else {
            "Connected. A backup of the previous file was kept beside it.".into()
        }
    }))
}

/// "Use a prompt…" in Settings › Agents: text to paste into the agent so it adds the same
/// entries Connect writes, for people who would rather the agent edit its own settings.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupPrompt {
    pub agent: String,
    pub name: String,
    pub path: PathBuf,
    pub prompt: String,
}

pub fn prompt(agent: &str, home: &Path, executable: &str) -> Result<SetupPrompt, String> {
    let target = target(agent, home)?;
    let name = super::AgentKind::parse(agent)?.label();
    let file = target.file.display();
    let mut text = format!(
        "Set up Doze for {name} on this computer. Doze is a desktop app that keeps the computer awake while you work, and it learns when you start and stop from {}.\n\n",
        if target.method == "Plugin" {
            "a small plugin"
        } else {
            "hooks in your settings"
        }
    );
    if agent == "opencode" {
        text += &format!(
            "Create {file} with exactly the content below. If the file already exists and is not Doze's plugin, stop and ask me first. Change nothing else.\n\n```js\n{}```\n\n",
            opencode_plugin(executable)
        );
    } else {
        let mut hooks = Map::new();
        for event in target.events {
            hooks.insert(
                (*event).into(),
                json!([{"hooks": [entry(agent, event, command(executable, agent, event))]}]),
            );
        }
        let snippet =
            serde_json::to_string_pretty(&json!({ "hooks": hooks })).map_err(|e| e.to_string())?;
        text += &format!(
            "Edit {file} (create it containing {{}} if it doesn't exist):\n\
             1. First copy it to {file}.doze-backup-<date and time>, so it can be restored.\n\
             2. Under \"hooks\", append each entry below to that event's list. Keep every existing setting and hook exactly as it is. Skip an event whose list already has a command containing \"hook {agent} \".\n\
             3. Keep the file valid JSON, without comments.\n\n\
             ```json\n{snippet}\n```\n\n\
             Change nothing else, then show me what you added.\n"
        );
    }
    text += match agent {
        "claude-code" => {
            "Claude Code loads hooks when a session starts, so remind me to start a new session."
        }
        "codex" => {
            "Codex runs new hooks only after I trust them, so remind me to run /hooks and trust Doze's entries."
        }
        "gemini-cli" => "Gemini CLI loads hooks when it starts, so remind me to restart it.",
        _ => "OpenCode loads plugins when it starts, so remind me to restart it.",
    };
    text.push('\n');
    Ok(SetupPrompt {
        agent: agent.into(),
        name,
        path: target.file,
        prompt: text,
    })
}

fn backup_path(path: &Path, now: std::time::SystemTime) -> PathBuf {
    let seconds = now
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (year, month, day) = civil(seconds / 86400);
    let stamp = format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    );
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!("{name}.doze-backup-{stamp}"))
}

/// Days since 1970-01-01 to a UTC calendar date (Howard Hinnant's algorithm).
fn civil(days: u64) -> (i64, u32, u32) {
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn fingerprint(text: &str) -> u64 {
    // FNV-1a: only detects that the reviewed file changed; not a security boundary.
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// A unified-style line diff with two lines of context, for the confirmation sheet.
pub fn diff(before: &str, after: &str) -> String {
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = after.lines().collect();
    let (n, m) = (a.len(), b.len());
    if n.saturating_mul(m) > 4_000_000 {
        return format!("(file too large to compare: {n} → {m} lines)");
    }
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let mut lines = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            lines.push((' ', a[i]));
            i += 1;
            j += 1;
        } else if j < m && (i == n || lcs[i][j + 1] >= lcs[i + 1][j]) {
            lines.push(('+', b[j]));
            j += 1;
        } else {
            lines.push(('-', a[i]));
            i += 1;
        }
    }
    let changed: Vec<usize> = (0..lines.len()).filter(|&k| lines[k].0 != ' ').collect();
    let mut out = String::new();
    let mut last = None;
    for (k, (mark, text)) in lines.iter().enumerate() {
        let near = changed.iter().any(|&c| c.abs_diff(k) <= 2);
        if !near {
            continue;
        }
        if last.is_some_and(|l: usize| k > l + 1) {
            out.push_str("  …\n");
        }
        out.push(*mark);
        out.push(' ');
        out.push_str(text);
        out.push('\n');
        last = Some(k);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> PathBuf {
        let home = std::env::temp_dir().join(format!("doze-connect-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&home).unwrap();
        home
    }
    const EXE: &str = "/Applications/Doze.app/Contents/MacOS/doze";

    #[test]
    fn connect_keeps_other_entries_and_remove_restores_them() {
        let home = home();
        let path = home.join(".claude/settings.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original = r#"{
  "model": "opus",
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "say done"
          }
        ]
      }
    ]
  }
}
"#;
        std::fs::write(&path, original).unwrap();
        let change = preview("claude-code", false, &home, EXE).unwrap();
        assert!(change.diff.contains("+ "));
        assert!(!change.diff.contains("- "), "connect only adds lines");
        apply("claude-code", false, &change.token, &home, EXE).unwrap();
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(written["model"], "opus");
        assert_eq!(
            written["hooks"]["Stop"][0]["hooks"][0]["command"],
            "say done"
        );
        assert!(
            written["hooks"]["UserPromptSubmit"][0]["hooks"][0]["command"]
                .as_str()
                .unwrap()
                .ends_with("hook claude-code UserPromptSubmit")
        );
        assert!(connections(&home)[0].connected);
        // A backup of the original sits beside the file.
        let backups: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".doze-backup-"))
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(
            std::fs::read_to_string(backups[0].path()).unwrap(),
            original
        );
        // Connecting twice is refused; removing restores the original document exactly.
        assert!(preview("claude-code", false, &home, EXE).is_err());
        let change = preview("claude-code", true, &home, EXE).unwrap();
        apply("claude-code", true, &change.token, &home, EXE).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn the_setup_prompt_asks_for_exactly_what_connect_writes() {
        let home = home();
        for agent in AGENTS {
            let setup = prompt(agent, &home, EXE).unwrap();
            assert!(setup.prompt.contains(&setup.path.display().to_string()));
            assert!(setup.prompt.contains(EXE));
            if agent == "opencode" {
                assert!(setup.prompt.contains(&opencode_plugin(EXE)));
                continue;
            }
            // The prompt's JSON block holds the entries Connect adds, event for event.
            let block = setup.prompt.split("```json\n").nth(1).unwrap();
            let snippet: Value =
                serde_json::from_str(block.split("\n```").next().unwrap()).unwrap();
            let target = target(agent, &home).unwrap();
            assert_eq!(
                snippet["hooks"].as_object().unwrap().len(),
                target.events.len()
            );
            for event in target.events {
                assert_eq!(
                    snippet["hooks"][*event][0]["hooks"][0],
                    entry(agent, event, command(EXE, agent, event))
                );
            }
            assert!(connected_in(&snippet, agent), "Doze recognises the entries");
            assert!(setup
                .prompt
                .contains("Keep every existing setting and hook"));
        }
        assert!(prompt("cursor", &home, EXE).is_err());
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn a_file_changed_after_review_is_not_written() {
        let home = home();
        let change = preview("gemini-cli", false, &home, EXE).unwrap();
        let path = home.join(".gemini/settings.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{\"theme\":\"dark\"}").unwrap();
        assert!(apply("gemini-cli", false, &change.token, &home, EXE).is_err());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\"theme\":\"dark\"}"
        );
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn codex_session_end_stays_within_its_three_second_limit() {
        let home = home();
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        let change = preview("codex", false, &home, EXE).unwrap();
        apply("codex", false, &change.token, &home, EXE).unwrap();
        let hooks: Value =
            serde_json::from_slice(&std::fs::read(home.join(".codex/hooks.json")).unwrap())
                .unwrap();
        assert_eq!(hooks["hooks"]["SessionEnd"][0]["hooks"][0]["timeout"], 3);
        assert_eq!(hooks["hooks"]["Stop"][0]["hooks"][0]["timeout"], 10);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn json_with_comments_is_refused_and_opencode_uses_its_own_file() {
        let home = home();
        let path = home.join(".codex/hooks.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "// mine\n{}").unwrap();
        assert!(preview("codex", false, &home, EXE).is_err());
        let change = preview("opencode", false, &home, EXE).unwrap();
        assert!(change.before.is_none());
        apply("opencode", false, &change.token, &home, EXE).unwrap();
        let plugin = home.join(".config/opencode/plugins/doze.js");
        assert!(std::fs::read_to_string(&plugin).unwrap().contains(EXE));
        assert!(connections(&home)
            .iter()
            .any(|c| c.id == "opencode" && c.connected));
        let change = preview("opencode", true, &home, EXE).unwrap();
        apply("opencode", true, &change.token, &home, EXE).unwrap();
        assert!(!plugin.exists());
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn backups_are_timestamped_in_utc() {
        let at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_791_158_400 + 3661);
        let name = backup_path(Path::new("/x/settings.json"), at);
        assert_eq!(
            name,
            PathBuf::from("/x/settings.json.doze-backup-20261005-010101")
        );
    }
}
