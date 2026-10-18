#!/usr/bin/env python3
"""Read live GitHub state for exactly one CONTROL-authorized Aren work item.

This module is deliberately read-only. Unknown evidence never becomes a green
gate. A separate executor must own mutations after the decision is qualified.
"""
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

OWNER_GATE = re.compile(r"<!--\s*aren:owner-gate:([^>]+?)\s*-->")
EXTERNAL_CI = re.compile(r"<!--\s*aren:external-ci:(required|not-required)\s*-->")
ACCEPTANCE = re.compile(r"<!--\s*aren:acceptance:(required|not-required)\s*-->")
RELEASE = re.compile(r"<!--\s*aren:release:(required|not-required)(?:\s+tag=([^\s>]+))?\s*-->")
CANONICAL_PR = "<!-- aren:canonical-pr issue={issue} -->"


def paginated_list(api: GitHub, path: str, *, max_pages: int = 20) -> list[dict]:
    items: list[dict] = []
    separator = "&" if "?" in path else "?"
    for page in range(1, max_pages + 1):
        batch = api.request("GET", f"{path}{separator}per_page=100&page={page}")
        if not isinstance(batch, list):
            raise GateError(f"Expected paginated list from {path}")
        items.extend(batch)
        if len(batch) < 100:
            return items
    raise GateError(f"Pagination limit exceeded for {path}")


def train_contract(body: str) -> dict:
    owner = OWNER_GATE.findall(body)
    external = EXTERNAL_CI.findall(body)
    acceptance = ACCEPTANCE.findall(body)
    release = RELEASE.findall(body)
    if len(owner) != 1:
        raise GateError("Train contract must declare exactly one aren:owner-gate marker")
    if len(external) != 1:
        raise GateError("Train contract must declare exactly one aren:external-ci marker")
    if len(acceptance) != 1:
        raise GateError("Train contract must declare exactly one aren:acceptance marker")
    if len(release) != 1:
        raise GateError("Train contract must declare exactly one aren:release marker")
    owner_value = owner[0].strip()
    release_mode, release_tag = release[0]
    if release_mode == "required" and not release_tag:
        raise GateError("Release-required train must declare an exact release tag")
    return {
        "owner_gate": None if owner_value == "none" else owner_value,
        "external_ci_required": external[0] == "required",
        "acceptance_required": acceptance[0] == "required",
        "release_required": release_mode == "required",
        "release_tag": release_tag or None,
    }


def authorized_issue(body: str) -> int:
    checkpoint = CHECKPOINT.search(body)
    if not checkpoint:
        raise GateError("Missing authoritative LIVE CHECKPOINT")
    next_action = NEXT_ACTION.search(checkpoint.group(1))
    if not next_action:
        raise GateError("Missing EXACT NEXT ACTION in LIVE CHECKPOINT")
    # PLAN's next-queue activation is not a CHAT1 authorization.
    assignments = re.findall(
        r"(?m)^\*\*CHAT1(?:/[^*]+)?:\*\*[ \t]*(.*)$",
        next_action.group(1),
    )
    if len(assignments) != 1:
        raise GateError("Exactly one explicit CHAT1 assignment required")
    action = assignments[0]
    explicit = re.findall(r"\bactivate[ \t]+#([0-9]+)\b", action, re.I)
    if len(explicit) > 1:
        raise GateError("Multiple CHAT1 activations are ambiguous")

    live = re.search(
        r"(?ms)^### Live state[ \t]*\n(.*?)(?=^### |\Z)",
        checkpoint.group(1),
    )
    active = (
        re.findall(r"(?m)^-[ \t]+\*\*#([0-9]+):\*\*[ \t]+ACTIVE\b", live.group(1))
        if live else []
    )
    if len(active) > 1:
        raise GateError("Multiple ACTIVE items in authoritative LIVE CHECKPOINT")

    if explicit:
        issue = int(explicit[0])
        if active and int(active[0]) != issue:
            raise GateError("CHAT1 activation conflicts with LIVE ACTIVE item")
        return issue

    if not re.search(r"\b(?:continue|implement|execute)\b", action, re.I):
        raise GateError("CHAT1 continuation instruction missing")
    if len(active) != 1:
        raise GateError("Exactly one LIVE ACTIVE issue required for continuation")
    return int(active[0])


