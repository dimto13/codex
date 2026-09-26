# agent-exec — how a shell-less worker changes code

The autonomous worker sessions described in [`control-plane.md`](control-plane.md)
reach this repository only through the GitHub API. `agent-exec` gives them a
runner: it applies an edit, formats it, runs clippy and the scoped tests,
pushes the result and reports the real toolchain output back.

## Triggering

Post a comment on the issue labelled `agent:exec`. The comment must start with
`/agent-exec` on its own line and must come from the repository owner.

The workflow always runs from the default branch, so the branch under work can
never redefine it. The payload is declarative and is never executed: the only
code the runner runs is the repository's own toolchain.

## Payload

Exactly one `yaml` settings block, then exactly one payload block.

````markdown
/agent-exec
```yaml
branch: feature/queue-existing-session-cli
base: main
message: "feat(#28): add CLI queue and status adapter"
checks: [fmt, clippy, test]
packages: [codex-cli, codex-app-server-daemon]
push: true
```
```json
[
  {
    "op": "create",
    "path": "codex-rs/cli/src/thread_cmd.rs",
    "content": "use clap::Args;\n\n// ...\n"
  },
  {
    "op": "replace",
    "path": "codex-rs/cli/src/main.rs",
    "find": "mod remote_control_cmd;",
    "replace": "mod remote_control_cmd;\nmod thread_cmd;"
  }
]
```
````

### Settings

| Key | Default | Meaning |
| --- | --- | --- |
| `branch` | required | Work branch. `main` is rejected. |
| `base` | `main` | Branch to create `branch` from when it does not exist. |
| `message` | required unless `push: false` | Commit message. |
| `checks` | `[fmt, clippy, test]` | Any of `fmt`, `clippy`, `test`, `bench-smoke`, `bazel-lock`. |
| `packages` | — | Cargo packages for `clippy`/`test`; required when either is requested. |
| `push` | `true` | `false` runs the checks and throws the result away — use it as a dry run. |

Add `bazel-lock` whenever the payload touches `Cargo.toml` or `Cargo.lock`;
`AGENTS.md` requires `MODULE.bazel.lock` to move in the same change.

### Payload block

**`json`** — a list of edit operations. Prefer this: it carries no line numbers
to get wrong, and each operation states the source it expects.

| Op | Fields | Behaviour |
| --- | --- | --- |
| `replace` | `path`, `find`, `replace`, `count` (default `1`) | Fails unless `find` occurs exactly `count` times. This is also how you insert: match an anchor and put it back alongside the new text. |
| `create` | `path`, `content` | Fails if the file already exists. Creates parent directories. |
| `delete` | `path` | Fails if the file does not exist. |

Validation runs over the whole list before anything is written, so a stale
anchor in the last operation leaves the worktree untouched rather than half
edited. There is deliberately no whole-file overwrite of an existing file:
rewriting a file you only partially read is how source gets lost.

**`diff`** — a unified diff, applied with `git apply --3way`. Use it for large
mechanical changes where a diff is genuinely easier to produce.

## Rules

- **Read the anchor first.** An anchor quoted from memory is how a payload
  silently targets the wrong line. Fetch the file, copy the exact bytes.
- The workflow refuses edits to `codex-rs/deny.toml` and to the blocking
  workflows. Those are owner gates.
- A payload that leaves the worktree unchanged fails.
- On failure, nothing is pushed. Read the linked job log, fix the root cause,
  post a corrected payload. Never repost an identical failing payload.

## Freshness of pull-request checks

When the repository secret `AREN_AGENT_PAT` exists (a fine-grained PAT with
`contents: write` on this repository), `agent-exec` pushes with it and the
resulting pull-request CI re-runs on the new head.

Without that secret the push still lands, but GitHub suppresses workflow runs
for pushes made with `GITHUB_TOKEN`. The pull request then shows checks from an
older head. Those are stale and must not be used to satisfy the merge lane —
open the pull request only after the final `agent-exec` push, so the
`pull_request` open event produces CI on the real head.
