//! `doze-cli.exe`: the console front end for `doze run`, `doze watch` and `doze hook` on Windows.
//!
//! `doze.exe` is a GUI program, so cmd.exe and PowerShell return to the prompt without waiting
//! for it, drop its exit status and cannot capture its output. This console program starts the
//! `doze.exe` beside it with the same arguments and terminal, waits, and exits with its status.
//! It never starts the desktop app: anything other than a job command shows the usage.
use std::{
    ffi::OsString,
    path::PathBuf,
    process::{exit, Command},
};

fn main() {
    let mut args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let job = matches!(
        args.first().and_then(|arg| arg.to_str()),
        Some("run" | "watch" | "hook" | "agent-event" | "help" | "--help" | "-h")
    );
    let unknown = !job && !args.is_empty();
    if unknown {
        eprintln!("doze: unknown command {:?}.\n", args[0]);
    }
    if !job {
        args = vec!["help".into()];
    }
    let doze = match beside_this_program() {
        Some(path) if path.is_file() => path,
        _ => {
            eprintln!("doze: doze.exe was not found beside doze-cli.exe. Reinstall Doze.");
            exit(1)
        }
    };
    let mut child = match Command::new(&doze).args(&args).spawn() {
        Ok(child) => child,
        Err(error) => {
            eprintln!("doze: could not start {}: {error}", doze.display());
            exit(1)
        }
    };
    // After the spawn, so the job still receives Ctrl-C: doze.exe then reports the failure
    // and releases the computer, and this program waits to return its status.
    ignore_interrupts();
    let code = match child.wait() {
        Ok(status) => status.code().unwrap_or(1),
        Err(error) => {
            eprintln!("doze: {error}");
            1
        }
    };
    exit(if unknown { 2 } else { code })
}

fn beside_this_program() -> Option<PathBuf> {
    let name = if cfg!(windows) { "doze.exe" } else { "doze" };
    Some(std::env::current_exe().ok()?.with_file_name(name))
}

#[cfg(windows)]
fn ignore_interrupts() {
    type Handler = unsafe extern "system" fn(u32) -> i32;
    #[link(name = "kernel32")]
    extern "system" {
        fn SetConsoleCtrlHandler(handler: Option<Handler>, add: i32) -> i32;
    }
    // SAFETY: a null handler with add=TRUE only makes this process ignore Ctrl-C.
    unsafe {
        SetConsoleCtrlHandler(None, 1);
    }
}

#[cfg(not(windows))]
fn ignore_interrupts() {}
