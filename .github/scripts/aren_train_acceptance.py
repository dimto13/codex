#!/usr/bin/env python3
"""Verify owner-visible acceptance for a merged autonomous Aren train."""
from __future__ import annotations

import os
import re
import sys

from aren_release_tag import GateError, GitHub
from aren_train_live import (
    linked_pr,
    main_ci_result,
    paginated_list,
    train_contract,
)

SHA = re.compile(r"[0-9a-f]{40}\Z")


def verify(api: GitHub, issue_number: int, main_sha: str) -> str:
    if not SHA.fullmatch(main_sha):
        raise GateError("Acceptance requires an exact 40-character main SHA")
    issue = api.request("GET", f"issues/{issue_number}")
    if not issue:
        raise GateError("Acceptance issue is unavailable")
    contract = train_contract(issue.get("body") or "")
    if not contract["acceptance_required"]:
        raise GateError("Acceptance workflow invoked for a train that does not require acceptance")

    main = api.request("GET", "branches/main")
    if (main or {}).get("commit", {}).get("sha") != main_sha:
        raise GateError("Acceptance SHA is not current main")

    pr = linked_pr(api, issue_number)
    if not pr:
        raise GateError("Acceptance requires a canonical implementation PR")
    detail = api.request("GET", f"pulls/{pr['number']}")
    if not detail or detail.get("merged") is not True:
        raise GateError("Canonical implementation PR is not merged")
    if detail.get("merge_commit_sha") != main_sha:
        raise GateError("Canonical PR merge SHA does not equal acceptance main SHA")
    if main_ci_result(api, main_sha) != "success":
        raise GateError("Exact resulting-main blocking CI is not green")

    comments = paginated_list(api, f"issues/{issue_number}/comments")
    worker = re.compile(
        rf"<!--\s*aren:worker:success\s+issue={issue_number}\s+"
        r"head=[0-9a-f]{40}\s+action=[A-Za-z0-9_-]+\s+run=([0-9]+)\s*-->"
    )
    worker_runs = [
        worker.search(comment.get("body") or "")
        for comment in comments
    ]
    run_ids = [match.group(1) for match in worker_runs if match]
    if not run_ids:
        raise GateError("No successful capable-worker consumption evidence exists")

    marker = f"<!-- aren:acceptance:success main={main_sha} -->"
    if any(marker in (comment.get("body") or "") for comment in comments):
        return "already_verified"

    body = (
        f"{marker}\n\n"
        f"Autonomous train acceptance verified on exact resulting main `{main_sha}`.\n"
        f"Evidence includes successful worker run(s): {', '.join(run_ids[-5:])}; "
        "canonical PR merged; exact resulting-main blocking CI green."
    )
    api.request("POST", f"issues/{issue_number}/comments", {"body": body})
    return "verified"


def main() -> int:
    try:
        issue_number = int(sys.argv[1])
        main_sha = sys.argv[2]
        api = GitHub(os.environ.get("GITHUB_REPOSITORY", ""),
                     os.environ.get("GITHUB_TOKEN", ""))
        print(verify(api, issue_number, main_sha))
        return 0
    except (GateError, ValueError, IndexError) as exc:
        print(f"FAIL_CLOSED: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
