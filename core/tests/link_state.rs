use feedrv3_core::domain::link_state::{compute_delay_seconds, LinkPhase, LinkState, NextAction};

#[test]
fn delay_is_clamped_and_jittered() {
    let d = compute_delay_seconds(10, 3, 60, 0.1, 0.5); // centered jitter = 0
    assert!(d.total_seconds <= 60);
}

#[test]
fn decide_action_respects_next_action_at() {
    let mut s = LinkState::initial("f1".to_string(), 10, 60, 0.1, 1_000);
    s.phase = LinkPhase::NeedsGet;
    s.next_action_at_ms = 2_000;
    let a = LinkState::decide_next_action(&s, 1_500);
    match a {
        NextAction::SleepUntil { at_ms } => assert_eq!(at_ms, 2_000),
        _ => panic!("expected sleep"),
    }
}
