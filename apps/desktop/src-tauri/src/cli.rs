//! `doze run` and `doze watch`: keep the computer awake while a job runs, then optionally
//! sleep. Jobs are sessions from the built-in "Command line" client of the running app, sent
//! over the private bridge. A successful exit finishes the job, so its action follows the
//! usual final warning; a failure, Ctrl-C or an unknown result releases it without acting.
use crate::mcp::{server::forward, tools::Call};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

const USAGE: &str = "Keep this computer awake while a job runs, then optionally sleep.

Usage:
  doze run [--then ACTION] [--reason TEXT] -- COMMAND [ARGS...]
  doze watch --pid PID [--then ACTION] [--reason TEXT]

ACTION is nothing (default), sleep, display-off, lock, shutdown or hibernate.
The action runs only after the job succeeds, following Doze's final warning, which you can
cancel or snooze. A failed command or Ctrl-C releases the computer without any action.
`watch` follows a process that is already running and treats its exit as success.
Doze must be running. Exit status: the command's own status, or 2 for usage errors.";

/// Whether these arguments are a command-line job rather than the desktop app.
pub fn requested(args: &[String]) -> bool {
    matches!(
        args.get(1).map(String::as_str),
        Some("run" | "watch" | "help" | "--help")
    )
}

pub fn main(args: &[String]) -> i32 {
    #[cfg(windows)]
    attach_console();
    match parse(&args[1..]) {
        Ok(job) => match execute(job) {
            Ok(code) => code,
            Err(error) => {
                eprintln!("doze: {error}");
                1
            }
        },
        Err(error) => {
            if !error.is_empty() {
                eprintln!("doze: {error}\n");
            }
            eprintln!("{USAGE}");
            if error.is_empty() {
                0
            } else {
                2
            }
        }
    }
}

#[derive(Debug, PartialEq)]
enum Target {
    Command(Vec<String>),
    Process(u32),
}
#[derive(Debug, PartialEq)]
struct Job {
    target: Target,
    then: &'static str,
    reason: Option<String>,
    endpoint: Option<PathBuf>,
}

fn action(name: &str) -> Result<&'static str, String> {
    Ok(match name {
        "nothing" | "none" | "normal" => "return_to_normal",
        "sleep" => "sleep",
        "display-off" | "display_off" => "display_off",
        "lock" => "lock",
        "shutdown" | "shut-down" => "shutdown",
        "hibernate" => "hibernate",
        _ => return Err(format!("Unknown action \"{name}\".")),
    })
}

fn parse(args: &[String]) -> Result<Job, String> {
    let mut args = args.iter();
    let mode = args.next().map(String::as_str);
    if matches!(mode, Some("help" | "--help") | None) {
        return Err(String::new());
    }
    let mut then = "return_to_normal";
    let mut reason = None;
    let mut endpoint = None;
    let mut pid = None;
    let mut command = Vec::new();
    while let Some(arg) = args.next() {
        let value = |args: &mut std::slice::Iter<String>| {
            args.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs a value."))
        };
        match arg.as_str() {
            "--then" => then = action(&value(&mut args)?)?,
            "--reason" => reason = Some(value(&mut args)?),
            "--endpoint" => endpoint = Some(PathBuf::from(value(&mut args)?)),
            "--pid" if mode == Some("watch") => {
                pid = Some(
                    value(&mut args)?
                        .parse::<u32>()
                        .map_err(|_| "--pid must be a process number.")?,
                )
            }
            "--" if mode == Some("run") => {
                command = args.by_ref().cloned().collect();
            }
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("Unexpected argument \"{other}\".")),
        }
    }
    let target = match mode {
        Some("run") if !command.is_empty() => Target::Command(command),
        Some("run") => {
            return Err("Give the command after --, for example: doze run -- make".into())
        }
        Some("watch") => Target::Process(pid.ok_or("watch needs --pid PID.")?),
        _ => return Err(String::new()),
    };
    Ok(Job {
        target,
        then,
        reason,
        endpoint,
    })
}

