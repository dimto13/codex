use super::CreateArgs;
use super::cancel;
use super::create;
use super::dispatch_due;
use super::list;
use super::status;
use crate::scheduler_task::ScheduledTask;
use crate::scheduler_task::TaskState;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::Mutex;
use tempfile::TempDir;

fn create_args(session_id: &str, message: &str, in_seconds: Option<u64>, at_utc_ms: Option<u64>) -> CreateArgs {
    CreateArgs {
        session_id: session_id.to_string(),
        message: message.to_string(),
        in_seconds,
        at_utc_ms,
    }
}

#[test]
fn create_relative_and_absolute_persist_canonical_target_and_print_id() {
    let home = TempDir::new().expect("tempdir");
    let relative_id = create(home.path(), create_args("session-canonical", "later", Some(60), None))
        .expect("create relative");
    let absolute_id = create(home.path(), create_args("session-canonical", "at time", None, Some(42)))
        .expect("create absolute");

    let tasks: Vec<ScheduledTask> = serde_json::from_str(&list(home.path()).expect("list")).expect("json");
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].id, relative_id);
    assert_eq!(tasks[0].session_id, "session-canonical");
    assert!(tasks[0].due_at_utc_ms >= tasks[0].created_at_utc_ms + 60_000);
    assert_eq!(tasks[1].id, absolute_id);
    assert_eq!(tasks[1].session_id, "session-canonical");
    assert_eq!(tasks[1].due_at_utc_ms, 42);
}

#[test]
fn list_and_status_expose_tasks_and_unknown_status_errors() {
    let home = TempDir::new().expect("tempdir");
    let first_id = create(home.path(), create_args("session-a", "one", None, Some(100))).expect("create first");
    let second_id = create(home.path(), create_args("session-b", "two", None, Some(200))).expect("create second");

    let tasks: Vec<ScheduledTask> = serde_json::from_str(&list(home.path()).expect("list")).expect("json");
    assert_eq!(tasks.iter().map(|task| task.id.as_str()).collect::<Vec<_>>(), vec![first_id.as_str(), second_id.as_str()]);
    assert_eq!(tasks[0].session_id, "session-a");
    assert_eq!(tasks[0].due_at_utc_ms, 100);
    assert_eq!(tasks[0].state, TaskState::Pending);

    let task: ScheduledTask = serde_json::from_str(&status(home.path(), &second_id).expect("status")).expect("json");
    assert_eq!(task, tasks[1]);
    assert_eq!(status(home.path(), "missing").expect_err("unknown task").to_string(), "unknown task id: missing");
}

#[test]
fn cancel_is_idempotent() {
    let home = TempDir::new().expect("tempdir");
    let id = create(home.path(), create_args("session-a", "cancel me", None, Some(u64::MAX))).expect("create");

    let first: ScheduledTask = serde_json::from_str(&cancel(home.path(), &id).expect("first cancel")).expect("json");
    let second: ScheduledTask = serde_json::from_str(&cancel(home.path(), &id).expect("second cancel")).expect("json");
    assert_eq!(first, second);
    assert_eq!(second.state, TaskState::Cancelled);
}

#[tokio::test]
async fn dispatch_due_prints_one_result_per_task_and_does_not_dispatch_twice() {
    let home = TempDir::new().expect("tempdir");
    let first_id = create(home.path(), create_args("session-a", "one", None, Some(0))).expect("create first");
    let second_id = create(home.path(), create_args("session-b", "two", None, Some(0))).expect("create second");
    let calls = Arc::new(Mutex::new(Vec::new()));

    let first_calls = Arc::clone(&calls);
    let results = dispatch_due(home.path(), move |task| {
        let calls = Arc::clone(&first_calls);
        async move {
            calls.lock().expect("calls lock").push(task.id.clone());
            Ok(format!("accepted:{}", task.id))
        }
    })
    .await
    .expect("dispatch");

    assert_eq!(calls.lock().expect("calls lock").as_slice(), &[first_id.clone(), second_id.clone()]);
    assert_eq!(results, vec![format!("accepted:{first_id}"), format!("accepted:{second_id}")]);

    let second_calls = Arc::clone(&calls);
    let second_results = dispatch_due(home.path(), move |task| {
        let calls = Arc::clone(&second_calls);
        async move {
            calls.lock().expect("calls lock").push(task.id.clone());
            Ok(format!("accepted:{}", task.id))
        }
    })
    .await
    .expect("second dispatch");
    assert!(second_results.is_empty());
    assert_eq!(calls.lock().expect("calls lock").as_slice(), &[first_id, second_id]);
}
