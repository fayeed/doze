//! User-facing explanations for the native tray controls.
use crate::state::Snapshot;

pub(super) fn text(snapshot: &Snapshot) -> String {
    let settings = &snapshot.settings;
    format!(
        "{status}\n\n\
         STATUS AND GREY OPTIONS\n\
         Normal sleep allowed: Doze is not preventing sleep. Your normal Windows power settings apply.\n\
         The top two menu rows report the current state. Click either to open this help.\n\
         Grey options are unavailable right now. Stop needs an active session; Extend needs a timed awake session; Cancel and Snooze need a running countdown. Grey power actions are not supported on this computer.\n\n\
         KEEP AWAKE\n\
         Choose how long to prevent automatic sleep and keep the display awake. Custom duration accepts minutes; a specific time accepts a local date and time. Indefinitely lasts until you stop it or quit Doze.\n\
         Stop keeping awake ends manual and audio-based awake sessions. Extend adds 15 minutes to a timed awake session.\n\n\
         KEEP AWAKE WHILE AUDIO PLAYS\n\
         Check this to keep the computer awake during audible playback and short pauses. It releases the awake request after {silence} seconds of silence. A checkmark means enabled; it may be waiting for audio.\n\n\
         TIMER ACTION AND START TIMER\n\
         Timer action chooses what happens at the end. Currently selected: {timer_action}.\n\
         Start timer chooses the duration before the final warning begins. For example, a 15-minute timer is followed by your {countdown}-second countdown. Doze keeps the computer awake while the timer is running. Stop timer removes that timer and its countdown.\n\
         Sleep: suspend the computer, keeping your session in memory.\n\
         Hibernate: save your session to disk and power down.\n\
         Shut down: close Windows and turn off the computer. Windows may ask about unsaved work.\n\
         Lock: lock your Windows session.\n\
         Turn display off: switch off the screen.\n\n\
         {playback_action} AFTER PLAYBACK STOPS\n\
         Check this to watch for ongoing audio playback, then perform {playback_action} after it stops. Brief sounds do not arm the rule. It waits for {silence} seconds of silence and {idle} seconds without keyboard or mouse activity before the final countdown. Resumed audio or user activity cancels the pending playback action.\n\
         Manual Keep Awake blocks this automatic action. A timer you start takes priority.\n\n\
         COUNTDOWN, CANCEL AND SNOOZE\n\
         The native warning window shows the action and time remaining. Cancel, Escape, or closing the warning cancels the action. Snooze adds 15 minutes to the countdown.\n\
         Preview countdown opens a demonstration. Its buttons only affect the preview and cannot trigger a power action.\n\n\
         SETTINGS AND QUIT\n\
         Settings changes launch at sign-in, starting in the tray, notifications, default actions, silence/idle/countdown durations, and local logging.\n\
         Quit stops Doze and its sessions. Windows resumes its normal power settings. Sessions are also cleared after a restart or suspend/resume.",
        status = crate::tray::status_text(snapshot),
        silence = settings.silence_seconds,
        idle = settings.idle_seconds,
        countdown = settings.countdown_seconds,
        timer_action = snapshot.selected_action.label(),
        playback_action = settings.playback_action.label(),
    )
}
