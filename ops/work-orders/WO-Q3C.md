# WO-Q3C — CLI adapter for thread queue and status

- **Issue:** #28 (Aren upstream capability A3)
- **Branch:** `feature/queue-existing-session-cli`
- **Status source:** CONTROL state block, item `Q3c`

## Goal

Finish #28 from the outside: let an operator queue one turn into an existing
session and read that session's runtime state from the command line, in human
and JSON form, against the running daemon.

Q3a landed the durable queue storage and Q3b landed the protocol and runtime.
Both are on `main`. Q3c adds only the last hop — daemon client calls and a CLI
surface. Nothing in core, nothing in the protocol.

## Substrate that already exists

In `codex-rs/app-server-protocol/src/protocol/v2/thread.rs`:

```rust
pub struct ThreadQueueParams { pub thread_id: String, pub message: String, pub client_user_message_id: Option<String> }
pub struct ThreadQueueResponse { pub thread_id: String, pub queue_id: String }
pub enum ThreadRuntimeState { Idle, Running, Queued, NotFound }   // SCREAMING_SNAKE_CASE on the wire
pub struct ThreadStatusGetParams { pub thread_id: String }
pub struct ThreadStatusGetResponse { pub thread_id: String, pub state: ThreadRuntimeState, pub active_turn: bool, pub queued: u32 }
```

Methods, from `protocol/common.rs`: `thread/queue` and `thread/status/get`.

In `codex-rs/app-server-daemon/src/client.rs`: `connect`, `initialize`,
`send_message`, `read_message`, `CONTROL_SOCKET_RESPONSE_TIMEOUT`. All are
`pub(crate)` and stay that way.

`codex-rs/app-server-daemon/src/remote_control_client.rs` is the pattern to
copy for a typed request over the control socket.

## Files

| Path | Change |
| --- | --- |
| `codex-rs/app-server-daemon/src/thread_queue_client.rs` | **new.** Typed `thread/queue` + `thread/status/get` calls over the control socket. |
| `codex-rs/app-server-daemon/src/thread_queue_client_tests.rs` | **new.** Unit tests, wired with `#[cfg(test)] #[path = "thread_queue_client_tests.rs"] mod tests;`. |
| `codex-rs/app-server-daemon/src/lib.rs` | add `mod thread_queue_client;` and the two `pub async fn` entry points below. Nothing else — the file is already over 850 lines. |
| `codex-rs/cli/src/thread_cmd.rs` | **new.** `codex thread queue` and `codex thread status`. Do **not** grow `remote_control_cmd.rs` (771 lines). |
| `codex-rs/cli/src/thread_cmd_tests.rs` | **new.** Output-rendering tests. |
| `codex-rs/cli/src/main.rs` | `mod thread_cmd;`, one `Thread(ThreadCommand)` variant on `enum Subcommand`, one dispatch arm. |

No `Cargo.toml` change is expected. If one turns out to be necessary, add the
`bazel-lock` check to the `agent-exec` payload in the same run.

## Design

In `lib.rs`, exactly these two public entry points:

```rust
pub async fn queue_thread_message(
    thread_id: String,
    message: String,
) -> Result<ThreadQueueResponse>;

pub async fn get_thread_status(thread_id: String) -> Result<ThreadStatusGetResponse>;
```

Both resolve the canonical control socket the same way the existing remote
control entry points do, then delegate to `thread_queue_client`. Re-export
`ThreadQueueResponse`, `ThreadStatusGetResponse` and `ThreadRuntimeState` from
`lib.rs` so the CLI does not depend on `codex-app-server-protocol` directly.

CLI surface:

```
codex thread queue <THREAD_ID> <MESSAGE> [--json]
codex thread status <THREAD_ID> [--json]
```

`--json` prints exactly the response struct's camelCase serialization, one
object, no wrapper. Human output is one line each:

```
Queued turn for <thread-id> (queue id <queue-id>).
<thread-id>: RUNNING, active turn, 2 queued
```

`client_user_message_id` is always `None` in Q3c; the CLI has no stable client
identity to supply and inventing one would create a second source of truth.

Exit codes, reusing `codex-rs/cli/src/exit_status.rs`: `0` on success; a
distinct non-zero code when the daemon reports `NOT_FOUND`, so a script can
branch without parsing text. `NOT_FOUND` is a normal answer, not an error —
with `--json` it still prints the response object before exiting non-zero.

## Tests

In `thread_queue_client_tests.rs`:

- `queue_request_serializes_expected_params` — the outgoing JSON-RPC frame for
  `thread/queue` carries method, `threadId`, `message`, and `clientUserMessageId: null`.
- `status_response_deserializes_all_runtime_states` — each of
  `IDLE`/`RUNNING`/`QUEUED`/`NOT_FOUND` round-trips into `ThreadRuntimeState`.
- `rpc_error_is_surfaced_not_swallowed` — a JSON-RPC error response becomes an
  `Err`, not a defaulted `NotFound`.

In `thread_cmd_tests.rs`:

- `json_output_matches_response_shape` — compare the whole serialized object
  with `pretty_assertions::assert_eq`, not field by field.
- `human_output_renders_each_runtime_state` — one assertion per state.
- `not_found_uses_dedicated_exit_code`.

## Acceptance

```
just test -p codex-app-server-daemon -p codex-cli
just fmt
cargo clippy --tests -p codex-app-server-daemon -p codex-cli -- -D warnings
```

Then: `aren-ci` fully green on the exact pull-request head, including the
`Scoped package tests` job, which will pick up both packages from the diff.

## Out of scope

- Any retry or nudge policy. #28 specifies `send → status → optional nudge` as
  an **external** loop; the CLI must not automate it.
- Any new session or runtime state root.
- `WAITING_FOR_INPUT`. The protocol enum has four states; adding a fifth is a
  protocol change and belongs to its own item.
- Touching core, the app-server, or the Q3a/Q3b code that already merged.
- Q4/#25 scheduler work of any kind.
