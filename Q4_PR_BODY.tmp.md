Wires the existing `scheduler_cmd` module into the `codex` binary. This PR changes only `codex-rs/cli/src/main.rs` (+7 lines, 1 file). `scheduler_cmd.rs`, the test files, manifests and workflows are unchanged.

## SHAs

- **Base** `feature/session-scheduler`: `a746f864da482d7e4d2e702e55fcb314918bd5de` (matches the expected head)
- **PR head** `bugfix/q4-wire-scheduler-cli`: `2e5bf57ea982f210d03f7941044eeacf99d3c79f`

## Facts from `codex-rs/cli/src/scheduler_cmd.rs` (read only)

- **(a) Public clap type:** `SchedulerCommand`. Visibility is `pub(crate)`.
- **(b) Derive:** `#[derive(Debug, Parser)] pub(crate) struct SchedulerCommand { #[command(subcommand)] command: SchedulerSubcommand }`. It is a **struct deriving `clap::Parser`**, so the variant gets **no attribute**.
- **(c) Signature:** `pub(crate) async fn run(command: SchedulerCommand) -> anyhow::Result<()>`. It is **async**, takes one parameter, and finds the Codex home itself with `codex_core::config::find_codex_home()`.

## What changed (commit `2e5bf57`: `BUGFIX(Q4): wire scheduler_cmd into codex CLI`)

1. `mod scheduler_cmd;` placed alphabetically between `mod sandbox_setup;` and `mod state_db_recovery;`.
2. The variant at the end of `enum Subcommand`, with no attribute:
   ```rust
   /// Schedule one-shot turns for existing sessions.
   Scheduler(scheduler_cmd::SchedulerCommand),
   ```
3. An arm at the end of `match subcommand` in `cli_main`:
   ```rust
   Some(Subcommand::Scheduler(scheduler_cli)) => {
       scheduler_cmd::run(scheduler_cli).await?;
   }
   ```
4. A second exhaustive `match` on `Subcommand` in `main.rs`: `unsupported_subcommand_name_for_strict_config` has no wildcard arm, so without this it would fail with E0004. I added the arm next to `Queue`:
   ```rust
   Some(Subcommand::Scheduler(_)) => Some("scheduler"),
   ```
   `profile_v2_for_subcommand` has a `_ =>` arm and needed no change.

## Diagnostics

The runner has **no Rust toolchain**. `which cargo` exits with code 1 and prints nothing, and `cargo`, `rustc`, `just` and `cargo-shear` are not on `PATH`. Direct `cargo ...` calls also need interactive approval, which this unattended run cannot get. None of the commands below produced any output.

### `cargo check -p codex-cli --tests` @ `2e5bf57ea982f210d03f7941044eeacf99d3c79f`
not run: no Rust toolchain on the runner (`which cargo` → exit 1)

### `cargo test -p codex-cli scheduler` @ `2e5bf57ea982f210d03f7941044eeacf99d3c79f`
not run: no Rust toolchain on the runner (`which cargo` → exit 1)

### `cargo test -p codex-cli scheduler` (the package that contains `scheduler_task_tests.rs`, which is `codex-cli`) @ `2e5bf57ea982f210d03f7941044eeacf99d3c79f`
not run: no Rust toolchain on the runner (`which cargo` → exit 1)

### `cargo clippy -p codex-cli --tests` @ `2e5bf57ea982f210d03f7941044eeacf99d3c79f`
not run: no Rust toolchain on the runner (`which cargo` → exit 1)

### `Check Rust formatting`: `cargo fmt -- --config imports_granularity=Item --check` @ `2e5bf57ea982f210d03f7941044eeacf99d3c79f`
(command copied from `.github/workflows/blocking-ci.yml`, workflow `aren-ci`, job `format`, `working-directory: codex-rs`)
not run: no Rust toolchain or rustfmt on the runner (`which cargo` → exit 1)

### `Check unused dependencies`: `cargo shear --deny-warnings` @ `2e5bf57ea982f210d03f7941044eeacf99d3c79f`
(command copied from `.github/workflows/blocking-ci.yml`, workflow `aren-ci`, job `cargo-shear`, `working-directory: codex-rs`)
not run: no Rust toolchain or cargo-shear on the runner (`which cargo` → exit 1)

### Formatter commit (step 3)
Skipped, because the formatter could not run on the runner.

## Leftover formatter files

The formatter did not run, so there is **no reported list**. The following is a static prediction, **not formatter output**: it lists lines longer than the rustfmt default `max_width = 100` (`codex-rs/rustfmt.toml` sets only `edition` and `imports_granularity`).
- `codex-rs/cli/src/scheduler_cmd.rs`: 2 lines over 100 characters (75, 141)
- `codex-rs/cli/src/scheduler_cmd_tests.rs`: 16 lines over 100 characters (14, 28, 31, 44, 45, 47, 48, 53, 55, 61, 63, 64, 72, 73, 87)
- `scheduler_task.rs`, `scheduler_task_tests.rs`: no lines over 100 characters

## Expected compile errors for CHAT1 (static reading, **not compiler output**)

These come from reading the code by hand and must be confirmed by a real `cargo check`:

1. **`crate::scheduler_task` does not resolve in the binary crate.** `scheduler_cmd.rs:1-3` and `scheduler_cmd_tests.rs:7-8` import `crate::scheduler_task::{DueTime, ScheduledTask, SchedulerStore, TaskState}`. `mod scheduler_task;` exists only in `lib.rs` (the `codex_cli` library crate, private). Once `scheduler_cmd` is a module of `main.rs`, `crate::` means the `codex` binary, so expect `E0432 unresolved import`.
2. **`crate::app_server_control_socket_path()` does not exist in `main.rs`.** `scheduler_cmd.rs:66` calls it with no arguments. The only definition I found is `codex_app_server::app_server_control_socket_path(codex_home: &Path)` (defined in `app-server-transport/src/transport/mod.rs:56`), which takes a path. Expect `E0425`. The same call is in `queue_cmd.rs:20`, which is already wired into `main.rs`, so that error probably exists already on `feature/session-scheduler` and `main`.
3. Possible behavior follow-up, not a compile error: the `Queue` arm calls `reject_remote_mode_for_subcommand(..., "queue")` before `run`. As specified, the `Scheduler` arm does not, so `codex --remote … scheduler …` silently ignores `--remote`.

Refs #25 · CONTROL #68

🤖 Generated with [Claude Code](https://claude.com/claude-code)
