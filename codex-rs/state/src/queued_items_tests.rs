use super::*;
use pretty_assertions::assert_eq;

fn temp_home() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("codex-queue-test-{}", Uuid::now_v7()))
}

#[tokio::test]
async fn queue_is_ordered_and_thread_scoped() {
    let home = temp_home();
    let runtime = StateRuntime::init(home.clone(), "test-provider".to_string())
        .await
        .expect("initialize state runtime");
    let first_thread = ThreadId::new();
    let second_thread = ThreadId::new();

    let first = runtime
        .enqueue_user_submission(first_thread, r#"{"text":"one"}"#)
        .await
        .expect("enqueue first submission");
    let second = runtime
        .enqueue_user_submission(first_thread, r#"{"text":"two"}"#)
        .await
        .expect("enqueue second submission");
    runtime
        .enqueue_user_submission(second_thread, r#"{"text":"other"}"#)
        .await
        .expect("enqueue other thread submission");

    assert_eq!(
        runtime
            .queued_user_submissions(first_thread)
            .await
            .expect("list queue"),
        vec![first.clone(), second.clone()]
    );
    assert!(
        runtime
            .delete_queued_user_submission(first_thread, &first.id)
            .await
            .expect("delete queued submission")
    );
    assert_eq!(
        runtime
            .queued_user_submissions(first_thread)
            .await
            .expect("list queue after delete"),
        vec![second]
    );

    runtime.close().await;
    let _ = tokio::fs::remove_dir_all(home).await;
}
