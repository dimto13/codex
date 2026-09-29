use super::DueTime;
use super::ScheduledTask;
use super::SchedulerStore;
use super::TaskState;
use pretty_assertions::assert_eq;
use std::fs;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn normalizes_relative_and_absolute_due_times() {
    assert_eq!(DueTime::Relative(Duration::from_secs(5)).normalize(1_000), 6_000);
    assert_eq!(DueTime::AbsoluteUtcMs(9_000).normalize(1_000), 9_000);
}

#[test]
fn persistence_round_trip_and_cancel_are_stable() {
    let home = TempDir::new().expect("tempdir");
    let mut store = SchedulerStore::load(home.path()).expect("load");
    let created = store
        .create(
            "session-1".to_string(),
            DueTime::AbsoluteUtcMs(u64::MAX),
            "continue".to_string(),
        )
        .expect("create");

    let mut reloaded = SchedulerStore::load(home.path()).expect("reload");
    assert_eq!(reloaded.list(), &[created.clone()]);
    let cancelled = reloaded.cancel(&created.id).expect("cancel").expect("task");
    assert_eq!(cancelled.state, TaskState::Cancelled);
    let cancelled_again = reloaded
        .cancel(&created.id)
        .expect("cancel again")
        .expect("task");
    assert_eq!(cancelled_again, cancelled);
}

#[test]
fn restart_fails_running_and_keeps_pending_due_for_single_dispatch() {
    let home = TempDir::new().expect("tempdir");
    let path = home.path().join("scheduled_tasks.json");
    let tasks = vec![
        ScheduledTask {
            id: "running".to_string(),
            session_id: "session-1".to_string(),
            due_at_utc_ms: 1,
            prompt: "running".to_string(),
            state: TaskState::Running,
            created_at_utc_ms: 1,
            updated_at_utc_ms: 1,
            last_error: None,
        },
        ScheduledTask {
            id: "pending".to_string(),
            session_id: "session-1".to_string(),
            due_at_utc_ms: 1,
            prompt: "pending".to_string(),
            state: TaskState::Pending,
            created_at_utc_ms: 1,
            updated_at_utc_ms: 1,
            last_error: None,
        },
    ];
    fs::write(&path, serde_json::to_vec(&tasks).expect("serialize")).expect("write");

    let store = SchedulerStore::load(home.path()).expect("load");
    let running = store.get("running").expect("running");
    assert_eq!(running.state, TaskState::Failed);
    assert_eq!(running.last_error.as_deref(), Some("interrupted_by_restart"));
    assert_eq!(store.recoverable_due_task_ids(u64::MAX), vec!["pending"]);
}
