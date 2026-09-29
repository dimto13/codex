use super::DueTime;
use super::ScheduledTask;
use super::SchedulerStore;
use super::TaskState;
use pretty_assertions::assert_eq;
use std::fs;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn normalizes_relative_and_absolute_due_times() {
    assert_eq!(
        DueTime::Relative(Duration::from_secs(5)).normalize(1_000),
        6_000
    );
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
    assert_eq!(
        running.last_error.as_deref(),
        Some("interrupted_by_restart")
    );
    assert_eq!(store.recoverable_due_task_ids(u64::MAX), vec!["pending"]);
}

#[tokio::test]
async fn dispatch_claims_before_call_and_never_dispatches_twice() {
    let home = TempDir::new().expect("tempdir");
    let mut store = SchedulerStore::load(home.path()).expect("load");
    let task = store
        .create(
            "session-1".to_string(),
            DueTime::AbsoluteUtcMs(1),
            "continue".to_string(),
        )
        .expect("create");
    let path = home.path().to_path_buf();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let calls_for_dispatch = Arc::clone(&calls);

    store
        .dispatch_due(u64::MAX, move |claimed| {
            let path = path.clone();
            let calls = Arc::clone(&calls_for_dispatch);
            async move {
                let persisted: Vec<ScheduledTask> = serde_json::from_slice(
                    &fs::read(path.join("scheduled_tasks.json")).expect("read persisted"),
                )
                .expect("parse persisted");
                assert_eq!(persisted[0].state, TaskState::Running);
                calls.lock().expect("calls").push(claimed.id);
                Ok(())
            }
        })
        .await
        .expect("dispatch");
    store
        .dispatch_due(u64::MAX, |_| async { Ok(()) })
        .await
        .expect("second dispatch");

    assert_eq!(*calls.lock().expect("calls"), vec![task.id.clone()]);
    assert_eq!(
        store.get(&task.id).expect("task").state,
        TaskState::Succeeded
    );
}

#[tokio::test]
async fn busy_target_is_accepted_once_after_persisted_claim() {
    let home = TempDir::new().expect("tempdir");
    let mut store = SchedulerStore::load(home.path()).expect("load");
    let task = store
        .create(
            "canonical-session-id".to_string(),
            DueTime::AbsoluteUtcMs(1),
            "continue while busy".to_string(),
        )
        .expect("create");
    let path = home.path().to_path_buf();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let calls_for_dispatch = Arc::clone(&calls);

    store
        .dispatch_due(u64::MAX, move |claimed| {
            let path = path.clone();
            let calls = Arc::clone(&calls_for_dispatch);
            async move {
                let persisted: Vec<ScheduledTask> = serde_json::from_slice(
                    &fs::read(path.join("scheduled_tasks.json")).expect("read persisted"),
                )
                .expect("parse persisted");
                assert_eq!(persisted[0].state, TaskState::Running);
                calls
                    .lock()
                    .expect("calls")
                    .push(claimed.session_id.clone());
                // Main's #28 queue path accepts a turn for a busy target by queueing it,
                // so the scheduler seam observes a successful dispatch.
                Ok(())
            }
        })
        .await
        .expect("busy target accepted");

    let calls_for_second_dispatch = Arc::clone(&calls);
    store
        .dispatch_due(u64::MAX, move |claimed| {
            let calls = Arc::clone(&calls_for_second_dispatch);
            async move {
                calls.lock().expect("calls").push(claimed.session_id);
                Ok(())
            }
        })
        .await
        .expect("second dispatch");

    assert_eq!(
        *calls.lock().expect("calls"),
        vec!["canonical-session-id".to_string()]
    );
    assert_eq!(
        store.get(&task.id).expect("task").state,
        TaskState::Succeeded
    );
}

#[tokio::test]
async fn dispatch_records_failure_and_skips_non_pending_tasks() {
    let home = TempDir::new().expect("tempdir");
    let mut store = SchedulerStore::load(home.path()).expect("load");
    let failed = store
        .create(
            "session-1".to_string(),
            DueTime::AbsoluteUtcMs(1),
            "continue".to_string(),
        )
        .expect("create");
    let cancelled = store
        .create(
            "session-2".to_string(),
            DueTime::AbsoluteUtcMs(1),
            "cancelled".to_string(),
        )
        .expect("create");
    store.cancel(&cancelled.id).expect("cancel");

    store
        .dispatch_due(u64::MAX, |_| async { anyhow::bail!("resume failed") })
        .await
        .expect("dispatch");

    let failed = store.get(&failed.id).expect("failed task");
    assert_eq!(failed.state, TaskState::Failed);
    assert_eq!(failed.last_error.as_deref(), Some("resume failed"));
    assert_eq!(
        store.get(&cancelled.id).expect("cancelled").state,
        TaskState::Cancelled
    );
}
