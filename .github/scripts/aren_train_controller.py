#!/usr/bin/env python3
"""Execute safe, CONTROL-authorized Aren train transitions.

The controller is deliberately conservative: it performs only routine mutations
that are fully qualified by live GitHub evidence. Work that still needs code or
review changes is escalated back into CONTROL with an exact, de-duplicated action
instead of silently idling.
"""
from __future__ import annotations

import dataclasses
import json
import os
import re
import sys
import urllib.parse

from aren_release_tag import GateError, GitHub, ensure_tag_and_release
from aren_train_live import build_snapshot, linked_pr, open_active_control
from aren_train_state import State, decide

WATCH_MARKER = "<!-- aren-train-watchdog issue={issue} action={action} head={head} -->"


def issue_comments(api: GitHub, number: int) -> list[dict]:
    comments = api.request("GET", f"issues/{number}/comments?per_page=100")
    if not isinstance(comments, list) or len(comments) >= 100:
        raise GateError("Issue comment discovery incomplete")
    return comments


def comment_once(api: GitHub, number: int, marker: str, body: str) -> bool:
    if any(marker in (comment.get("body") or "") for comment in issue_comments(api, number)):
        return False
    api.request("POST", f"issues/{number}/comments", {"body": f"{marker}\n\n{body}"})
    return True


def current_pr(api: GitHub, issue_number: int) -> dict | None:
    pr = linked_pr(api, issue_number)
    if not pr:
        return None
    detail = api.request("GET", f"pulls/{pr['number']}")
    if not detail:
        raise GateError("Linked PR disappeared")
    return detail


def ensure_head_ci_started(api: GitHub, pr: dict) -> str:
    head = (pr.get("head") or {}).get("sha")
    branch = (pr.get("head") or {}).get("ref")
    if not head or not branch:
        raise GateError("PR head/branch unavailable")
    query = urllib.parse.urlencode({"head_sha": head, "per_page": 100})
    runs = api.request("GET", f"actions/workflows/blocking-ci.yml/runs?{query}")
    matching = [
        run for run in (runs or {}).get("workflow_runs", [])
        if run.get("head_sha") == head
        and run.get("head_branch") == branch
        and run.get("event") in ("pull_request", "workflow_dispatch")
    ]
    if matching:
        return "ci_already_started"
    api.request("POST", "actions/workflows/blocking-ci.yml/dispatches", {"ref": branch})
    return "ci_dispatched"


def mark_ready(api: GitHub, pr: dict) -> None:
    if not pr.get("draft"):
        return
    node_id = pr.get("node_id")
    if not node_id:
        raise GateError("Draft PR missing node_id")
    api.graphql(
        """mutation($id:ID!) {
          markPullRequestReadyForReview(input:{pullRequestId:$id}) {
            pullRequest { isDraft }
          }
        }""",
        {"id": node_id},
    )


def merge_pr(api: GitHub, pr: dict) -> str:
    head = (pr.get("head") or {}).get("sha")
    if not head:
        raise GateError("Cannot merge without exact PR head SHA")
    result = api.request(
        "PUT",
        f"pulls/{pr['number']}/merge",
        {"sha": head, "merge_method": "squash"},
    )
    if not result or result.get("merged") is not True:
        raise GateError(f"GitHub did not merge PR #{pr['number']}: {result}")
    return result.get("sha") or ""


def release_tag_from_issue(issue: dict) -> str:
    match = re.search(r"<!--\s*aren:release-tag:([^\s]+)\s*-->", issue.get("body") or "")
    if not match:
        raise GateError("Release transition lacks explicit aren:release-tag marker")
    return match.group(1)


def escalate(api: GitHub, control_number: int, issue_number: int, action: str,
             head: str, reason: str) -> str:
    marker = WATCH_MARKER.format(issue=issue_number, action=action, head=head or "none")
    body = (
        f"## ACTIONABLE TRAIN STATE — #{issue_number}\n\n"
        f"- action: `{action}`\n"
        f"- exact head: `{head or 'none'}`\n"
        f"- reason: {reason}\n\n"
        "This is routine train work, not an owner gate. The authorized worker/controller "
        "must continue this action and drive the train toward DONE."
    )
    return "escalated" if comment_once(api, control_number, marker, body) else "already_escalated"


def execute(api: GitHub) -> dict:
    control = open_active_control(api)
    control_number = control["number"]
    issue_number, snapshot = build_snapshot(api)
    issue = api.request("GET", f"issues/{issue_number}")
    if not issue:
        raise GateError("Authorized issue disappeared")
    decision = decide(snapshot)
    pr = current_pr(api, issue_number)
    head = snapshot.head_sha or snapshot.main_sha or ""

    if decision.state == State.HARD_BLOCKED:
        outcome = escalate(api, control_number, issue_number, decision.action, head, decision.reason)
        return {"issue": issue_number, "state": decision.state.value,
                "action": decision.action, "outcome": outcome}

    if decision.action == "poll_exact_head_ci":
        if not pr:
            return {"issue": issue_number, "state": decision.state.value,
                    "action": decision.action,
                    "outcome": escalate(api, control_number, issue_number,
                                        "implement_or_open_pr", head,
                                        "Authorized work has no linked PR")}
        outcome = ensure_head_ci_started(api, pr)
        return {"issue": issue_number, "state": decision.state.value,
                "action": decision.action, "outcome": outcome}

    if decision.action == "merge_if_control_permits":
        if not pr:
            raise GateError("READY_TO_MERGE without linked PR")
        mark_ready(api, pr)
        merge_sha = merge_pr(api, pr)
        return {"issue": issue_number, "state": decision.state.value,
                "action": decision.action, "outcome": "merged", "merge_sha": merge_sha}

    if decision.action in ("qualify_and_tag", "dispatch_release"):
        tag = release_tag_from_issue(issue)
        result = ensure_tag_and_release(api, tag, snapshot.main_sha or "")
        return {"issue": issue_number, "state": decision.state.value,
                "action": decision.action, "outcome": result}

    if decision.action == "record_done":
        api.request("PATCH", f"issues/{issue_number}", {"state": "closed"})
        marker = f"<!-- aren-train-done issue={issue_number} main={snapshot.main_sha} -->"
        comment_once(
            api,
            control_number,
            marker,
            f"## TRAIN DONE — #{issue_number}\n\n"
            f"Exact resulting main: `{snapshot.main_sha}`. "
            "All applicable machine-readable gates are satisfied.",
        )
        return {"issue": issue_number, "state": decision.state.value,
                "action": decision.action, "outcome": "issue_closed"}

    if decision.action in ("poll_main_ci",):
        return {"issue": issue_number, "state": decision.state.value,
                "action": decision.action, "outcome": "waiting_for_existing_main_ci"}

    outcome = escalate(api, control_number, issue_number, decision.action, head, decision.reason)
    return {"issue": issue_number, "state": decision.state.value,
            "action": decision.action, "outcome": outcome}


def main() -> int:
    try:
        api = GitHub(os.environ.get("GITHUB_REPOSITORY", ""),
                     os.environ.get("GITHUB_TOKEN", ""))
        result = execute(api)
        print(json.dumps(result, sort_keys=True))
        return 0 if result.get("state") != State.HARD_BLOCKED.value else 2
    except (GateError, KeyError, TypeError, ValueError) as exc:
        print(json.dumps({"status": "CONTROL_PLANE_BLOCKED", "reason": str(exc)}))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
