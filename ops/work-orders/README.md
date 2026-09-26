# Work orders

A queue item in the CONTROL state block is not startable until a work order for
it exists here, on `main`.

Work orders exist because the worker is replaced constantly. An issue written
in prose gets re-interpreted into a slightly different design on every run, so
nothing converges — which is exactly what happened to Q3c. A work order fixes
the design once, in writing, so every session that picks the item up builds the
same thing.

PLAN writes work orders. CHAT1 executes them and does not redesign them. If a
work order has gone stale against the code, that is a reportable blocker
against PLAN, not an invitation to improvise.

Name files `WO-<queue-id>.md`.

## Template

```markdown
# WO-<ID> — <short title>

- **Issue:** #<n>
- **Branch:** `<branch>`
- **Status source:** CONTROL state block, item `<ID>`

## Goal

One paragraph. What a user can do afterwards that they cannot do now.

## Files

Exact paths, each with what happens to it. Respect the `AGENTS.md` module size
rules: prefer a new module over growing a large one, and say which.

## Design

Exact signatures, wire shapes and enum values. No prose where a signature fits.

## Tests

Each test by name, with the file it lives in and the behaviour it pins.

## Acceptance

A command, and the output that counts as passing.

## Out of scope

The things a worker will be tempted to add. Name them so the temptation is
already answered.
```
