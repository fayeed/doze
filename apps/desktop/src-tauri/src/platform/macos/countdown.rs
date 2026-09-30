//! Native warning UI, driven by the engine's remaining time rather than a second timer.
use crate::{
    core::{countdown::Countdown, sessions::PowerAction},
    platform::native_ui,
    state::Request,
};
use serde_json::json;
use std::{cell::Cell, sync::mpsc::Sender};

pub struct Warning {
    requests: Sender<Request>,
    visible: Cell<bool>,
}

impl Warning {
    pub fn new(requests: Sender<Request>) -> Result<Self, String> {
        native_ui::executable()?;
        Ok(Self {
            requests,
            visible: Cell::new(false),
        })
    }

    pub fn update(&self, countdown: Option<&Countdown>, now: u64) {
        if countdown.is_none() && !self.visible.replace(countdown.is_some()) {
            return;
        }
        self.visible.set(countdown.is_some());
        let state = countdown.map(|countdown| {
            json!({
                "action": countdown.action.label(),
                "remaining": countdown.deadline.saturating_sub(now),
            })
        });
        if let Err(error) = native_ui::send(
            json!({ "type": "countdown", "countdown": state }),
            self.requests.clone(),
        ) {
            let _ = self.requests.send(Request::WarningFailed(error));
        }
    }

    pub fn preview(&self, action: PowerAction) {
        if let Err(error) = native_ui::send(
            json!({ "type": "preview", "action": action.label() }),
            self.requests.clone(),
        ) {
            eprintln!("Could not preview native countdown: {error}");
        }
    }
}
