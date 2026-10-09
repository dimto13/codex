import dataclasses
import unittest

from aren_release_tag import GateError
from aren_train_live import authorized_issue, build_snapshot, open_active_control
from aren_train_state import State, decide

CHECKPOINT = """# Historical note
## LIVE CHECKPOINT — authoritative
### Live state
- Old work: DONE.
### EXACT NEXT ACTION
**CHAT1/implementation controller:** activate #123 from green main, then #124.
---
# Historical Q3/#28 — not active
"""
SHA = "a" * 40


class FakeAPI:
    def __init__(self):
        self.controls = [{"number": 7}]
        self.issue = {"number": 123, "state": "open"}
        self.prs = []

    def request(self, method, path, payload=None):
        assert method == "GET"
        if path.startswith("issues?"):
            return self.controls
        if path == "issues/7":
            return {"number": 7, "state": "open", "body": CHECKPOINT}
        if path == "issues/123":
            return self.issue
        if path == "branches/main":
            return {"commit": {"sha": SHA}}
        if path.startswith("pulls?"):
            return self.prs
        raise AssertionError(path)


class LiveTrainTests(unittest.TestCase):
    def setUp(self):
        self.api = FakeAPI()

    def test_only_exact_next_action_authorizes_item(self):
        self.assertEqual(authorized_issue(CHECKPOINT), 123)
        self.assertEqual(build_snapshot(self.api)[0], 123)

    def test_missing_or_multiple_activation_fails_closed(self):
        with self.assertRaises(GateError):
            authorized_issue(CHECKPOINT.replace("activate #123", "implement #123"))
        with self.assertRaises(GateError):
            authorized_issue(CHECKPOINT.replace("activate #123", "activate #123 and activate #124"))

    def test_multiple_controls_fails_closed(self):
        self.api.controls.append({"number": 8})
        with self.assertRaisesRegex(GateError, "CONTROL_PLANE_BLOCKED"):
            open_active_control(self.api)

    def test_closed_authorized_issue_fails_closed(self):
        self.api.issue["state"] = "closed"
        with self.assertRaises(GateError):
            build_snapshot(self.api)

    def test_no_pr_is_not_merge_ready(self):
        number, snapshot = build_snapshot(self.api)
        self.assertEqual(number, 123)
        self.assertEqual(snapshot.main_sha, SHA)
        self.assertEqual(decide(snapshot).state, State.ACTIVE_IMPLEMENTATION)
        self.assertEqual(decide(snapshot).action, "implement_or_open_pr")

    def test_duplicate_linked_prs_fails_closed(self):
        self.api.prs = [{"number": 1, "body": "Refs #123"},
                        {"number": 2, "body": "Closes #123"}]
        with self.assertRaisesRegex(GateError, "Multiple PRs"):
            build_snapshot(self.api)

    def test_unlinked_pr_does_not_authorize_merge(self):
        self.api.prs = [{"number": 1, "body": "Fixes #124"}]
        _, snapshot = build_snapshot(self.api)
        self.assertFalse(snapshot.has_pr)


if __name__ == "__main__":
    unittest.main()
