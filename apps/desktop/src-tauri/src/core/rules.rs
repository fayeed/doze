use super::{
    countdown::{Countdown, Source},
    sessions::{Engine, Event, Phase, PowerAction, Settings, Timer},
};

/// How long the user must be away before finished agents start the final warning.
pub const AGENT_FINISH_IDLE_SECONDS: u64 = 120;

impl Engine {
    /// Whether finished agents are waiting for the user to step away, so the runtime keeps
    /// observing input.
    pub fn completion_waiting(&self, settings: &Settings) -> bool {
        self.countdown.is_none()
            && self
                .agents
                .completion(settings.agents.default_completion)
                .is_some()
    }
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
    /// Cancel or Stay Awake chosen by the user. After Playback is one-shot: dismissing its
    /// warning turns the rule off, so it cannot trip again later in the day. Automatic
    /// cancellations (input or resumed audio) keep it armed for fresh playback.
    pub fn dismiss_countdown(&mut self) {
        let playback = self
            .countdown
            .as_ref()
            .is_some_and(|c| c.source == Source::Playback);
        self.cancel_countdown();
        if playback {
            self.playback_enabled = false;
            self.message = Some("Power action cancelled · After Playback turned off".into());
        }
    }
    pub fn snooze(&mut self, seconds: u64) -> Result<(), String> {
        let c = self.countdown.as_mut().ok_or("No countdown is active.")?;
        c.deadline = c.deadline.saturating_add(seconds);
        Ok(())
    }
    /// Battery guard: on battery below the floor, release every agent lease and manual
    /// keep-awake once, notify, and keep agents from holding until power returns.
    /// Timers and countdowns are the user's own plans and stay.
    pub fn battery(&mut self, percent: u8, on_battery: bool, floor: u8) {
        self.battery = Some((percent, on_battery));
        let low = floor > 0 && on_battery && percent < floor;
        if !low {
            self.battery_low = false;
            return;
        }
        let mut released = self.release_for_battery();
        if self.awake || self.while_audio {
            self.stop_awake();
            self.while_audio = false;
            released = true;
        }
        if released || !self.battery_low {
            self.message = Some(format!("Battery at {percent}% · stopped keeping awake"));
        }
        if released && !self.battery_low {
            self.events.push(Event::BatteryGuard { percent });
        }
        self.battery_low = true;
    }
    fn release_for_battery(&mut self) -> bool {
        let mut released = false;
        for session in &mut self.agents.items {
            if session.holds() {
                session.status = crate::mcp::sessions::Status::Cancelled;
                released = true;
            }
        }
        if released {
            self.agents.completion_consumed = true;
            if self
                .countdown
                .as_ref()
                .is_some_and(|c| c.source == Source::Agents)
            {
                self.countdown = None;
            }
        }
        released
    }
    pub fn reset_transient(&mut self, reason: &str) {
        self.agents.uncertain(self.now);
        self.agents.completion_consumed = true;
        self.stop_awake();
        self.while_audio = false;
        self.timer = None;
        self.countdown = None;
        self.cancel_playback();
        // After Playback is one-shot like the timer: a sleep from elsewhere (Start menu, lid,
        // power button) has done its job, so it must not stay armed for tomorrow's music.
        self.playback_enabled = false;
        self.audio_active = false;
        self.message = Some(reason.into());
    }
    pub fn needs_audio(&self) -> bool {
        self.playback_enabled || self.while_audio
    }
    /// Keep awake while audio plays only holds during audio and its silence grace.
    fn audio_holds_awake(&self) -> bool {
        self.while_audio && (self.audio_active || self.silence_since.is_some())
    }
    pub fn should_hold_awake(&self) -> bool {
        self.agents.holds_awake()
            || self.awake
            || self.audio_holds_awake()
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
        for agent in self.agents.expire(now) {
            self.events.push(Event::AgentStalled { agent });
        }
        if self.battery_low {
            self.release_for_battery();
        }
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
            self.events.push(Event::KeepAwakeEnded);
        }
        let previously_playing = self.audio_active;
        self.audio_active = audio == Some(true);
        if self.needs_audio() {
            match audio {
                None => {
                    // A transient device or idle-monitor error is not evidence that the
                    // playback rule was cancelled. If playback had been heard, preserve
                    // the armed state and require a fresh silence/inactivity interval.
                    if self.playback_enabled {
                        if self.playback_phase == Phase::Active {
                            self.silence_since = None;
                        } else if matches!(
                            self.playback_phase,
                            Phase::GracePeriod | Phase::Countdown
                        ) {
                            if self
                                .countdown
                                .as_ref()
                                .is_some_and(|c| c.source == Source::Playback)
                            {
                                self.countdown = None;
                            }
                            self.playback_phase = Phase::GracePeriod;
                            self.silence_since = Some(now);
                        }
                    } else {
                        self.silence_since = None;
                    }
                    self.audio_streak = 0;
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
                // Activity or an unavailable idle observation resets the inactivity wait,
                // but playback has already been heard. Keep After Playback armed and start
                // a fresh silence/idle interval instead of requiring new audio.
                if self
                    .countdown
                    .as_ref()
                    .is_some_and(|c| c.source == Source::Playback)
                {
                    self.countdown = None;
                }
                self.playback_phase = Phase::GracePeriod;
                self.silence_since = Some(now);
                self.audio_streak = 0;
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
        // An enabled After Playback rule deliberately takes precedence over agent completion.
        // Audio keep-awake only defers completion while audio is actually holding the computer.
        let other_wake_required =
            self.awake || self.audio_holds_awake() || self.timer.is_some() || self.playback_enabled;
        // When the last working agent ends, the final warning starts once the user has been
        // away from the keyboard and mouse for a moment, so finishing a turn while they watch
        // does not put the computer to sleep under them. Without an idle reading it starts
        // at once; it is always cancellable.
        if !(self.agents.holds_awake() || other_wake_required || self.countdown.is_some()) {
            if let Some(action) = self.agents.completion(settings.agents.default_completion) {
                if idle.is_none_or(|seconds| seconds >= AGENT_FINISH_IDLE_SECONDS) {
                    self.countdown = Some(Countdown {
                        deadline: now.saturating_add(settings.agent_warning_seconds()),
                        action,
                        source: Source::Agents,
                    });
                    self.message = Some("All agents finished".into());
                    self.events.push(Event::AgentsFinished { action });
                }
            }
        }
        if self
            .countdown
            .as_ref()
            .is_some_and(|c| c.source == Source::Agents)
            && (self.awake || self.audio_holds_awake() || self.timer.is_some())
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
            // One-shot: after its action runs, After Playback must be turned on again.
            if c.source == Source::Playback {
                self.playback_enabled = false;
            }
            self.silence_since = None;
            self.message = Some(format!("{} requested", c.action.label()));
            return Some(c.action);
        }
        None
    }
}
