use crate::scheduler_task::DueTime;
use crate::scheduler_task::ScheduledTask;
use crate::scheduler_task::SchedulerStore;
use anyhow::Context;
use clap::Args;
use clap::Parser;
use clap::Subcommand;
use codex_app_server_protocol::ThreadQueueParams;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

/// Manage persistent one-shot tasks for existing sessions.
#[derive(Debug, Parser)]
pub(crate) struct SchedulerCommand {
    #[command(subcommand)]
    command: SchedulerSubcommand,
}

#[derive(Debug, Subcommand)]
enum SchedulerSubcommand {
    /// Schedule a one-shot message for an existing session.
    Create(CreateArgs),
    /// List scheduled tasks.
    List,
    /// Show one scheduled task.
    Status(TaskIdArgs),
    /// Cancel a pending task.
    Cancel(TaskIdArgs),
    /// Dispatch all tasks that are currently due exactly once.
    DispatchDue,
}

#[derive(Debug, Args)]
struct CreateArgs {
    /// Canonical session/thread id.
    session_id: String,
    /// Message to send when the task becomes due.
    message: String,
    /// Delay from now in seconds.
    #[arg(long, conflicts_with = "at_utc_ms", required_unless_present = "at_utc_ms")]
    in_seconds: Option<u64>,
    /// Absolute UTC Unix timestamp in milliseconds.
    #[arg(long, conflicts_with = "in_seconds", required_unless_present = "in_seconds")]
    at_utc_ms: Option<u64>,
}

#[derive(Debug, Args)]
struct TaskIdArgs {
    task_id: String,
}

#[allow(clippy::print_stdout)]
pub(crate) async fn run(command: SchedulerCommand) -> anyhow::Result<()> {
    let codex_home = codex_core::config::find_codex_home()?;
    let mut store = SchedulerStore::load(&codex_home)?;
    match command.command {
        SchedulerSubcommand::Create(args) => {
            let due = match (args.in_seconds, args.at_utc_ms) {
                (Some(seconds), None) => DueTime::Relative(Duration::from_secs(seconds)),
                (None, Some(timestamp)) => DueTime::AbsoluteUtcMs(timestamp),
                _ => anyhow::bail!("provide exactly one of --in-seconds or --at-utc-ms"),
            };
            let task = store.create(args.session_id, due, args.message)?;
            println!("{}", task.id);
        }
        SchedulerSubcommand::List => {
            println!("{}", serde_json::to_string_pretty(store.list())?);
        }
        SchedulerSubcommand::Status(args) => {
            let task = task_or_error(&store, &args.task_id)?;
            println!("{}", serde_json::to_string_pretty(task)?);
        }
        SchedulerSubcommand::Cancel(args) => {
            let task = store
                .cancel(&args.task_id)?
                .with_context(|| format!("unknown task id: {}", args.task_id))?;
            println!("{}", serde_json::to_string_pretty(&task)?);
        }
        SchedulerSubcommand::DispatchDue => {
            let socket = crate::app_server_control_socket_path()?;
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
            store
                .dispatch_due(now, |task| {
                    let socket = socket.clone();
                    async move {
                        let params = ThreadQueueParams {
                            thread_id: task.session_id,
                            message: task.prompt,
                            client_user_message_id: Default::default(),
                        };
                        let response = codex_app_server_daemon::cli_queue_thread(&socket, params).await?;
                        println!("{}", serde_json::to_string(&response)?);
                        Ok(())
                    }
                })
                .await?;
        }
    }
    Ok(())
}

fn task_or_error<'a>(store: &'a SchedulerStore, task_id: &str) -> anyhow::Result<&'a ScheduledTask> {
    store
        .get(task_id)
        .with_context(|| format!("unknown task id: {task_id}"))
}
