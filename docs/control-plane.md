# Control Plane

## Purpose

This repository is operated through autonomous development sessions that can be replaced at any time. GitHub is the persistent operational single source of truth (SSOT); chat sessions must not contain exclusive project state.

Every agent must be able to reconstruct the current objective, active work, branch ownership, pull requests, CI, blockers, completed work, and next actions from GitHub alone.

## Roles

### PLAN

PLAN is the control plane, planner, and watchdog. PLAN owns queue maintenance, dependencies, status reconciliation, the global merge lane, CONTROL maintenance and rollover, scheduler oversight, handoff consolidation, and blocker detection.

PLAN is not a product feature worker. It must not invent feature work, bypass CI/security gates, overwrite worker branches, fabricate release evidence, or move release/deploy state without owner authorization.

### CHAT1

CHAT1 is the implementation worker. CHAT1 may work only on queue items explicitly authorized for CHAT1 in the active CONTROL issue.

Normal lifecycle:

Issue -> Branch -> Implementation -> Tests -> PR -> exact-head CI -> review disposition -> Merge -> resulting main CI -> DONE -> next authorized item

A worker must never modify another worker's branch.

## CONTROL discovery

There must be exactly one OPEN GitHub issue with label `control:active`.

- Exactly one: it is the operative CONTROL SSOT.
- Zero: `CONTROL_PLANE_BLOCKED`.
- More than one: `CONTROL_PLANE_BLOCKED`.

When `CONTROL_PLANE_BLOCKED` applies, agents fail closed:

- no feature mutation,
- no guessing a CONTROL issue number,
- no reuse of a previously known CONTROL issue number as authoritative.

CONTROL issue numbers must never be hardcoded in prompts, schedulers, or operating logic.

## Mandatory pre-mutation reconciliation

Before any repository mutation, an agent must dynamically discover the active CONTROL and live-check the relevant repository state, including:

1. current `main`,
2. release/deploy branch if one exists,
3. open issues and authorized queue items,
4. open pull requests and their heads/bases,
5. exact-head CI for active work,
6. reviews and unresolved review threads,
7. current main CI,
8. dependencies,
9. merge-lane state,
10. owner/release blockers,
11. newest relevant PLAN and worker handoffs.

Historical CONTROL comments are audit history, not automatically current state. Current live GitHub evidence wins when reconciling contradictions.

## Operative states

- `READY`: authorized and may be started.
- `IN_PROGRESS`: active implementation is underway.
- `WAIT`: waiting for a known dependency.
- `BLOCKED`: a concrete technical or organizational cause prevents progress.
- `CI_RUNNING`: required CI is running.
- `MERGE_READY`: acceptance, CI, and review gates are satisfied and the PR is waiting for the merge lane.
- `MERGED_PENDING_MAIN_CI`: merged, but resulting main CI has not yet been validated.
- `DONE`: integrated and resulting main CI is green, with acceptance criteria satisfied.
- `OWNER_PAUSED`: intentionally paused by the owner; this is not a scheduler failure.

`BLOCKED` requires all of:

- concrete cause,
- concrete evidence,
- concrete condition required to unblock.

"I do not know what to do next" is not a valid blocker definition.

## Global merge lane

There is one global merge lane.

Required sequence:

PR fully green -> reviews/threads clean -> merge -> `MERGED_PENDING_MAIN_CI` -> resulting main CI green -> `DONE`

While any item is `MERGED_PENDING_MAIN_CI`, no second PR may be merged.

Do not merge when any of the following is true:

- CI is red,
- required CI is incomplete,
- unresolved review thread exists,
- `CHANGES_REQUESTED` is active,
- a finding is not dispositioned,
- an acceptance criterion is not satisfied.

CI green does not by itself mean feature DONE. Acceptance criteria and CI are separate gates.

## CI failure handling

No blind retry loops.

When CI fails:

1. identify the exact failed workflow/job,
2. read its concrete logs,
3. determine root cause,
4. make a targeted correction,
5. produce new CI through a new commit or a justified retry.

Never weaken CI, security, architecture, or acceptance guards merely to obtain green status.

## Stale branches

If `main` advances after another merge:

1. synchronize the worker branch with current `main`,
2. do not force-overwrite foreign work,
3. run the full relevant CI again,
4. only then re-evaluate merge readiness.

## Handoffs and SESSION-CUT

A chat session may end at any time. Chats are disposable; GitHub state is not.

Before the end of a substantial work session, write a handoff into the active CONTROL issue.

### Worker handoff

```text
## CHAT1 HANDOFF — YYYY-MM-DD HH:MM Europe/Berlin
- Issue / Queue-Position:
- Branch:
- PR:
- Head SHA:
- Status:
- Basis-main:
- PR-CI:
- Reviews / Threads:
- main-CI after merge:
- Result:
- Scope / Files:
- Dependencies:
- Risks / Collisions:
- Next exact action:
```

### PLAN handoff

```text
## PLAN/WATCHDOG HANDOFF — YYYY-MM-DD HH:MM Europe/Berlin
- CONTROL discovery:
- main:
- release/deploy:
- Main-Push Gate:
- CHAT1:
- open PRs:
- CI:
- Merge-Lane:
- Dependencies:
- Scheduler:
- Owner-/Release-Blocker:
- next actions:
```

`SESSION-CUT` does not mean STOP. The next session reconstructs from GitHub and continues the authorized work.

## Owner gates

Agents must not autonomously decide owner-gated actions such as:

- production deployments,
- cost approvals,
- real credentials/secrets,
- external acceptance decisions,
- architecture changes outside authorized scope,
- release/deploy movement without explicit owner authorization.

Project-specific owner gates belong in the active CONTROL issue.

## CONTROL rollover

Roll CONTROL when the body is too large, historical state obscures the current checkpoint, a substantial work wave is complete, or rapid reconstruction is no longer possible.

Procedure:

1. create the successor CONTROL completely,
2. transfer the current checkpoint, queue, dependencies, gates, and next actions,
3. verify the successor,
4. apply `control:active` to the successor,
5. remove `control:active` from the predecessor,
6. close the predecessor as archived,
7. verify again that exactly one OPEN issue has `control:active`.

## Scheduler contract

Canonical two-role cadence:

- PLAN: hourly at minute `00`.
- CHAT1: hourly at minute `30`.

Scheduler prompts must dynamically discover the active CONTROL through `control:active`; they must never contain a CONTROL issue number.

PLAN should reconcile global state between CHAT1 implementation runs. Additional workers may be introduced only when queue size and branch/file ownership can be cleanly separated.