def open_active_control(api: GitHub) -> dict:
    issues = paginated_list(api, "issues?state=open&labels=control%3Aactive")
    controls = [issue for issue in issues if "pull_request" not in issue]
    if len(controls) != 1:
        raise GateError("CONTROL_PLANE_BLOCKED: exactly one OPEN control:active issue required")
    control = api.request("GET", f"issues/{controls[0]['number']}")
    if not control or control.get("state") != "open":
        raise GateError("Active CONTROL disappeared during discovery")
    return control


def linked_pr(api: GitHub, issue_number: int) -> dict | None:
    prs = paginated_list(api, "pulls?state=all&sort=updated&direction=desc")
    explicit_marker = CANONICAL_PR.format(issue=issue_number)
    explicit = [pr for pr in prs if explicit_marker in (pr.get("body") or "")]
    if len(explicit) > 1:
        raise GateError("Multiple canonical PR markers for authorized issue")
    if explicit:
        return explicit[0]

    pattern = re.compile(
        rf"(?im)^(?:refs|closes|fixes)[ \t]+#{issue_number}(?:\b|[.,;:])"
    )
    candidates = [pr for pr in prs if pattern.search(pr.get("body") or "")]
    if len(candidates) > 1:
        raise GateError("Multiple PRs linked to authorized issue; add one canonical PR marker")
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
    try:
        reviews = paginated_list(api, f"pulls/{number}/reviews")
    except GateError:
        return None
    latest = {}
    for review in reviews:
        user = (review.get("user") or {}).get("login")
        if not user:
            return None
        if review.get("state") in ("APPROVED", "CHANGES_REQUESTED", "DISMISSED"):
            latest[user] = review.get("state")
    return "CHANGES_REQUESTED" not in latest.values()


def review_threads_result(api: GitHub, number: int) -> bool | None:
    owner, name = api.repo.split("/", 1)
    data = api.graphql(
        """query($owner:String!, $name:String!, $number:Int!) {
          repository(owner:$owner, name:$name) {
            pullRequest(number:$number) {
              reviewThreads(first:100) {
                nodes { isResolved }
                pageInfo { hasNextPage }
              }
            }
          }
        }""",
        {"owner": owner, "name": name, "number": number},
    )
    threads = (((data.get("repository") or {}).get("pullRequest") or {})
               .get("reviewThreads") or {})
    if threads.get("pageInfo", {}).get("hasNextPage"):
        return None
    nodes = threads.get("nodes")
    if nodes is None:
        return None
    return all(node.get("isResolved") is True for node in nodes)


def external_ci_result(api: GitHub, issue_number: int, required: bool, head: str) -> str:
    if not required:
        return "not_required"
    try:
        comments = paginated_list(api, f"issues/{issue_number}/comments")
    except GateError:
        return "pending"
    ok = f"<!-- aren:external-ci:success head={head} -->"
    failed = f"<!-- aren:external-ci:failure head={head} -->"
    bodies = [comment.get("body") or "" for comment in comments]
    if any(failed in body for body in bodies):
        return "failure"
    if any(ok in body for body in bodies):
        return "success"
    return "pending"


def acceptance_result(api: GitHub, issue_number: int, required: bool, main_sha: str) -> bool:
    if not required:
        return False
    try:
        comments = paginated_list(api, f"issues/{issue_number}/comments")
    except GateError:
        return False
    ok = f"<!-- aren:acceptance:success main={main_sha} -->"
    return any(ok in (comment.get("body") or "") for comment in comments)


