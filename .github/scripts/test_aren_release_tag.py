import unittest

from aren_release_tag import GateError, REQUIRED_CHECKS, ensure_tag_and_release

SHA = "a" * 40
TAG = "aren-v0.1.12"


class FakeAPI:
    def __init__(self):
        self.main = SHA
        self.ci = "success"
        self.checks = {n: "success" for n in REQUIRED_CHECKS}
        self.tag = None
        self.runs = []
        self.writes = []

    def request(self, method, path, payload=None):
        if method == "POST":
            self.writes.append((path, payload))
            if path == "git/refs":
                self.tag = {"object": {"type": "commit", "sha": payload["sha"]}}
            return None
        if path == "branches/main":
            return {"commit": {"sha": self.main}}
        if path.startswith("actions/workflows/blocking-ci.yml/runs"):
            return {"workflow_runs": [{"id": 5, "head_sha": SHA, "head_branch": "main",
                                       "event": "push", "conclusion": self.ci}]}
        if path.startswith("commits/"):
            return {"check_runs": [{"name": n, "status": "completed", "conclusion": status}
                                   for n, status in self.checks.items()]}
        if path.startswith("git/ref/tags/"):
            return self.tag
        if path.startswith("actions/workflows/aren-release.yml/runs"):
            return {"workflow_runs": self.runs}
        raise AssertionError((method, path))


class ReleaseTagTests(unittest.TestCase):
    def setUp(self):
        self.api = FakeAPI()

    def test_tag_created_and_release_dispatched(self):
        self.assertEqual(ensure_tag_and_release(self.api, TAG, SHA),
                         "tag_created_release_dispatched")
        self.assertEqual([p for p, _ in self.api.writes],
                         ["git/refs", "actions/workflows/aren-release.yml/dispatches"])
        self.assertEqual(self.api.writes[-1][1], {"ref": TAG})

    def test_idempotent_existing_tag_and_run(self):
        self.api.tag = {"object": {"type": "commit", "sha": SHA}}
        self.api.runs = [{"head_sha": SHA, "head_branch": TAG, "event": "workflow_dispatch"}]
        self.assertEqual(ensure_tag_and_release(self.api, TAG, SHA),
                         "tag_existing_release_already_started")
        self.assertEqual(self.api.writes, [])

    def test_wrong_main_rejected_before_write(self):
        self.api.main = "b" * 40
        with self.assertRaises(GateError):
            ensure_tag_and_release(self.api, TAG, SHA)
        self.assertEqual(self.api.writes, [])

    def test_failed_ci_rejected(self):
        self.api.ci = "failure"
        with self.assertRaises(GateError):
            ensure_tag_and_release(self.api, TAG, SHA)
        self.assertEqual(self.api.writes, [])

    def test_missing_or_failed_required_check_rejected(self):
        self.api.checks.pop(next(iter(REQUIRED_CHECKS)))
        with self.assertRaises(GateError):
            ensure_tag_and_release(self.api, TAG, SHA)
        self.assertEqual(self.api.writes, [])

    def test_mismatched_tag_rejected(self):
        self.api.tag = {"object": {"type": "commit", "sha": "b" * 40}}
        with self.assertRaises(GateError):
            ensure_tag_and_release(self.api, TAG, SHA)
        self.assertEqual(self.api.writes, [])

    def test_invalid_tag_rejected(self):
        with self.assertRaises(GateError):
            ensure_tag_and_release(self.api, "not-a-release", SHA)
        self.assertEqual(self.api.writes, [])


if __name__ == "__main__":
    unittest.main()
