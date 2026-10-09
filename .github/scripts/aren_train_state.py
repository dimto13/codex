#!/usr/bin/env python3
"""Fail-closed, side-effect-free transition decision for an authorized Aren train.

The caller must obtain the snapshot from live GitHub CONTROL/PR/check/review data.
This module never treats chat state, stale CI, or absent evidence as success.
"""
from __future__ import annotations

import dataclasses
import enum
import json
import sys


class State(str, enum.Enum):
    ACTIVE_IMPLEMENTATION = "ACTIVE_IMPLEMENTATION"
    CI_FIX_LOOP = "CI_FIX_LOOP"
    REVIEW_FIX_LOOP = "REVIEW_FIX_LOOP"
    READY_TO_MERGE = "READY_TO_MERGE"
    MERGED_PENDING_MAIN_CI = "MERGED_PENDING_MAIN_CI"
    READY_TO_RELEASE = "READY_TO_RELEASE"
    RELEASING = "RELEASING"
    ACCEPTANCE_SMOKE = "ACCEPTANCE_SMOKE"
    DONE = "DONE"
    HARD_BLOCKED = "HARD_BLOCKED"


@dataclasses.dataclass(frozen=True)
class Snapshot:
    """Live evidence for exactly one CONTROL-authorized queue item.

    None means unknown and MUST NOT be treated as a passing gate.
    """

    control_count: int
    authorized: bool
    owner_gate: str | None = None
    has_pr: bool = False
    merged: bool = False
    head_sha: str | None = None
    base_sha: str | None = None
    main_sha: str | None = None
    head_ci: str | None = None  # success, pending, failure
    reviews_clean: bool | None = None
    threads_clean: bool | None = None
    mergeable: bool | None = None
    scope_accepted: bool | None = None
    preservation_verified: bool | None = None
    external_ci: str | None = None  # success, pending, failure, not_required
    resulting_main_sha: str | None = None
    resulting_main_ci: str | None = None
    release_required: bool = False
    release_tag_qualified: bool = False
    release_started: bool = False
    release_success: bool = False
    acceptance_required: bool = False
    acceptance_verified: bool = False


@dataclasses.dataclass(frozen=True)
class Decision:
    state: State
    action: str
    reason: str


def decide(s: Snapshot) -> Decision:
    if s.control_count != 1:
        return Decision(State.HARD_BLOCKED, "fail_closed", "CONTROL_PLANE_BLOCKED: exactly one open active CONTROL required")
    if not s.authorized:
        return Decision(State.HARD_BLOCKED, "fail_closed", "No live CONTROL authorization for this queue item")
    if s.owner_gate:
        return Decision(State.HARD_BLOCKED, "escalate_owner_gate", s.owner_gate)

    if s.merged:
        if not s.resulting_main_sha or s.main_sha != s.resulting_main_sha:
            return Decision(State.MERGED_PENDING_MAIN_CI, "reconcile_main", "Resulting exact main SHA not verified")
        if s.resulting_main_ci != "success":
            action = "repair_main_ci" if s.resulting_main_ci == "failure" else "poll_main_ci"
            return Decision(State.MERGED_PENDING_MAIN_CI, action, "Resulting exact-main blocking CI is not green")
        if s.release_required:
            if not s.release_tag_qualified:
                return Decision(State.READY_TO_RELEASE, "qualify_and_tag", "Release tag requires exact-green main")
            if not s.release_started:
                return Decision(State.READY_TO_RELEASE, "dispatch_release", "Tag qualified; canonical release not started")
            if not s.release_success:
                return Decision(State.RELEASING, "repair_or_poll_release", "Canonical release not yet successful")
        if s.acceptance_required and not s.acceptance_verified:
            return Decision(State.ACCEPTANCE_SMOKE, "run_acceptance", "Real owner-visible acceptance still missing")
        return Decision(State.DONE, "record_done", "Exact resulting-main CI and all applicable gates passed")

    if not s.has_pr:
        return Decision(State.ACTIVE_IMPLEMENTATION, "implement_or_open_pr", "Authorized item has no PR")
    if not s.head_sha or not s.base_sha or not s.main_sha:
        return Decision(State.ACTIVE_IMPLEMENTATION, "refresh_pr_evidence", "Missing exact head/base/main SHA")
    if s.base_sha != s.main_sha:
        return Decision(State.ACTIVE_IMPLEMENTATION, "rebase", "PR base is stale against current main")
    if s.head_ci == "failure" or s.external_ci == "failure":
        return Decision(State.CI_FIX_LOOP, "fix_ci", "Exact-head CI failure")
    if s.head_ci != "success" or s.external_ci not in ("success", "not_required"):
        return Decision(State.ACTIVE_IMPLEMENTATION, "poll_exact_head_ci", "Exact-head CI or external validation pending/unknown")
    if s.reviews_clean is False or s.threads_clean is False:
        return Decision(State.REVIEW_FIX_LOOP, "fix_review_findings", "Open review findings/threads")
    if s.reviews_clean is not True or s.threads_clean is not True:
        return Decision(State.REVIEW_FIX_LOOP, "inspect_reviews", "Review/thread evidence unknown")
    if s.mergeable is not True or s.scope_accepted is not True or s.preservation_verified is not True:
        return Decision(State.ACTIVE_IMPLEMENTATION, "verify_merge_gates", "Mergeability/scope/preservation not proven")
    return Decision(State.READY_TO_MERGE, "merge_if_control_permits", "Exact-head merge lane fully qualified")


def main() -> int:
    try:
        raw = json.load(sys.stdin)
        snapshot = Snapshot(**raw)
        decision = decide(snapshot)
        print(json.dumps(dataclasses.asdict(decision)))
        return 0 if decision.state != State.HARD_BLOCKED else 2
    except (ValueError, TypeError, json.JSONDecodeError) as exc:
        print(json.dumps({"state": "HARD_BLOCKED", "action": "fail_closed", "reason": str(exc)}))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
