import unittest

from aren_release_tag import GateError
from aren_train_live import (
    authorized_issue,
    build_snapshot,
    external_ci_result,
    linked_pr,
    open_active_control,
    train_contract,
)
from aren_train_state import State, decide

CHECKPOINT = """# Implementation Control v3 — ACTIVE
## LIVE CHECKPOINT — 2026-10-18 — AUTHORITATIVE
### Live state
- **CONTROL #81:** exactly one OPEN control.
- **#94:** ACTIVE / AUTHORIZED reliability train.
- **#95:** WAIT / next only after #94.
### EXACT NEXT ACTION
**CHAT1:** continue #94 / PR #96 on its current exact head, actively obtain validation and repair findings.
**PLAN:** keep the merge lane serialized.
---
# Historical material
"""
ISSUE_BODY = """## Machine-readable train contract
<!-- aren:owner-gate:none -->
<!-- aren:external-ci:required -->
<!-- aren:acceptance:required -->
<!-- aren:release:not-required -->
"""
SHA = "a" * 40


def page_number(path):
    if "page=" not in path:
        return 1
    return int(path.rsplit("page=", 1)[1].split("&", 1)[0])


class FakeAPI:
    def __init__(self):
        self.controls = [{"number": 81}]
        self.issue = {"number": 94, "state": "open", "body": ISSUE_BODY}
        self.prs = []
        self.comments = []

    def request(self, method, path, payload=None):
        assert method == "GET"
        if path.startswith("issues?"):
            return self.controls if page_number(path) == 1 else []
        if path == "issues/81":
            return {"number": 81, "state": "open", "body": CHECKPOINT}
        if path == "issues/94":
            return self.issue
        if path == "branches/main":
            return {"commit": {"sha": SHA}}
        if path.startswith("pulls?"):
            return self.prs if page_number(path) == 1 else []
        if path.startswith("issues/94/comments"):
            return self.comments if page_number(path) == 1 else []
        raise AssertionError(path)


class LiveTrainTests(unittest.TestCase):
    def setUp(self):
        self.api = FakeAPI()

    def test_actual_control_grammar_authorizes_current_active_item(self):
        self.assertEqual(authorized_issue(CHECKPOINT), 94)
        number, snapshot = build_snapshot(self.api)
        self.assertEqual(number, 94)
        self.assertTrue(snapshot.acceptance_required)
        self.assertFalse(snapshot.release_required)
        self.assertIsNone(snapshot.owner_gate)

    def test_control_grammar_drift_fails_closed_helpfully(self):
        with self.assertRaisesRegex(GateError, "LIVE CHECKPOINT"):
            authorized_issue(CHECKPOINT.replace("## LIVE CHECKPOINT", "## CHECKPOINT"))
        with self.assertRaisesRegex(GateError, "EXACT NEXT ACTION"):
            authorized_issue(CHECKPOINT.replace("### EXACT NEXT ACTION", "### NEXT"))
        with self.assertRaisesRegex(GateError, "LIVE ACTIVE"):
            authorized_issue(CHECKPOINT.replace("**#94:** ACTIVE", "**#94:** WAIT"))

    def test_train_contract_is_explicit_and_fail_closed(self):
        contract = train_contract(ISSUE_BODY)
        self.assertEqual(
            contract,
            {
                "owner_gate": None,
                "external_ci_required": True,
                "acceptance_required": True,
                "release_required": False,
                "release_tag": None,
            },
        )
        with self.assertRaisesRegex(GateError, "aren:acceptance"):
            train_contract(ISSUE_BODY.replace("<!-- aren:acceptance:required -->", ""))

    def test_multiple_controls_fails_closed(self):
        self.api.controls.append({"number": 82})
        with self.assertRaisesRegex(GateError, "CONTROL_PLANE_BLOCKED"):
            open_active_control(self.api)

    def test_closed_authorized_issue_fails_closed(self):
        self.api.issue["state"] = "closed"
        with self.assertRaises(GateError):
            build_snapshot(self.api)

    def test_no_pr_is_not_merge_ready(self):
        number, snapshot = build_snapshot(self.api)
        self.assertEqual(number, 94)
        self.assertEqual(snapshot.main_sha, SHA)
        self.assertEqual(decide(snapshot).state, State.ACTIVE_IMPLEMENTATION)
        self.assertEqual(decide(snapshot).action, "implement_or_open_pr")

    def test_supplementary_pr_cannot_capture_canonical_implementation(self):
        self.api.prs = [
            {"number": 99, "body": "Related bootstrap for #94 / PR #96."},
            {
                "number": 96,
                "body": "<!-- aren:canonical-pr issue=94 -->\n\nRefs #94.",
            },
        ]
        self.assertEqual(linked_pr(self.api, 94)["number"], 96)

    def test_duplicate_canonical_pr_markers_fail_closed(self):
        marker = "<!-- aren:canonical-pr issue=94 -->"
        self.api.prs = [
            {"number": 96, "body": marker},
            {"number": 99, "body": marker},
        ]
        with self.assertRaisesRegex(GateError, "Multiple canonical PR"):
            linked_pr(self.api, 94)

    def test_external_ci_requires_exact_head_marker(self):
        head = "b" * 40
        self.api.comments = [
            {"body": "Jenkins SUCCESS"},
            {"body": f"<!-- aren:external-ci:success head={'c' * 40} -->"},
        ]
        self.assertEqual(external_ci_result(self.api, 94, True, head), "pending")
        self.api.comments.append(
            {"body": f"<!-- aren:external-ci:success head={head} -->"}
        )
        self.assertEqual(external_ci_result(self.api, 94, True, head), "success")

    def test_unlinked_supplementary_pr_does_not_authorize_merge(self):
        self.api.prs = [{"number": 99, "body": "Related bootstrap for #94 / PR #96."}]
        _, snapshot = build_snapshot(self.api)
        self.assertFalse(snapshot.has_pr)


if __name__ == "__main__":
    unittest.main()
