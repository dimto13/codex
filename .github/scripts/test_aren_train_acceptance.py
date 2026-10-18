import unittest
from unittest.mock import patch

from aren_release_tag import GateError
from aren_train_acceptance import verify

MAIN = "c" * 40
HEAD = "a" * 40
ISSUE_BODY = """<!-- aren:owner-gate:none -->
<!-- aren:external-ci:required -->
<!-- aren:acceptance:required -->
<!-- aren:release:not-required -->
"""


class FakeAPI:
    def __init__(self):
        self.repo = "dimto13/codex"
        self.comments = [
            {
                "body": (
                    "<!-- aren:worker:success issue=94 "
                    f"head={HEAD} action=fix_ci run=12345 -->"
                )
            }
        ]
        self.writes = []

    def request(self, method, path, payload=None):
        if method == "GET" and path == "issues/94":
            return {"number": 94, "state": "open", "body": ISSUE_BODY}
        if method == "GET" and path == "branches/main":
            return {"commit": {"sha": MAIN}}
        if method == "GET" and path == "pulls/96":
            return {"number": 96, "merged": True, "merge_commit_sha": MAIN}
        if method == "GET" and path.startswith("issues/94/comments"):
            page = 1 if "page=" not in path else int(path.rsplit("page=", 1)[1])
            return self.comments if page == 1 else []
        if method == "POST" and path == "issues/94/comments":
            self.writes.append(payload["body"])
            self.comments.append({"body": payload["body"]})
            return {"id": 99}
        raise AssertionError((method, path, payload))


class AcceptanceTests(unittest.TestCase):
    def setUp(self):
        self.api = FakeAPI()

    def run_verify(self):
        with patch("aren_train_acceptance.linked_pr", return_value={"number": 96}), \
             patch("aren_train_acceptance.main_ci_result", return_value="success"):
            return verify(self.api, 94, MAIN)

    def test_live_acceptance_requires_worker_and_resulting_main_evidence(self):
        self.assertEqual(self.run_verify(), "verified")
        self.assertIn(f"<!-- aren:acceptance:success main={MAIN} -->", self.api.writes[0])

    def test_acceptance_is_idempotent(self):
        self.assertEqual(self.run_verify(), "verified")
        self.assertEqual(self.run_verify(), "already_verified")
        self.assertEqual(len(self.api.writes), 1)

    def test_missing_capable_worker_evidence_fails_closed(self):
        self.api.comments.clear()
        with self.assertRaisesRegex(GateError, "worker"):
            self.run_verify()

    def test_wrong_resulting_main_fails_closed(self):
        with patch("aren_train_acceptance.linked_pr", return_value={"number": 96}), \
             patch("aren_train_acceptance.main_ci_result", return_value="success"):
            with self.assertRaisesRegex(GateError, "current main"):
                verify(self.api, 94, "d" * 40)


if __name__ == "__main__":
    unittest.main()
