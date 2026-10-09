#!/usr/bin/env python3
"""Read live GitHub state for exactly one CONTROL-authorized Aren work item.

This module is deliberately read-only. Unknown evidence never becomes a green
gate. A separate executor must own mutations after the decision is qualified.
"""
from __future__ import annotations

import dataclasses
import json
import os
import re
import sys
import urllib.parse

from aren_release_tag import GateError, GitHub, REQUIRED_CHECKS
from aren_train_state import Snapshot, decide

CHECKPOINT = re.compile(
    r"(?ms)^## LIVE CHECKPOINT[^\n]*\n(.*?)(?=^---[ \t]*$|\Z)"
)
NEXT_ACTION = re.compile(
    r"(?ms)^### EXACT NEXT ACTION[ \t]*\n(.*?)(?=^###? |\Z)"
)


def authorized_issue(body: str) -> int:
    checkpoint = CHECKPOINT.search(body)
    if not checkpoint:
        raise GateError("Missing authoritative LIVE CHECKPOINT")
    next_action = NEXT_ACTION.search(checkpoint.group(1))
    if not next_action:
        raise GateError("Missing EXACT NEXT ACTION in LIVE CHECKPOINT")
    matches = re.findall(r"\bactivate[ \t]+#([0-9]+)\b", next_action.group(1), re.I)
    if len(matches) != 1:
        raise GateError("Exactly one explicitly activated issue is required")
    return int(matches[0])


def open_active_control(api: GitHub) -> dict:
    issues = api.request("GET", "issues?state=open&labels=control%3Aactive&per_page=100")
    if not isinstance(issues, list) or len(issues) >= 100:
        raise GateError("CONTROL discovery incomplete")
    controls = [issue for issue in issues if "pull_request" not in issue]
    if len(controls) != 1:
        raise GateError("CONTROL_PLANE_BLOCKED: exactly one OPEN control:active issue required")
    control = api.request("GET", f"issues/{controls[0]['number']}")
    if not control or control.get("state") != "open":
        raise GateError("Active CONTROL disappeared during discovery")
    return control


def linked_pr(api: GitHub, issue_number: int) -> dict | None:
    prs = api.request("GET", "pulls?state=all&sort=updated&direction=desc&per_page=100")
    if not isinstance(prs, list) or len(prs) >= 100:
        raise GateError("PR discovery incomplete; pagination required")
    pattern = re.compile(rf"\b(?:refs|closes|fixes)[ \t]+#{issue_number}\b", re.I)
    candidates = [pr for pr in prs if pattern.search(pr.get("body") or "")]
    if len(candidates) > 1:
        raise GateError("Multiple PRs linked to authorized issue; reconciliation required")
    return candidates[0] if candidates else None


def check_result(api: GitHub, sha: str, branch: str) -> str | None:
    query = urllib.parse.urlencode({"head_sha": sha, "per_page": 100})
    data = api.request("GET", f"actions/workflows/blocking-ci.yml/runs?{query}")
    runs = [r for r in (data or {}).get("workflow_runs", [])
            if r.get("head_sha") == sha and r.get("head_branch") == branch
            and r.get("event") in ("pull_request", "workflow_dispatch")]
    if not runs:
        return None
    newest = max(runs, key=lambda r: (r.get("run_number", 0), r.get("run_attempt", 0)))
    if newest.get("status") != "completed":
        return "pending"
    if newest.get("conclusion") != "success":
        return "failure"
    checks = api.request("GET", f"commits/{sha}/check-runs?per_page=100")
    if not isinstance(checks, dict) or checks.get("total_count", 101) > 100:
        return None
    found = {c["name"]: c for c in checks.get("check_runs", [])
             if c.get("name") in REQUIRED_CHECKS}
    if set(found) != REQUIRED_CHECKS:
        return None
    if any(c.get("status") != "completed" or c.get("conclusion") != "success"
           for c in found.values()):
        return "failure"
    return "success"


def reviews_result(api: GitHub, number: int) -> bool | None:
    reviews = api.request("GET", f"pulls/{number}/reviews?per_page=100")
    if not isinstance(reviews, list) or len(reviews) >= 100:
        return None
    latest = {}
    for review in reviews:
        user = (review.get("user") or {}).get("login")
        if not user:
            return None
        if review.get("state") in ("APPROVED", "CHANGES_REQUESTED", "DISMISSED"):
            latest[user] = review.get("state")
    return "CHANGES_REQUESTED" not in latest.values()


def build_snapshot(api: GitHub) -> tuple[int, Snapshot]:
    control = open_active_control(api)
    number = authorized_issue(control.get("body") or "")
    issue = api.request("GET", f"issues/{number}")
    if not issue or issue.get("state") != "open":
        raise GateError("Authorized issue missing or no longer open; refresh CONTROL")
    main = api.request("GET", "branches/main")
    main_sha = (main or {}).get("commit", {}).get("sha")
    if not main_sha:
        raise GateError("Cannot determine exact main SHA")
    pr = linked_pr(api, number)
    snapshot = Snapshot(control_count=1, authorized=True, main_sha=main_sha)
    if pr is None:
        return number, snapshot

    detail = api.request("GET", f"pulls/{pr['number']}")
    if not detail:
        raise GateError("Linked PR vanished")
    head = (detail.get("head") or {}).get("sha")
    branch = (detail.get("head") or {}).get("ref")
    if not head or not branch:
        raise GateError("Linked PR has no exact head")
    if detail.get("merged"):
        # The exact resulting-main SHA must still be verified separately.
        return number, dataclasses.replace(snapshot, has_pr=True, merged=True,
                                            head_sha=head, resulting_main_sha=detail.get("merge_commit_sha"))

    comparison = api.request("GET", f"compare/{urllib.parse.quote(main_sha)}...{urllib.parse.quote(head)}")
    merge_base = (comparison or {}).get("merge_base_commit", {}).get("sha")
    ci = check_result(api, head, branch)
    reviews = reviews_result(api, detail["number"])
    # Inline review-thread resolution, external Jenkins, scope and preservation
    # must be independently proven before merge. Never infer from empty comments.
    return number, dataclasses.replace(
        snapshot, has_pr=True, head_sha=head, base_sha=merge_base,
        head_ci=ci, reviews_clean=reviews, mergeable=detail.get("mergeable"),
    )


def main() -> int:
    try:
        api = GitHub(os.environ.get("GITHUB_REPOSITORY", ""),
                     os.environ.get("GITHUB_TOKEN", ""))
        number, snapshot = build_snapshot(api)
        decision = decide(snapshot)
        print(json.dumps({"issue": number, "snapshot": dataclasses.asdict(snapshot),
                          "decision": dataclasses.asdict(decision)}))
        return 0
    except (GateError, ValueError, KeyError, TypeError) as exc:
        print(json.dumps({"status": "CONTROL_PLANE_BLOCKED", "reason": str(exc)}))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
