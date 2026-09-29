use super::QueueCommand;
use clap::Parser;

#[test]
fn parses_queue_message() {
    let cmd = QueueCommand::try_parse_from(["queue", "T1", "hello"]).expect("parse");
    assert_eq!(cmd.thread_id, "T1");
    assert_eq!(cmd.message.as_deref(), Some("hello"));
    assert!(!cmd.status);
}

#[test]
fn parses_status() {
    let cmd = QueueCommand::try_parse_from(["queue", "--status", "T1"]).expect("parse");
    assert!(cmd.status);
    assert_eq!(cmd.message, None);
}

#[test]
fn rejects_missing_message() {
    assert!(QueueCommand::try_parse_from(["queue", "T1"]).is_err());
}
