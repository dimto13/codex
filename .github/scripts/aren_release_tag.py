#!/usr/bin/env python3
"""Guarded Aren tag bootstrap for an already-authorized release transition.

GITHUB_TOKEN tag pushes do not trigger downstream push workflows. Dispatch the
canonical release workflow explicitly at the tag ref after creating the tag.
"""
from __future__ import annotations

import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request

REQUIRED_CHECKS = frozenset({
    "Blob size policy / Blob size policy",
    "cargo-deny / cargo-deny",
    "Format and benchmark smoke test",
    "Cargo dependency hygiene",
})
TAG_PATTERN = re.compile(r"aren-v[0-9]+\.[0-9]+\.[0-9]+(?:[-.][A-Za-z0-9.-]+)?\Z")
SHA_PATTERN = re.compile(r"[0-9a-f]{40}\Z")


class GateError(RuntimeError):
    pass


class GitHub:
    def __init__(self, repo: str, token: str):
        if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repo):
            raise GateError("Invalid GITHUB_REPOSITORY")
        if not token:
            raise GateError("GITHUB_TOKEN is required")
        self.repo = repo
        self.token = token

    def request(self, method: str, path: str, payload=None):
        url = f"https://api.github.com/repos/{self.repo}/{path}"
        data = json.dumps(payload).encode() if payload is not None else None
        request = urllib.request.Request(url, data=data, method=method, headers={
            "Authorization": f"Bearer {self.token}",
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "aren-release-tag-bootstrap",
        })
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                raw = response.read()
                return json.loads(raw) if raw else None
        except urllib.error.HTTPError as exc:
            if exc.code == 404:
                return None
            raise GateError(f"GitHub {method} {path}: HTTP {exc.code}") from exc


def verify_main_ci(api: GitHub, sha: str) -> None:
    main = api.request("GET", "branches/main")
    if not main or main.get("commit", {}).get("sha") != sha:
        raise GateError("Release SHA must equal current main HEAD")

    runs = api.request("GET", "actions/workflows/blocking-ci.yml/runs?"
                       + urllib.parse.urlencode({"head_sha": sha, "event": "push", "per_page": 30}))
    matches = [r for r in (runs or {}).get("workflow_runs", [])
               if r.get("head_sha") == sha and r.get("head_branch") == "main"
               and r.get("event") == "push"]
    if not matches or max(matches, key=lambda r: r["id"]).get("conclusion") != "success":
        raise GateError("Exact-main blocking-ci push workflow not SUCCESS")

    checks = api.request("GET", f"commits/{sha}/check-runs?per_page=100")
    found = {}
    for check in (checks or {}).get("check_runs", []):
        name = check.get("name")
        if name in REQUIRED_CHECKS:
            found[name] = check
    if set(found) != REQUIRED_CHECKS or any(
        c.get("status") != "completed" or c.get("conclusion") != "success"
        for c in found.values()
    ):
        raise GateError("Missing or non-success exact-main required checks")


def ensure_tag_and_release(api: GitHub, tag: str, sha: str) -> str:
    if not TAG_PATTERN.fullmatch(tag) or not SHA_PATTERN.fullmatch(sha):
        raise GateError("Invalid Aren tag or 40-character commit SHA")
    verify_main_ci(api, sha)

    existing = api.request("GET", f"git/ref/tags/{tag}")
    if existing:
        target = existing.get("object", {})
        if target.get("type") != "commit" or target.get("sha") != sha:
            raise GateError("Existing tag does not point to the exact requested commit")
        result = "existing"
    else:
        api.request("POST", "git/refs", {"ref": f"refs/tags/{tag}", "sha": sha})
        result = "created"

    # GITHUB_TOKEN-created tags do not fire tag-push workflows. Explicit dispatch
    # is required; never dispatch at main because that produces a rehearsal.
    runs = api.request("GET", "actions/workflows/aren-release.yml/runs?"
                       + urllib.parse.urlencode({"head_sha": sha, "per_page": 100}))
    already_started = any(
        r.get("head_sha") == sha and r.get("head_branch") == tag
        and r.get("event") == "workflow_dispatch"
        for r in (runs or {}).get("workflow_runs", [])
    )
    if not already_started:
        api.request("POST", "actions/workflows/aren-release.yml/dispatches", {"ref": tag})
        return f"tag_{result}_release_dispatched"
    return f"tag_{result}_release_already_started"


def main() -> int:
    try:
        tag, sha = sys.argv[1:]
        api = GitHub(os.environ.get("GITHUB_REPOSITORY", ""), os.environ.get("GITHUB_TOKEN", ""))
        print(ensure_tag_and_release(api, tag, sha))
        return 0
    except (ValueError, GateError) as exc:
        print(f"FAIL_CLOSED: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