fn default_endpoint() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join("Library/Application Support"));
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    Ok(base
        .ok_or("Could not find Doze's settings folder.")?
        .join("app.getdoze.desktop/mcp-endpoint.json"))
}

fn label(target: &Target) -> String {
    let text = match target {
        Target::Command(command) => command.join(" "),
        Target::Process(pid) => format!("process {pid}"),
    };
    // Reasons are short and single-line; Doze rejects control characters.
    let clean: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(200)
        .collect();
    clean
}

fn execute(job: Job) -> Result<i32, String> {
    let endpoint = match job.endpoint.clone() {
        Some(path) => path,
        None => default_endpoint()?,
    };
    let call = {
        let endpoint = endpoint.clone();
        move |name: &str, arguments: Value| {
            forward(
                &endpoint,
                Call {
                    key: String::new(),
                    name: format!("job.{name}"),
                    arguments,
                },
            )
        }
    };
    if let Target::Process(pid) = job.target {
        if !alive(pid) {
            return Err(format!("No running process with id {pid}."));
        }
    }
    let what = label(&job.target);
    let session = call(
        "start_session",
        json!({
            "reason": job.reason.clone().unwrap_or_else(|| format!("Running {what}")),
            "completion_action": job.then,
        }),
    )
    .map_err(|error| format!("Could not reach Doze: {error}"))?;
    let id = session["session_id"]
        .as_str()
        .ok_or("Doze returned no session.")?
        .to_string();
    let then = action_label(job.then);
    eprintln!("doze: keeping this computer awake while {what} runs (then: {then}).");

    let stop = Arc::new(AtomicBool::new(false));
    let lease = session["lease_expires_at"]
        .as_u64()
        .zip(session["last_heartbeat"].as_u64())
        .map_or(300, |(expires, renewed)| expires.saturating_sub(renewed));
    let heartbeat = {
        let (stop, id, call) = (stop.clone(), id.clone(), call.clone());
        std::thread::spawn(move || {
            let interval = (lease / 3).clamp(5, 600);
            let mut warned = false;
            'renew: loop {
                for _ in 0..interval {
                    if stop.load(Ordering::Relaxed) {
                        break 'renew;
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
                match call("heartbeat", json!({ "session_id": id })) {
                    Ok(_) => warned = false,
                    Err(error) if !warned => {
                        warned = true;
                        eprintln!("doze: could not renew the wake request: {error}");
                    }
                    Err(_) => {}
                }
            }
        })
    };

    let outcome = match &job.target {
        Target::Command(command) => run(command),
        Target::Process(pid) => {
            while alive(*pid) {
                std::thread::sleep(Duration::from_secs(2));
            }
            Ok(0)
        }
    };
    stop.store(true, Ordering::Relaxed);
    let _ = heartbeat.join();
    match outcome {
        Ok(0) => {
            call("finish_session", json!({ "session_id": id }))?;
            if job.then == "return_to_normal" {
                eprintln!("doze: done; normal sleep settings apply again.");
            } else {
                eprintln!("doze: done; {then} follows Doze's final warning.");
            }
            Ok(0)
        }
        Ok(code) => {
            call("fail_session", json!({ "session_id": id }))?;
            eprintln!("doze: {what} exited with status {code}; released without {then}.");
            Ok(code)
        }
        Err(error) => {
            call("fail_session", json!({ "session_id": id }))?;
            Err(format!("Could not run {what}: {error}"))
        }
    }
}

fn action_label(action: &str) -> &'static str {
    match action {
        "sleep" => "sleep",
        "display_off" => "turn the display off",
        "lock" => "lock",
        "shutdown" => "shut down",
        "hibernate" => "hibernate",
        _ => "nothing",
    }
}

