//! Native warning UI, driven by the engine's remaining time rather than a second timer.
use crate::{
    core::{
        countdown::{Countdown, Source},
        sessions::{PowerAction, Theme},
    },
    platform::native_ui,
    state::Request,
};
use serde_json::json;
use std::{cell::RefCell, sync::mpsc::Sender};

#[derive(Default)]
struct WarningState {
    visible: bool,
    snoozed: Option<(u64, PowerAction, Source, u64)>,
}

impl WarningState {
    fn update(
        &mut self,
        countdown: Option<&Countdown>,
        now: u64,
        snoozing: bool,
    ) -> Option<serde_json::Value> {
        if let Some(countdown) = countdown {
            if snoozing {
                self.snoozed = Some((
                    countdown.deadline,
                    countdown.action,
                    countdown.source,
                    now.saturating_add(900),
                ));
            }
        } else {
            self.snoozed = None;
        }
        let suppressed = countdown.is_some_and(|countdown| {
            self.snoozed
                .is_some_and(|(deadline, action, source, resume_at)| {
                    countdown.deadline == deadline
                        && countdown.action == action
                        && countdown.source == source
                        && now < resume_at
                })
        });
        let shown = countdown.filter(|_| !suppressed);
        if shown.is_none() && !self.visible {
            return None;
        }
        self.visible = shown.is_some();
        Some(
            json!({ "type": "countdown", "countdown": shown.map(|countdown| json!({
            "action": countdown.action.label(),
            "remaining": countdown.deadline.saturating_sub(now),
        })) }),
        )
    }
}

pub struct Warning {
    requests: Sender<Request>,
    state: RefCell<WarningState>,
}

impl Warning {
    pub fn new(requests: Sender<Request>) -> Result<Self, String> {
        native_ui::executable()?;
        Ok(Self {
            requests,
            state: RefCell::new(WarningState::default()),
        })
    }

    pub fn update(&self, countdown: Option<&Countdown>, now: u64, snoozing: bool, theme: Theme) {
        let Some(mut message) = self.state.borrow_mut().update(countdown, now, snoozing) else {
            return;
        };
        message["theme"] = json!(theme);
        if let Err(error) = native_ui::send(message, self.requests.clone()) {
            let _ = self.requests.send(Request::WarningFailed(error));
        }
    }

    pub fn preview(&self, action: PowerAction, theme: Theme) {
        if let Err(error) = native_ui::send(
            json!({ "type": "preview", "action": action.label(), "theme": theme }),
            self.requests.clone(),
        ) {
            eprintln!("Could not preview native countdown: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paused_playback_reaches_native_warning_after_idle_wait() {
        use crate::core::sessions::{Engine, Settings};
        let mut engine = Engine::default();
        let settings = Settings::default();
        let mut warning = WarningState::default();
        engine.enable_playback(true);
        for now in 0..3 {
            engine.tick(now, Some(true), Some(0), false, &settings);
        }
        engine.tick(3, Some(false), Some(0), true, &settings);
        assert!(warning
            .update(engine.countdown.as_ref(), 3, false)
            .is_none());
        engine.tick(303, Some(false), Some(300), false, &settings);
        let message = warning
            .update(engine.countdown.as_ref(), 303, false)
            .unwrap();
        assert_eq!(message["type"], "countdown");
        assert_eq!(message["countdown"]["remaining"], 300);
    }

    #[test]
    fn snooze_hides_warning_for_fifteen_minutes_then_restores_it() {
        let mut state = WarningState::default();
        let mut countdown = Countdown {
            deadline: 300,
            action: PowerAction::Sleep,
            source: Source::Timer,
        };
        assert_eq!(
            state.update(Some(&countdown), 0, false).unwrap()["countdown"]["remaining"],
            300
        );
        countdown.deadline += 900;
        assert!(state.update(Some(&countdown), 0, true).unwrap()["countdown"].is_null());
        assert!(state.update(Some(&countdown), 899, false).is_none());
        assert_eq!(
            state.update(Some(&countdown), 900, false).unwrap()["countdown"]["remaining"],
            300
        );
    }

    #[test]
    fn a_new_countdown_is_not_hidden_by_an_old_snooze() {
        let mut state = WarningState::default();
        let mut countdown = Countdown {
            deadline: 1200,
            action: PowerAction::Sleep,
            source: Source::Timer,
        };
        state.update(Some(&countdown), 0, true);
        countdown.deadline = 600;
        assert!(state.update(Some(&countdown), 1, false).unwrap()["countdown"].is_object());
        assert!(state.update(None, 2, false).unwrap()["countdown"].is_null());
        assert!(state.update(None, 3, false).is_none());
    }
}
