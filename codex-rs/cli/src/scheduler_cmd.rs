use crate::scheduler_task::DueTime;
use crate::scheduler_task::ScheduledTask;
use crate::scheduler_task::SchedulerStore;
use anyhow::Context;
use clap::Args;
use clap::Parser;
use clap::Subcommand;
use codex_app_server_protocol::ThreadQueueParams;
use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
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
    #[arg(
        long,
        conflicts_with = "at_utc_ms",
        required_unless_present = "at_utc_ms"
    )]
    in_seconds: Option<u64>,
    /// Absolute UTC Unix timestamp in milliseconds.
    #[arg(
        long,
        conflicts_with = "in_seconds",
        required_unless_present = "in_seconds"
    )]
    at_utc_ms: Option<u64>,
}

#[derive(Debug, Args)]
struct TaskIdArgs {
    task_id: String,
}

#[allow(clippy::print_stdout)]
pub(crate) async fn run(command: SchedulerCommand) -> anyhow::Result<()> {
    let codex_home = codex_core::config::find_codex_home()?;
    match command.command {
        SchedulerSubcommand::Create(args) => println!("{}", create(&codex_home, args)?),
        SchedulerSubcommand::List => println!("{}", list(&codex_home)?),
        SchedulerSubcommand::Status(args) => println!("{}", status(&codex_home, &args.task_id)?),
        SchedulerSubcommand::Cancel(args) => println!("{}", cancel(&codex_home, &args.task_id)?),
        SchedulerSubcommand::DispatchDue => {
            let socket = codex_app_server::app_server_control_socket_path(&codex_home)?;
            for result in dispatch_due(&codex_home, |task| {
                let socket = socket.clone();
                async move {
                    let params = ThreadQueueParams {
                        thread_id: task.session_id,
                        message: task.prompt,
                        client_user_message_id: Default::default(),
                    };
                    let response =
                        codex_app_server_daemon::cli_queue_thread(&socket, params).await?;
                    Ok(serde_json::to_string(&response)?)
                }
            })
            .await?
            {
                println!("{result}");
            }
        }
    }
    Ok(())
}

fn create(codex_home: &Path, args: CreateArgs) -> anyhow::Result<String> {
    let mut store = SchedulerStore::load(codex_home)?;
    let due = match (args.in_seconds, args.at_utc_ms) {
        (Some(seconds), None) => DueTime::Relative(Duration::from_secs(seconds)),
        (None, Some(timestamp)) => DueTime::AbsoluteUtcMs(timestamp),
        _ => anyhow::bail!("provide exactly one of --in-seconds or --at-utc-ms"),
    };
    Ok(store.create(args.session_id, due, args.message)?.id)
}

fn list(codex_home: &Path) -> anyhow::Result<String> {
    let store = SchedulerStore::load(codex_home)?;
    Ok(serde_json::to_string_pretty(store.list())?)
}

fn status(codex_home: &Path, task_id: &str) -> anyhow::Result<String> {
    let store = SchedulerStore::load(codex_home)?;
    Ok(serde_json::to_string_pretty(task_or_error(
        &store, task_id,
    )?)?)
}

fn cancel(codex_home: &Path, task_id: &str) -> anyhow::Result<String> {
    let mut store = SchedulerStore::load(codex_home)?;
    let task = store
        .cancel(task_id)?
        .with_context(|| format!("unknown task id: {task_id}"))?;
    Ok(serde_json::to_string_pretty(&task)?)
}

async fn dispatch_due<F, Fut>(codex_home: &Path, mut dispatch: F) -> anyhow::Result<Vec<String>>
where
    F: FnMut(ScheduledTask) -> Fut,
    Fut: Future<Output = anyhow::Result<String>>,
{
    let mut store = SchedulerStore::load(codex_home)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    let results = Arc::new(Mutex::new(Vec::new()));
    store
        .dispatch_due(now, |task| {
            let future = dispatch(task);
            let results = Arc::clone(&results);
            async move {
                let result = future.await?;
                results.lock().expect("scheduler results lock").push(result);
                Ok(())
            }
        })
        .await?;
    Ok(Arc::try_unwrap(results)
        .expect("scheduler results still referenced")
        .into_inner()
        .expect("scheduler results lock"))
}

fn task_or_error<'a>(
    store: &'a SchedulerStore,
    task_id: &str,
) -> anyhow::Result<&'a ScheduledTask> {
    store
        .get(task_id)
        .with_context(|| format!("unknown task id: {task_id}"))
}

#[cfg(test)]
#[path = "scheduler_cmd_tests.rs"]
mod tests;