def release_result(api: GitHub, required: bool, tag: str | None, main_sha: str) -> tuple[bool, bool, bool]:
    if not required:
        return False, False, False
    if not tag:
        return False, False, False
    ref = api.request("GET", f"git/ref/tags/{tag}")
    target = (ref or {}).get("object", {})
    qualified = target.get("type") == "commit" and target.get("sha") == main_sha
    runs = api.request(
        "GET",
        "actions/workflows/aren-release.yml/runs?"
        + urllib.parse.urlencode({"head_sha": main_sha, "per_page": 100}),
    )
    matches = [
        run for run in (runs or {}).get("workflow_runs", [])
        if run.get("head_sha") == main_sha and run.get("head_branch") == tag
    ]
    if not matches:
        return qualified, False, False
    newest = max(matches, key=lambda run: (run.get("run_number", 0), run.get("run_attempt", 0)))
    return qualified, True, newest.get("status") == "completed" and newest.get("conclusion") == "success"


def main_ci_result(api: GitHub, sha: str) -> str | None:
    query = urllib.parse.urlencode({"head_sha": sha, "event": "push", "per_page": 30})
    data = api.request("GET", f"actions/workflows/blocking-ci.yml/runs?{query}")
    runs = [r for r in (data or {}).get("workflow_runs", [])
            if r.get("head_sha") == sha and r.get("head_branch") == "main"
            and r.get("event") == "push"]
    if not runs:
        return None
    newest = max(runs, key=lambda r: (r.get("run_number", 0), r.get("run_attempt", 0)))
    if newest.get("status") != "completed":
        return "pending"
    if newest.get("conclusion") != "success":
        return "failure"
    checks = api.request("GET", f"commits/{sha}/check-runs?per_page=100")
    found = {check.get("name"): check for check in (checks or {}).get("check_runs", [])
             if check.get("name") in REQUIRED_CHECKS}
    if set(found) != REQUIRED_CHECKS:
        return None
    if any(check.get("status") != "completed" or check.get("conclusion") != "success"
           for check in found.values()):
        return "failure"
    return "success"


def build_snapshot(api: GitHub) -> tuple[int, Snapshot]:
    control = open_active_control(api)
    number = authorized_issue(control.get("body") or "")
    issue = api.request("GET", f"issues/{number}")
    if not issue or issue.get("state") != "open":
        raise GateError("Authorized issue missing or no longer open; refresh CONTROL")
    contract = train_contract(issue.get("body") or "")
    main = api.request("GET", "branches/main")
    main_sha = (main or {}).get("commit", {}).get("sha")
    if not main_sha:
        raise GateError("Cannot determine exact main SHA")
    pr = linked_pr(api, number)
    snapshot = Snapshot(
        control_count=1,
        authorized=True,
        main_sha=main_sha,
        owner_gate=contract["owner_gate"],
        release_required=contract["release_required"],
        acceptance_required=contract["acceptance_required"],
    )
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
        resulting = detail.get("merge_commit_sha")
        release_qualified, release_started, release_success = release_result(
            api, contract["release_required"], contract["release_tag"], main_sha
        )
        return number, dataclasses.replace(
            snapshot,
            has_pr=True,
            merged=True,
            head_sha=head,
            resulting_main_sha=resulting,
            resulting_main_ci=main_ci_result(api, resulting) if resulting == main_sha else None,
            release_tag_qualified=release_qualified,
            release_started=release_started,
            release_success=release_success,
            acceptance_verified=acceptance_result(
                api, number, contract["acceptance_required"], main_sha
            ),
        )

    comparison = api.request("GET", f"compare/{urllib.parse.quote(main_sha)}...{urllib.parse.quote(head)}")
    merge_base = (comparison or {}).get("merge_base_commit", {}).get("sha")
    ci = check_result(api, head, branch)
    reviews = reviews_result(api, detail["number"])
    threads = review_threads_result(api, detail["number"])
    pr_body = detail.get("body") or ""
    scope_accepted = "<!-- aren:scope-accepted -->" in pr_body
    preservation_verified = "<!-- aren:preservation-verified -->" in pr_body
    external_ci = external_ci_result(api, number, contract["external_ci_required"], head)
    return number, dataclasses.replace(
        snapshot,
        has_pr=True,
        head_sha=head,
        base_sha=merge_base,
        head_ci=ci,
        reviews_clean=reviews,
        threads_clean=threads,
        mergeable=detail.get("mergeable"),
        scope_accepted=scope_accepted,
        preservation_verified=preservation_verified,
        external_ci=external_ci,
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
