use super::*;
use pretty_assertions::assert_eq;

fn policy(max_observations: u32, nudge_enabled: bool) -> WatchdogPolicy {
    WatchdogPolicy {
        max_observations,
        nudge_enabled,
    }
}

#[test]
fn running_active_turn_succeeds_without_nudge() {
    let mut watchdog = ExecutionStartWatchdog::new("corr-1".into(), policy(2, true));

    assert_eq!(
        watchdog.observe(ThreadRuntimeState::Running, true),
        WatchdogAction::Succeed
    );
    assert_eq!(
        watchdog.outcome(StartConfirmation::Running),
        WatchdogOutcome {
            correlation_id: "corr-1".into(),
            status: StartConfirmation::Running,
            nudges: 0,
        }
    );
}

#[test]
fn nudge_is_emitted_at_most_once() {
    let mut watchdog = ExecutionStartWatchdog::new("corr-2".into(), policy(1, true));

    assert_eq!(
        watchdog.observe(ThreadRuntimeState::Idle, false),
        WatchdogAction::Nudge
    );
    assert_eq!(
        watchdog.observe(ThreadRuntimeState::Idle, false),
        WatchdogAction::Fail
    );
    assert_eq!(
        watchdog.outcome(StartConfirmation::StartNotConfirmed).nudges,
        1
    );
}

#[test]
fn disabled_nudge_fails_after_grace() {
    let mut watchdog = ExecutionStartWatchdog::new("corr-3".into(), policy(2, false));

    assert_eq!(
        watchdog.observe(ThreadRuntimeState::Idle, false),
        WatchdogAction::Observe
    );
    assert_eq!(
        watchdog.observe(ThreadRuntimeState::Idle, false),
        WatchdogAction::Fail
    );
}

#[test]
fn queued_does_not_consume_observation_budget() {
    let mut watchdog = ExecutionStartWatchdog::new("corr-4".into(), policy(2, true));

    for _ in 0..4 {
        assert_eq!(
            watchdog.observe(ThreadRuntimeState::Queued, false),
            WatchdogAction::Observe
        );
    }
    assert_eq!(
        watchdog.observe(ThreadRuntimeState::Idle, false),
        WatchdogAction::Observe
    );
    assert_eq!(
        watchdog.observe(ThreadRuntimeState::Idle, false),
        WatchdogAction::Nudge
    );
}

#[test]
fn late_start_after_nudge_succeeds_without_second_nudge() {
    let mut watchdog = ExecutionStartWatchdog::new("corr-5".into(), policy(1, true));

    assert_eq!(
        watchdog.observe(ThreadRuntimeState::Idle, false),
        WatchdogAction::Nudge
    );
    assert_eq!(
        watchdog.observe(ThreadRuntimeState::Running, true),
        WatchdogAction::Succeed
    );
    assert_eq!(watchdog.outcome(StartConfirmation::Running).nudges, 1);
}

#[test]
fn running_without_active_turn_is_not_confirmed() {
    let mut watchdog = ExecutionStartWatchdog::new("corr-6".into(), policy(1, false));

    assert_eq!(
        watchdog.observe(ThreadRuntimeState::Running, false),
        WatchdogAction::Fail
    );
}

#[test]
fn not_found_fails_immediately() {
    let mut watchdog = ExecutionStartWatchdog::new("corr-7".into(), policy(10, true));

    assert_eq!(
        watchdog.observe(ThreadRuntimeState::NotFound, false),
        WatchdogAction::Fail
    );
    assert_eq!(
        watchdog.outcome(StartConfirmation::NotFound),
        WatchdogOutcome {
            correlation_id: "corr-7".into(),
            status: StartConfirmation::NotFound,
            nudges: 0,
        }
    );
}
