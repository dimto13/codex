# Aren Control Plane v2

This repository is operated by autonomous chat sessions that are replaced at
any time. GitHub is the persistent operational single source of truth; a chat
holds nothing that matters. Everything here is additive to the root
`AGENTS.md`, which stays authoritative for code style, testing and
architecture.

`ops/` deliberately sits outside `docs/`: `AGENTS.md` reserves `docs/` for
product documentation, and keeping fork operations separate keeps upstream
syncs from fighting over these files.

## 0. The one fact every agent must internalize

**The worker has no shell.** It reaches this repository only through the GitHub
API — it can read files, write comments and move refs, but it cannot run
`cargo`, `just`, a formatter or a test. That is permanent and is not a blocker.

Code therefore changes through exactly one route:

> The worker posts an `/agent-exec` comment on the issue labelled `agent:exec`.
> `.github/workflows/agent-exec.yml` applies that declarative edit on a runner,
> formats it, runs clippy and the scoped tests, pushes the result and reports
> the real compiler and test output back as a comment.

See [`ops/agent-exec.md`](agent-exec.md) for the payload format.

Consequences, all binding:

- Never reconstruct an existing source file from partial reads and write it
  back whole. Edit through `agent-exec` with fail-closed pattern assertions.
- Never open a temporary workflow to get work done. `agent-exec` is permanent,
  reviewed and already has the toolchain.
- "I have no way to edit files" is **not** a blocker and must never be
  escalated again. If `agent-exec` itself is broken, that is a concrete,
  reportable defect: name the run URL and the failing step.

## 1. CONTROL discovery

Exactly one OPEN issue carries the label `control:active`. Zero or more than
one means `CONTROL_PLANE_BLOCKED`: no mutation, no guessing, no reusing a
remembered issue number. CONTROL issue numbers are never hardcoded anywhere.

### The state block is the only authority

The CONTROL body contains exactly one fenced `yaml` block introduced by the
marker `ARENSTATE v2`. That block is the operative state. Everything else —
the rest of the body, every comment, every handoff — is a log. When a comment
and the state block disagree, the state block wins; when the state block and
live GitHub disagree, live GitHub wins and the state block gets corrected.

```yaml
# ARENSTATE v2
updated_at: 2026-09-26T08:00:00+02:00
updated_by: PLAN
main: 9f7ea7f323c04fbbfe8ec0680e4fd6f2e15203c7
merge_lane: FREE          # FREE | HELD:<pr-number>
blockers: []              # see §5; an empty list means "work, do not ask"
queue:
  - id: Q3c
    issue: 28
    work_order: ops/work-orders/WO-Q3C.md
    owner: CHAT1
    status: READY
    branch: feature/queue-existing-session-cli
    pr: null
```

Reading the state block is the whole of state reconstruction. A worker that
finds itself parsing prose to decide what to do next has hit a defect in
CONTROL, not a decision it should make on its own — report it and stop.

### Picking the job

Deterministic, no judgement:

1. Any item with status `MERGED_PENDING_MAIN_CI` — finish verifying it.
2. Otherwise the **first** queue item with `owner: CHAT1` and status
   `IN_PROGRESS`.
3. Otherwise the **first** with `owner: CHAT1` and status `READY`.
4. Otherwise there is no authorized work. Say so and stop. Do not invent work,
   do not pull a later item forward, do not start something from the backlog.

## 2. Roles

**PLAN** is the watchdog. It reconciles the state block against live GitHub,
fixes stale entries, detects genuine deadlocks, prepares work orders for
upcoming items and rolls CONTROL. PLAN does not implement product features and
does not gate the worker's routine progress.

**CHAT1** is the implementation worker. It executes the selected queue item end
to end, including merging it.

One worker owns one branch. A worker never touches another worker's branch and
never rewrites history on a branch it did not create.

## 3. Nothing starts without a work order

A queue item becomes `READY` only when its `work_order` file exists on `main`
and names, concretely:

- the exact files to change,
- the exact symbols, signatures and wire shapes to add,
- the exact tests to add, by name,
- the acceptance check, as a command,
- what is explicitly out of scope.

This exists because re-deriving a design from an issue's prose every hour
produces a different design every hour, and nothing converges. If a work order
is missing or has gone stale against the code, that is a real blocker: report
it against PLAN and stop. Do not design it yourself.

Template and examples: [`ops/work-orders/`](work-orders/).

## 4. Lifecycle and merge lane

```
READY → IN_PROGRESS → PR_OPEN → CI_RUNNING → MERGE_READY
      → MERGED_PENDING_MAIN_CI → DONE
```

There is one global merge lane. While any item is `MERGED_PENDING_MAIN_CI`, no
second pull request may be merged.

**CHAT1 merges its own pull request** once the lane is satisfied on the exact
current head. This supersedes every older rule that required handing merge-ready
work back to PLAN; that round trip was a stall source and is gone. The gates
themselves are unchanged and all of them must hold on the head being merged:

1. every blocking check on `aren-ci` is `success`,
2. no `CHANGES_REQUESTED` and no unresolved review thread,
3. every review finding dispositioned,
4. mergeable, base current,
5. the work order's acceptance check satisfied,
6. no owner gate open.

Any change to the head invalidates all of it. After merging, the item is
`MERGED_PENDING_MAIN_CI`; it becomes `DONE` only once the resulting `main` run
of `aren-ci` is fully green. Never start the next item on a red `main`.

Note on freshness: `agent-exec` pushes with `AREN_AGENT_PAT` when that secret
exists, which re-triggers pull-request CI. Without it, a push lands but the
pull request keeps its previous checks — in that case the displayed checks are
stale evidence and must not be used to satisfy gate 1.

## 5. Blockers

A blocker needs all three of: a concrete cause, concrete evidence, and the
concrete condition that would clear it. "I do not know what to do next" and "I
cannot edit files" are not blockers.

Resolve these yourself; they are never escalation reasons:

| Situation | Action |
|---|---|
| formatting, lint or clippy failure | fix it and re-run `agent-exec` |
| snapshot or schema drift | regenerate with the repo's generator |
| `Cargo.toml`/`Cargo.lock` change | add the `bazel-lock` check to `agent-exec` |
| base moved | merge `main` into the branch, re-run the full lane |
| deterministic CI failure | read the failing job's log, fix the root cause |
| review finding in scope | fix it, refresh exact-head evidence |
| change too large for `AGENTS.md` | split into serial stages, merge each through the lane |

Do not retry an identical failing payload. Do not weaken CI, security or
acceptance gates to get green — `agent-exec` refuses edits to `deny.toml` and
to the blocking workflows for exactly that reason.

Only these need the owner: credentials and secrets, external cost, weakening a
security or CI policy, removing an existing Aren feature, architecture beyond
the work order, publishing anywhere other than the existing Aren release path,
and a genuine product fork where two different user-visible behaviors both
satisfy the issue.

## 6. Handoff

A session may end at any moment. Before it does:

1. **Edit the state block in the CONTROL body.** This is the handoff. Update
   `updated_at`, `updated_by`, `main`, the item's `status`, `branch`, `pr`, and
   `blockers`.
2. Post **one** log comment, at most fifteen lines: what moved, the evidence
   (run IDs, SHAs), and the exact next action.

Do not restate the state block in prose. Do not post a fresh "AUTHORITATIVE
checkpoint" — that is what the state block is, and stacking them is what made
the previous CONTROL unreadable.

`SESSION-CUT` does not mean stop. The next session reads the state block and
continues.

## 7. CONTROL rollover

Roll when the body exceeds roughly 150 lines or the issue exceeds 40 comments,
whichever comes first. This is a hard limit, not a judgement call: an
unreadable CONTROL is the failure mode this document exists to prevent.

1. create the successor with a complete, current state block,
2. link the predecessor as archive,
3. add `control:active` to the successor,
4. remove `control:active` from the predecessor and close it,
5. verify exactly one OPEN issue has `control:active`.
