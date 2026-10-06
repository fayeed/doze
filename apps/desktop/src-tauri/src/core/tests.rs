use super::{
    countdown::Source,
    sessions::{Engine, Phase, PowerAction, Settings},
};
fn audio(e: &mut Engine, t: u64, value: bool) -> Option<PowerAction> {
    e.tick(t, Some(value), Some(600), false, &Settings::default())
}
fn armed() -> Engine {
    let mut e = Engine::default();
    e.enable_playback(true);
    for t in 0..3 {
        audio(&mut e, t, true);
    }
    e
}
#[test]
fn silence_alone_never_arms() {
    let mut e = Engine::default();
    e.enable_playback(true);
    audio(&mut e, 10000, false);
    assert!(e.countdown.is_none());
}
#[test]
fn chime_does_not_arm() {
    let mut e = Engine::default();
    e.enable_playback(true);
    audio(&mut e, 0, true);
    audio(&mut e, 1, false);
    audio(&mut e, 1000, false);
    assert!(e.countdown.is_none());
}
#[test]
fn grace_then_countdown_then_action_once() {
    let mut e = armed();
    audio(&mut e, 3, false);
    audio(&mut e, 62, false);
    assert!(e.countdown.is_none());
    audio(&mut e, 63, false);
    assert_eq!(e.playback_phase, Phase::Countdown);
    assert_eq!(audio(&mut e, 363, false), Some(PowerAction::Sleep));
    assert_eq!(audio(&mut e, 364, false), None);
    // One-shot: the rule turns itself off after its action, so daytime audio cannot trip it.
    assert!(!e.playback_enabled);
    for t in 365..370 {
        audio(&mut e, t, true);
    }
    assert_eq!(audio(&mut e, 10000, false), None);
    assert!(e.countdown.is_none());
}
#[test]
fn dismissing_the_playback_warning_turns_the_rule_off() {
    let mut e = armed();
    audio(&mut e, 3, false);
    audio(&mut e, 63, false);
    assert_eq!(e.playback_phase, Phase::Countdown);
    e.dismiss_countdown();
    assert!(e.countdown.is_none());
    assert!(!e.playback_enabled);
    // A timer warning dismissed by the user leaves an armed playback rule alone.
    let mut e = armed();
    e.schedule(10, PowerAction::Lock);
    e.enable_playback(true);
    e.tick(20, Some(false), Some(600), false, &Settings::default());
    assert!(e
        .countdown
        .as_ref()
        .is_some_and(|c| c.source == Source::Timer));
    e.dismiss_countdown();
    assert!(e.playback_enabled);
}
#[test]
fn buffering_restarts_grace() {
    let mut e = armed();
    audio(&mut e, 3, false);
    audio(&mut e, 40, true);
    audio(&mut e, 41, false);
    audio(&mut e, 64, false);
    assert!(e.countdown.is_none());
    audio(&mut e, 101, false);
    assert_eq!(e.playback_phase, Phase::Countdown);
}

#[test]
fn repeated_observations_in_one_second_do_not_arm_playback() {
    let mut engine = Engine::default();
    engine.enable_playback(true);
    for _ in 0..5 {
        audio(&mut engine, 0, true);
    }
    audio(&mut engine, 1, false);
    audio(&mut engine, 1000, false);
    assert!(engine.countdown.is_none());
}

#[test]
fn audio_keep_awake_releases_after_grace_without_rearming_on_silence() {
    let mut engine = Engine {
        while_audio: true,
        ..Engine::default()
    };
    audio(&mut engine, 0, true);
    audio(&mut engine, 1, false);
    audio(&mut engine, 61, false);
    audio(&mut engine, 62, false);
    assert!(!engine.should_hold_awake());
}
#[test]
fn audio_keep_awake_releases_after_a_chime_while_playback_is_unarmed() {
    let mut engine = Engine {
        while_audio: true,
        ..Engine::default()
    };
    engine.enable_playback(true);
    audio(&mut engine, 0, true);
    audio(&mut engine, 1, false);
    audio(&mut engine, 61, false);
    assert!(!engine.should_hold_awake());
    assert!(engine.countdown.is_none());
}
#[test]
fn audio_keep_awake_preserves_armed_playback_grace() {
    let mut engine = armed();
    engine.while_audio = true;
    audio(&mut engine, 3, false);
    audio(&mut engine, 63, false);
    assert_eq!(
        engine.countdown.as_ref().map(|c| c.source),
        Some(Source::Playback)
    );
}
#[test]
fn resumed_audio_cancels_at_deadline() {
    let mut e = armed();
    audio(&mut e, 3, false);
    audio(&mut e, 63, false);
    assert_eq!(audio(&mut e, 363, true), None);
    assert!(e.countdown.is_none());
    // Resumed playback is not a user decision to turn the rule off.
    assert!(e.playback_enabled);
}

