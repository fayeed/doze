use super::{
    countdown::{Countdown, Source},
    sessions::{Engine, Phase, PowerAction, Settings, Timer},
};

impl Engine {
    pub fn keep_awake(&mut self, seconds: Option<u64>) {
        self.awake = true;
        self.awake_deadline = seconds.map(|s| self.now.saturating_add(s));
        self.cancel_playback();
        self.message = None;
    }
    pub fn stop_awake(&mut self) {
        self.awake = false;
        self.awake_deadline = None;
    }
    pub fn extend_awake(&mut self, seconds: u64) -> Result<(), String> {
        if !self.awake {
            return Err("No keep-awake session is active.".into());
        }
        if let Some(deadline) = self.awake_deadline {
            self.awake_deadline = Some(deadline.saturating_add(seconds));
        }
        Ok(())
    }
    pub fn schedule(&mut self, seconds: u64, action: PowerAction) {
        self.cancel_countdown();
        self.timer = Some(Timer {
            deadline: self.now.saturating_add(seconds),
            action,
        });
        self.message = None;
    }
    pub fn enable_playback(&mut self, enabled: bool) {
        self.cancel_playback();
        self.playback_enabled = enabled;
        self.playback_phase = Phase::Idle;
        self.audio_streak = 0;
    }
    pub fn cancel_playback(&mut self) {
        if self
            .countdown
            .as_ref()
            .is_some_and(|c| c.source == Source::Playback)
        {
            self.countdown = None;
        }
        self.playback_phase = Phase::Cancelled;
        self.silence_since = None;
        self.audio_streak = 0;
    }
    pub fn cancel_countdown(&mut self) {
        if let Some(c) = self.countdown.take() {
            if c.source == Source::Agents {
                self.agents.completion_consumed = true;
            }
            if c.source == Source::Playback {
                self.cancel_playback();
            }
            self.message = Some("Power action cancelled".into());
        }
    }
    pub fn snooze(&mut self) -> Result<(), String> {
        let c = self.countdown.as_mut().ok_or("No countdown is active.")?;
        c.deadline = c.deadline.saturating_add(900);
        Ok(())
    }
    pub fn reset_transient(&mut self, reason: &str) {
        self.agents.uncertain(self.now);
        self.agents.completion_consumed = true;
        self.stop_awake();
        self.while_audio = false;
        self.timer = None;
        self.countdown = None;
        self.cancel_playback();
        self.audio_active = false;
        self.message = Some(reason.into());
    }
    pub fn needs_audio(&self) -> bool {
        self.playback_enabled || self.while_audio
    }
    pub fn should_hold_awake(&self) -> bool {
        self.agents.holds_awake()
            || self.awake
            || (self.while_audio && (self.audio_active || self.silence_since.is_some()))
            || self.countdown.is_some()
            || self.timer.is_some()
            || (self.playback_enabled
                && matches!(
                    self.playback_phase,
                    Phase::Active | Phase::GracePeriod | Phase::Countdown
                ))
    }
    // All deadlines are monotonic seconds. Only the platform adapter produces observations.
    pub fn tick(
        &mut self,
        now: u64,
        audio: Option<bool>,
        idle: Option<u64>,
        user_active: bool,
        settings: &Settings,
    ) -> Option<PowerAction> {
        self.now = now;
        self.agents.expire(now);
        // Pause other power sources while agent work is unsettled. Do not continually
        // restart visible countdowns or emit a notification every engine tick.
        if self.agents.holds_awake() {
            if let Some(countdown) = self.countdown.take() {
                match countdown.source {
                    Source::Timer => {
                        self.timer = Some(Timer {
                            deadline: now,
                            action: countdown.action,
                        })
                    }
                    Source::Playback => {
                        self.playback_phase = Phase::GracePeriod;
                        self.silence_since.get_or_insert(now);
                    }
                    Source::Agents => self.agents.completion_consumed = true,
                }
            }
        }
        if self.awake_deadline.is_some_and(|d| now >= d) {
            self.stop_awake();
        }
        let previously_playing = self.audio_active;
        self.audio_active = audio == Some(true);
        if self.needs_audio() {
            match audio {
                None => {
                    self.cancel_playback();
                    self.silence_since = None;
                }
                Some(true) => {
                    self.silence_since = None;
                    if self.last_audio_at != Some(now) {
                        self.audio_streak = self.audio_streak.saturating_add(1);
                    }
                    self.last_audio_at = Some(now);
                    // Three meaningful observations avoid arming on a short system chime.
                    if self.playback_enabled
                        && (self.audio_streak >= 3
                            || matches!(self.playback_phase, Phase::GracePeriod | Phase::Countdown))
                    {
                        self.cancel_playback();
                        self.audio_streak = 3;
                        self.playback_phase = Phase::Active;
                    }
                }
                Some(false) => {
                    self.audio_streak = 0;
                    if (self.while_audio && previously_playing)
                        || self.playback_phase == Phase::Active
                    {
                        self.silence_since.get_or_insert(now);
                    }
                    if self.playback_enabled && self.playback_phase == Phase::Active {
                        self.playback_phase = Phase::GracePeriod;
                    }
                }
            }
            // A single resumed-audio sample must cancel an existing countdown immediately.
            if audio == Some(true)
                && self
                    .countdown
                    .as_ref()
                    .is_some_and(|c| c.source == Source::Playback)
            {
                self.cancel_playback();
                self.playback_phase = Phase::Active;
            }
            let playback_due_without_idle = self.countdown.as_ref().is_some_and(|countdown| {
                countdown.source == Source::Playback
                    && now >= countdown.deadline
                    && idle.is_none_or(|seconds| seconds < settings.idle_seconds)
            });
            if self.playback_enabled
                && (idle.is_none()
                    || playback_due_without_idle
                    || (user_active && self.playback_phase == Phase::Countdown))
                && matches!(self.playback_phase, Phase::GracePeriod | Phase::Countdown)
            {
                self.cancel_playback();
            }
            // Pausing playback is itself input. During grace, input resets the platform's
            // idle duration; keep waiting instead of permanently disarming the rule.
            // After Playback reuses the silence start only while it is armed and waiting.
            // Otherwise an unarmed rule would let a short chime hold the computer awake forever.
            let playback_waiting = self.playback_enabled
                && matches!(self.playback_phase, Phase::GracePeriod | Phase::Countdown);
            if self.while_audio
                && self
                    .silence_since
                    .is_some_and(|s| now.saturating_sub(s) >= settings.silence_seconds)
                && !playback_waiting
            {
                self.silence_since = None;
            }
            if self.playback_enabled
                && self.playback_phase == Phase::GracePeriod
                && self
                    .silence_since
                    .is_some_and(|s| now.saturating_sub(s) >= settings.silence_seconds)
                && idle.is_some_and(|i| i >= settings.idle_seconds)
                && !user_active
                && !self.agents.holds_awake()
                && !self.awake
                && self.timer.is_none()
                && self.countdown.is_none()
            {
                self.countdown = Some(Countdown {
                    deadline: now.saturating_add(settings.countdown_seconds),
                    action: settings.playback_action,
                    source: Source::Playback,
                });
                self.playback_phase = Phase::Countdown;
            }
        }
        if !self.agents.holds_awake() && self.timer.as_ref().is_some_and(|t| now >= t.deadline) {
            let timer = self.timer.take()?;
            self.cancel_playback();
            self.countdown = Some(Countdown {
                deadline: now.saturating_add(settings.countdown_seconds),
                action: timer.action,
                source: Source::Timer,
            });
        }
        // Every power path respects agent wake leases, including timers already counting down.
        let other_wake_required =
            self.awake || self.while_audio || self.timer.is_some() || self.playback_enabled;
        if !(self.agents.holds_awake() || other_wake_required || self.countdown.is_some()) {
            if let Some(action) = self.agents.completion() {
                self.countdown = Some(Countdown {
                    deadline: now.saturating_add(settings.countdown_seconds.max(300)),
                    action,
                    source: Source::Agents,
                });
                self.message = Some("All agents finished".into());
            }
        }
        if self
            .countdown
            .as_ref()
            .is_some_and(|c| c.source == Source::Agents)
            && (self.awake || self.while_audio || self.timer.is_some())
        {
            self.cancel_countdown();
        }
        if self.countdown.as_ref().is_some_and(|c| now >= c.deadline) {
            let c = self.countdown.take()?;
            if c.source == Source::Agents {
                self.agents.completion_consumed = true;
            }
            self.stop_awake();
            self.while_audio = false;
            self.playback_phase = Phase::Completed;
            self.silence_since = None;
            self.message = Some(format!("{} requested", c.action.label()));
            return Some(c.action);
        }
        None
    }
}