/// Runs the command with this terminal's input and output and returns its exit status.
fn run(command: &[String]) -> Result<i32, String> {
    let mut child = Command::new(&command[0])
        .args(&command[1..])
        .spawn()
        .map_err(|error| error.to_string())?;
    // Ctrl-C reaches the command, which exits; this process must survive to release the job.
    ignore_interrupts();
    let status = child.wait().map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return Ok(128 + signal);
        }
    }
    Ok(status.code().unwrap_or(1))
}

#[cfg(unix)]
fn ignore_interrupts() {
    extern "C" {
        fn signal(signal: i32, handler: usize) -> usize;
    }
    const SIGINT: i32 = 2;
    const SIG_IGN: usize = 1;
    unsafe {
        signal(SIGINT, SIG_IGN);
    }
}

#[cfg(windows)]
fn ignore_interrupts() {
    use windows::Win32::System::Console::SetConsoleCtrlHandler;
    let _ = unsafe { SetConsoleCtrlHandler(None, true) };
}

#[cfg(windows)]
fn attach_console() {
    use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    // The desktop binary is a GUI program on Windows; reuse the terminal it was started from.
    let _ = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

#[cfg(unix)]
fn alive(pid: u32) -> bool {
    extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }
    let Ok(pid) = i32::try_from(pid) else {
        return false;
    };
    if pid <= 0 {
        return false;
    }
    // Signal 0 checks existence. EPERM means it exists but belongs to another user.
    let exists = unsafe { kill(pid, 0) } == 0;
    exists || std::io::Error::last_os_error().raw_os_error() == Some(1)
}

#[cfg(windows)]
fn alive(pid: u32) -> bool {
    use windows::Win32::{
        Foundation::{CloseHandle, WAIT_TIMEOUT},
        System::Threading::{
            OpenProcess, WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION,
            PROCESS_SYNCHRONIZE,
        },
    };
    unsafe {
        let Ok(handle) = OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            false,
            pid,
        ) else {
            return false;
        };
        let running = WaitForSingleObject(handle, 0) == WAIT_TIMEOUT;
        let _ = CloseHandle(handle);
        running
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(text: &str) -> Vec<String> {
        text.split(' ').map(String::from).collect()
    }

    #[test]
    fn parses_run_and_watch_jobs() {
        assert_eq!(
            parse(&args("run --then sleep -- ffmpeg -i in.mov out.mp4")),
            Ok(Job {
                target: Target::Command(args("ffmpeg -i in.mov out.mp4")),
                then: "sleep",
                reason: None,
                endpoint: None,
            })
        );
        // Options after -- belong to the command.
        assert_eq!(
            parse(&args("run -- make --then sleep")).map(|job| job.then),
            Ok("return_to_normal")
        );
        assert_eq!(
            parse(&args("watch --pid 42 --then display-off")).map(|job| (job.target, job.then)),
            Ok((Target::Process(42), "display_off"))
        );
        assert!(parse(&args("run --then reboot -- make")).is_err());
        assert!(parse(&args("run")).is_err());
        assert!(parse(&args("watch")).is_err());
        assert!(parse(&args("run --pid 3 -- make")).is_err());
        assert_eq!(parse(&args("help")), Err(String::new()));
        assert!(requested(&args("doze run -- make")));
        assert!(!requested(&args("doze --startup")));
        assert!(!requested(&args("doze --mcp --endpoint x")));
    }

    #[test]
    fn reasons_are_single_line_and_bounded() {
        let long = Target::Command(vec!["echo".into(), "a\nb".into(), "x".repeat(400)]);
        let text = label(&long);
        assert!(!text.chars().any(char::is_control));
        assert_eq!(text.chars().count(), 200);
    }

    #[cfg(unix)]
    #[test]
    fn processes_are_followed_by_id() {
        assert!(alive(std::process::id()));
        assert!(!alive(0));
        let mut child = Command::new("/bin/sleep").arg("5").spawn().unwrap();
        let pid = child.id();
        assert!(alive(pid));
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(!alive(pid));
    }
}