#[test]
fn playback_action_rechecks_idle_at_execution_after_snoozing() {
    let mut engine = armed();
    audio(&mut engine, 3, false);
    audio(&mut engine, 63, false);
    engine.snooze(900).unwrap();
    let action = engine.tick(1263, Some(false), Some(5), false, &Settings::default());
    assert_eq!(action, None);
}
#[test]
fn input_cancels_at_deadline() {
    let mut e = armed();
    audio(&mut e, 3, false);
    audio(&mut e, 63, false);
    assert_eq!(
        e.tick(363, Some(false), Some(0), true, &Settings::default()),
        None
    );
    assert!(e.countdown.is_none());
    // Moving the mouse during the warning keeps the rule armed for the next playback.
    assert!(e.playback_enabled);
}
#[test]
fn idle_is_required_and_errors_fail_closed() {
    let mut e = armed();
    e.tick(3, Some(false), Some(0), false, &Settings::default());
    e.tick(63, Some(false), Some(20), false, &Settings::default());
    assert!(e.countdown.is_none());
    audio(&mut e, 303, false);
    e.tick(603, None, None, false, &Settings::default());
    assert!(e.countdown.is_none());
}

#[test]
fn clicking_pause_waits_for_idle_then_starts_playback_countdown() {
    let mut engine = armed();
    let settings = Settings {
        idle_seconds: 300,
        ..Settings::default()
    };
    engine.tick(3, Some(false), Some(0), true, &settings);
    engine.tick(63, Some(false), Some(60), false, &settings);
    assert!(engine.countdown.is_none());
    engine.tick(303, Some(false), Some(300), false, &settings);
    assert_eq!(
        engine.countdown.as_ref().map(|c| c.source),
        Some(Source::Playback)
    );
}

#[test]
fn input_during_silence_wait_defers_without_requiring_new_playback() {
    let mut engine = armed();
    let settings = Settings {
        idle_seconds: 300,
        ..Settings::default()
    };
    engine.tick(3, Some(false), Some(0), false, &settings);
    engine.tick(120, Some(false), Some(0), true, &settings);
    engine.tick(419, Some(false), Some(299), false, &settings);
    assert!(engine.countdown.is_none());
    engine.tick(420, Some(false), Some(300), false, &settings);
    assert_eq!(
        engine.countdown.as_ref().map(|c| c.source),
        Some(Source::Playback)
    );
}
#[test]
fn timer_uses_common_countdown_and_snooze() {
    let mut e = Engine::default();
    e.schedule(60, PowerAction::Lock);
    audio(&mut e, 60, false);
    assert_eq!(e.countdown.as_ref().unwrap().source, Source::Timer);
    e.snooze(900).unwrap();
    assert_eq!(audio(&mut e, 360, false), None);
    assert_eq!(audio(&mut e, 1260, false), Some(PowerAction::Lock));
}
#[test]
fn cancel_requires_fresh_playback() {
    let mut e = armed();
    audio(&mut e, 3, false);
    audio(&mut e, 63, false);
    e.cancel_countdown();
    audio(&mut e, 10000, false);
    assert!(e.countdown.is_none());
}
#[test]
fn resume_discards_sessions() {
    let mut e = armed();
    e.keep_awake(Some(30));
    e.schedule(5, PowerAction::Shutdown);
    e.reset_transient("Resumed");
    assert_eq!(audio(&mut e, 10000, false), None);
    assert!(!e.should_hold_awake());
}
#[test]
fn awake_expiry_and_extension() {
    let mut e = Engine::default();
    e.keep_awake(Some(60));
    e.extend_awake(900).unwrap();
    audio(&mut e, 60, false);
    assert!(e.awake);
    audio(&mut e, 960, false);
    assert!(!e.awake);
}
#[test]
fn manual_awake_blocks_playback_action() {
    let mut e = armed();
    e.keep_awake(None);
    for t in 4..7 {
        audio(&mut e, t, true);
    }
    audio(&mut e, 7, false);
    audio(&mut e, 1000, false);
    assert!(e.countdown.is_none());
}
#[test]
fn explicit_timer_wins_over_playback() {
    let mut e = armed();
    e.schedule(5, PowerAction::Lock);
    audio(&mut e, 3, false);
    audio(&mut e, 7, false);
    assert_eq!(e.countdown.as_ref().unwrap().action, PowerAction::Lock);
}
