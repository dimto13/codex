use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::*;

fn thread_id(value: &str) -> ThreadId {
    ThreadId::from_string(value).expect("valid thread id")
}

fn read_lookup(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).expect("read lookup")).expect("valid json")
}

#[test]
fn session_lookup_publishes_new_session_immediately() {
    let home = TempDir::new().expect("temp home");
    let id = thread_id("00000000-0000-0000-0000-000000000501");

    let path = publish_session_lookup_for_process(home.path(), 101, id).expect("publish lookup");
    let value = read_lookup(&path);

    assert_eq!(path, home.path().join("session-processes").join("101.json"),);
    assert_eq!(value["pid"].as_u64(), Some(101));
    assert_eq!(value["thread_id"], id.to_string());
}

#[test]
fn session_lookup_resume_keeps_same_session_id() {
    let home = TempDir::new().expect("temp home");
    let id = thread_id("00000000-0000-0000-0000-000000000502");

    let first =
        publish_session_lookup_for_process(home.path(), 102, id).expect("publish new session");
    let resumed =
        publish_session_lookup_for_process(home.path(), 102, id).expect("publish resumed session");

    assert_eq!(first, resumed);
    assert_eq!(read_lookup(&resumed)["thread_id"], id.to_string());
}

#[test]
fn session_lookup_disambiguates_parallel_processes() {
    let home = TempDir::new().expect("temp home");
    let first_id = thread_id("00000000-0000-0000-0000-000000000503");
    let second_id = thread_id("00000000-0000-0000-0000-000000000504");

    let first = publish_session_lookup_for_process(home.path(), 201, first_id)
        .expect("publish first process");
    let second = publish_session_lookup_for_process(home.path(), 202, second_id)
        .expect("publish second process");

    assert_ne!(first, second);
    assert_eq!(read_lookup(&first)["thread_id"], first_id.to_string());
    assert_eq!(read_lookup(&second)["thread_id"], second_id.to_string());
}

#[test]
fn session_id_message_snapshot() {
    let id = thread_id("00000000-0000-0000-0000-000000000505");

    insta::assert_snapshot!(session_id_message(id), @"Session ID: 00000000-0000-0000-0000-000000000505");
}
