use super::*;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

fn thread_id(value: &str) -> ThreadId {
    ThreadId::from_string(value).expect("valid thread id")
}

#[test]
fn session_lookup_publishes_new_session_immediately() {
    let home = TempDir::new().expect("temp home");
    let id = thread_id("00000000-0000-0000-0000-000000000501");

    let path = publish_session_lookup_for_process(home.path(), "pid-101-test", id)
        .expect("publish lookup");
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).expect("read lookup")).expect("valid json");

    assert_eq!(value["thread_id"], id.to_string());
    assert_eq!(value["process_key"], "pid-101-test");
}

#[test]
fn session_lookup_resume_keeps_same_session_id() {
    let home = TempDir::new().expect("temp home");
    let id = thread_id("00000000-0000-0000-0000-000000000502");

    let first = publish_session_lookup_for_process(home.path(), "pid-102-test", id)
        .expect("publish new session");
    let resumed = publish_session_lookup_for_process(home.path(), "pid-102-test", id)
        .expect("publish resumed session");

    assert_eq!(first, resumed);
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(resumed).expect("read lookup")).expect("valid json");
    assert_eq!(value["thread_id"], id.to_string());
}

#[test]
fn session_lookup_disambiguates_parallel_processes() {
    let home = TempDir::new().expect("temp home");
    let first_id = thread_id("00000000-0000-0000-0000-000000000503");
    let second_id = thread_id("00000000-0000-0000-0000-000000000504");

    let first = publish_session_lookup_for_process(home.path(), "pid-201-first", first_id)
        .expect("publish first process");
    let second = publish_session_lookup_for_process(home.path(), "pid-202-second", second_id)
        .expect("publish second process");

    assert_ne!(first, second);
    let first_value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(first).expect("read first lookup"))
            .expect("valid first json");
    let second_value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(second).expect("read second lookup"))
            .expect("valid second json");
    assert_eq!(first_value["thread_id"], first_id.to_string());
    assert_eq!(second_value["thread_id"], second_id.to_string());
}
