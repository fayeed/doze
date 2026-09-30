//! Test-only host. Never links native power adapters or runs a Tauri application.
#![allow(dead_code)]
#[path = "../../src/core/mod.rs"]
mod core;
#[path = "../../src/mcp/mod.rs"]
mod mcp;
mod state {
    pub enum Request {
        Control(String),
        Mcp(
            crate::mcp::tools::Call,
            std::sync::mpsc::Sender<Result<serde_json::Value, String>>,
        ),
    }
}
mod platform {
    pub trait PowerManager {
        fn supported_actions(&self) -> Vec<crate::core::sessions::PowerAction>;
        fn set_awake(&mut self, active: bool, allow_display_sleep: bool) -> Result<(), String>;
        fn execute(&mut self, action: crate::core::sessions::PowerAction) -> Result<(), String>;
    }
}
#[derive(Default)]
struct MockPower {
    awake: bool,
    executed: usize,
}
impl platform::PowerManager for MockPower {
    fn supported_actions(&self) -> Vec<core::sessions::PowerAction> {
        vec![
            core::sessions::PowerAction::Sleep,
            core::sessions::PowerAction::Shutdown,
        ]
    }
    fn set_awake(&mut self, active: bool, _: bool) -> Result<(), String> {
        self.awake = active;
        Ok(())
    }
    fn execute(&mut self, _: core::sessions::PowerAction) -> Result<(), String> {
        self.executed += 1;
        Ok(())
    }
}
fn main() {
    if std::env::args().any(|a| a == "--mcp") {
        if let Err(e) = mcp::server::bridge() {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    use platform::PowerManager;
    let args: Vec<_> = std::env::args().collect();
    let endpoint = std::path::PathBuf::from(&args[1]);
    let mut settings = core::sessions::Settings::default();
    settings.agents.enabled = true;
    for id in ["codex", "claude"] {
        settings.agents.clients.push(mcp::auth::TrustedClient {
            id: id.into(),
            name: id.into(),
            secret: format!("{id}-{}", "x".repeat(32)),
            keep_awake: true,
            actions: vec![core::sessions::PowerAction::Sleep],
        });
    }
    let (sender, receiver) = std::sync::mpsc::channel();
    mcp::server::start(endpoint, sender.clone()).unwrap();
    let commands = sender.clone();
    std::thread::spawn(move || {
        use std::io::BufRead;
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            let _ = commands.send(state::Request::Control(line));
        }
    });
    println!("ready");
    let mut engine = core::sessions::Engine::default();
    let mut power = MockPower::default();
    for request in receiver {
        let (call, reply) = match request {
            state::Request::Mcp(call, reply) => (call, reply),
            state::Request::Control(command) => {
                if command == "cancel" {
                    engine.cancel_countdown();
                }
                if command == "expire" {
                    engine.now += 1000;
                    engine.tick(engine.now, None, None, false, &settings);
                }
                power.set_awake(engine.should_hold_awake(), false).unwrap();
                println!(
                    "{}",
                    serde_json::json!({"awake":power.awake,"countdown":engine.countdown.is_some(),"executed":power.executed})
                );
                continue;
            }
        };
        engine.now += 1;
        let mut result = mcp::tools::call(&mut engine, &settings, &power.supported_actions(), call);
        if let Some(action) = engine.tick(engine.now, None, None, false, &settings) {
            power.execute(action).unwrap();
        }
        power.set_awake(engine.should_hold_awake(), false).unwrap();
        if let Ok(ref mut value) = result {
            if value.is_object() {
                value["test_awake"] = serde_json::json!(power.awake);
                value["test_countdown"] = serde_json::json!(engine.countdown.is_some());
                value["test_executed"] = serde_json::json!(power.executed);
                // Represents the same snapshot used by the native tray and UI.
                value["test_ui_sessions"] = serde_json::json!(engine.agents.items.len());
            }
        }
        let _ = reply.send(result);
    }
}
