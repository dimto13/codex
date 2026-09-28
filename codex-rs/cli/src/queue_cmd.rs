use clap::Parser;
use codex_app_server_protocol::ThreadQueueParams;
use codex_app_server_protocol::ThreadStatusGetParams;

/// Queue a message into an existing session, or print its status.
#[derive(Debug, Parser)]
pub struct QueueCommand {
    /// Existing session/thread id.
    pub thread_id: String,
    /// Message to queue into the session.
    #[arg(required_unless_present = "status")]
    pub message: Option<String>,
    /// Print the thread status instead of queueing a message.
    #[arg(long)]
    pub status: bool,
}

#[allow(clippy::print_stdout)]
pub async fn run(cmd: QueueCommand) -> anyhow::Result<()> {
    let socket = crate::app_server_control_socket_path()?;
    if cmd.status {
        let params = ThreadStatusGetParams {
            thread_id: cmd.thread_id,
        };
        let response = codex_app_server_daemon::cli_thread_status(&socket, params).await?;
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        let params = ThreadQueueParams {
            thread_id: cmd.thread_id,
            message: cmd.message.unwrap_or_default(),
            client_user_message_id: Default::default(),
        };
        let response = codex_app_server_daemon::cli_queue_thread(&socket, params).await?;
        println!("{}", serde_json::to_string_pretty(&response)?);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
}
