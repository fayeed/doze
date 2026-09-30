//! User-facing explanations for the native tray controls.
use crate::state::Snapshot;

pub(super) fn text(snapshot: &Snapshot) -> String {
    let settings = &snapshot.settings;
    let display = if settings.allow_display_sleep {
        "The screen may sleep while the system stays awake."
    } else {
        "The screen stays awake too."
    };
    format!(
        "{status}\n\n\
         STATUS AND GREY OPTIONS\n\
         Normal sleep allowed: Doze is not preventing sleep. Your normal Windows power settings apply.\n\
         The top two menu rows report the current state. Click either to open this help.\n\
         Grey options are unavailable right now. Stop needs an active session; Extend needs a timed awake session; Cancel and Snooze need a running countdown. Grey power actions are not supported on this computer.\n\n\
         KEEP AWAKE\n\
         Choose how long to prevent automatic system sleep. {display} Default starts a {awake_minutes}-minute session. Custom duration accepts minutes; a specific time accepts a local date and time. Indefinitely lasts until you stop it or quit Doze.\n\
         Stop keeping awake ends manual and audio-based awake sessions. Extend adds 15 minutes to a timed awake session.\n\n\
         KEEP AWAKE WHILE AUDIO PLAYS\n\
         Check this to keep the computer awake during audible playback and short pauses. It releases the awake request after {silence} seconds of silence. A checkmark means enabled; it may be waiting for audio.\n\n\
         POWER TIMER\n\
         Open Power Timer for the action, duration, and Stop timer controls.\n\
         Timer action chooses what happens at the end. Currently selected: {timer_action}.\n\
         Choose a duration before the final warning begins. For example, a 15-minute timer is followed by your {countdown}-second countdown. Doze keeps the computer awake while the timer is running. Stop timer removes that timer and its countdown.\n\
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
         The Countdown submenu contains Cancel, Snooze and Preview. Preview opens a demonstration. Its buttons only affect the preview and cannot trigger a power action.\n\n\
         QUICK SETTINGS\n\
         Checkmarks show saved preferences, rather than whether a session is running. Changes save immediately. Default durations apply to new sessions. Display sleep changes take effect on the current awake request; notification changes apply to future notifications.\n\n\
         SETTINGS, ABOUT AND QUIT\n\
         Settings has General, Session Defaults, After Playback, Notifications and Advanced tabs. Save applies changes; Cancel discards edits. Reset all fills in defaults for review, then Save applies them. Advanced shows local diagnostics and opens your data folder. About shows version, privacy information and acknowledgements.\n\
         Quit stops Doze and its sessions. Windows resumes its normal power settings. Sessions are also cleared after a restart or suspend/resume.",
        status = crate::tray::status_text(snapshot),
        silence = settings.silence_seconds,
        idle = settings.idle_seconds,
        countdown = settings.countdown_seconds,
        timer_action = snapshot.selected_action.label(),
        playback_action = settings.playback_action.label(),
        display = display,
        awake_minutes = settings.default_awake_minutes,
    )
}

pub(super) fn about(snapshot: &Snapshot) -> String {
    format!(
        "Keep your computer awake when it should be awake, and let it sleep when it should sleep. Doze lives in the tray, with native settings, timers and countdown warnings.\n\nLOCAL AND PRIVATE\nNo account, cloud service, ads, subscriptions or telemetry. Audio monitoring observes output levels; it does not record your audio. Preferences and optional error logs stay on this computer.\n\nTHIS RUN\nRunning for {} minutes. {}\n\nACKNOWLEDGEMENTS\nBuilt with Rust and Tauri. Windows API bindings, native power management, Core Audio and Windows notifications provide the system integration.\n\nDATA\nSettings: {}\n\nUse 'Help & About → Menu Guide' in the tray for feature explanations. Advanced Settings provides diagnostics and access to the data folder.",
        snapshot.engine.now / 60,
        crate::tray::status_text(snapshot), snapshot.settings_path.display(),
    )
}
