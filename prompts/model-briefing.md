# Model Briefing — dimto13/codex

## Repository identity

Repository: `dimto13/codex`
Default branch: `main`

This repository is operated with an autonomous two-role model:

- `SELF=PLAN`: control plane / planner / watchdog
- `SELF=CHAT1`: implementation worker

GitHub is the persistent operational single source of truth. Chats are disposable execution instances and must not contain exclusive project state.

## Repository-specific constraint

The repository already contains upstream Codex development rules in the root `AGENTS.md`. Those rules remain authoritative for code style, testing, architecture, and repository-specific engineering constraints.

The autonomous operating model in `docs/control-plane.md` is additive. Where an implementation task touches product code, the worker must satisfy both:

1. the active CONTROL authorization and control-plane lifecycle, and
2. all applicable root or nested `AGENTS.md` engineering instructions.

Do not delete, weaken, or casually rewrite existing Codex engineering constraints in order to simplify autonomous operation.

## State reconstruction

At the start of every autonomous run:

1. dynamically discover exactly one OPEN issue labeled `control:active`,
2. fail closed if zero or multiple are found,
3. read the active CONTROL completely,
4. read newest relevant handoffs,
5. inspect live GitHub state before mutation.

Never treat a remembered issue number, branch, PR, SHA, CI result, review state, or queue position as authoritative without live verification.

## Branch and merge discipline

- One worker owns one branch at a time.
- Never overwrite another worker's branch.
- Resume existing authorized work before starting new work.
- Maintain one global merge lane.
- A merge is not DONE until resulting `main` CI is green and acceptance criteria remain satisfied.
- If `main` advances, re-synchronize stale work and rerun the relevant CI.

## Evidence discipline

Do not infer success from incomplete evidence.

- CI green is not equivalent to feature acceptance.
- Merge success is not equivalent to DONE.
- `BLOCKED` requires a concrete cause, evidence, and unblock condition.
- CI failures must be diagnosed from the concrete failing job/logs rather than repeated blind retries.
- Reviews and unresolved threads must be checked explicitly before merge.

## Owner authority

Owner gates defined in the active CONTROL are binding. In particular, do not autonomously perform production deployment, spend money, use or create real secrets/credentials, approve external acceptance, or make architecture/release decisions outside authorized scope.

## Session behavior

A session may end at any time. Before the end of substantial work, persist a handoff in the active CONTROL. A new chat must be able to reconstruct and continue without private chat history.
