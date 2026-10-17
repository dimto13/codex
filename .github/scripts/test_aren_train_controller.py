import unittest
from unittest.mock import patch

from aren_release_tag import GateError
from aren_train_controller import execute
from aren_train_state import Snapshot


class FakeAPI:
    def __init__(self):
        self.repo = "dimto13/codex"
        self.comments = []
        self.workflow_runs = []
        self.writes = []
        self.graphql_writes = []

    def request(self, method, path, payload=None):
        if method == "GET" and path.startswith("issues/81/comments"):
            return self.comments
        if method == "GET" and path == "issues/94":
            return {"number": 94, "state": "open", "body": "body"}
        if method == "GET" and path.startswith("actions/workflows/blocking-ci.yml/runs"):
            return {"workflow_runs": self.workflow_runs}
        if method == "POST" and path == "actions/workflows/blocking-ci.yml/dispatches":
            self.writes.append((method, path, payload))
            return None
        if method == "POST" and path == "issues/81/comments":
            self.comments.append({"body": payload["body"]})
            self.writes.append((method, path, payload))
            return {"id": 1}
        if method == "PATCH" and path == "issues/94":
            self.writes.append((method, path, payload))
            return {"number": 94, "state": payload["state"]}
        if method == "PUT" and path == "pulls/96/merge":
            self.writes.append((method, path, payload))
            return {"merged": True, "sha": "c" * 40}
        raise AssertionError((method, path, payload))

    def graphql(self, query, variables=None):
        self.graphql_writes.append((query, variables))
        return {"markPullRequestReadyForReview": {"pullRequest": {"isDraft": False}}}


PR = {
    "number": 96,
    "node_id": "PR_node",
    "draft": True,
    "head": {"sha": "a" * 40, "ref": "chat1/94-autonomous-train-controller"},
}


class ControllerTests(unittest.TestCase):
    def setUp(self):
        self.api = FakeAPI()
        self.control = {"number": 81}

    def run_execute(self, snapshot, pr=PR):
        with patch("aren_train_controller.open_active_control", return_value=self.control), \
             patch("aren_train_controller.build_snapshot", return_value=(94, snapshot)), \
             patch("aren_train_controller.current_pr", return_value=pr):
            return execute(self.api)

    def test_missing_head_ci_is_dispatched_instead_of_idling(self):
        snapshot = Snapshot(
            control_count=1, authorized=True, has_pr=True,
            head_sha="a" * 40, base_sha="b" * 40, main_sha="b" * 40,
            head_ci=None, external_ci="not_required",
        )
        result = self.run_execute(snapshot)
        self.assertEqual(result["outcome"], "ci_dispatched")
        self.assertEqual(
            self.api.writes,
            [("POST", "actions/workflows/blocking-ci.yml/dispatches",
              {"ref": "chat1/94-autonomous-train-controller"})],
        )

    def test_actionable_code_state_escalates_once(self):
        snapshot = Snapshot(
            control_count=1, authorized=True, has_pr=True,
            head_sha="a" * 40, base_sha="b" * 40, main_sha="b" * 40,
            head_ci="failure", external_ci="not_required",
        )
        first = self.run_execute(snapshot)
        second = self.run_execute(snapshot)
        self.assertEqual(first["outcome"], "escalated")
        self.assertEqual(second["outcome"], "already_escalated")
        self.assertEqual(len(self.api.comments), 1)

    def test_fully_qualified_lane_marks_ready_and_merges_exact_head(self):
        snapshot = Snapshot(
            control_count=1, authorized=True, has_pr=True,
            head_sha="a" * 40, base_sha="b" * 40, main_sha="b" * 40,
            head_ci="success", external_ci="not_required",
            reviews_clean=True, threads_clean=True, mergeable=True,
            scope_accepted=True, preservation_verified=True,
        )
        result = self.run_execute(snapshot)
        self.assertEqual(result["outcome"], "merged")
        self.assertTrue(self.api.graphql_writes)
        merge = [write for write in self.api.writes if write[1] == "pulls/96/merge"]
        self.assertEqual(merge[0][2]["sha"], "a" * 40)

    def test_done_closes_issue_and_records_control_evidence(self):
        snapshot = Snapshot(
            control_count=1, authorized=True, has_pr=True, merged=True,
            head_sha="a" * 40, main_sha="c" * 40,
            resulting_main_sha="c" * 40, resulting_main_ci="success",
        )
        result = self.run_execute(snapshot, pr=None)
        self.assertEqual(result["outcome"], "issue_closed")
        self.assertIn(("PATCH", "issues/94", {"state": "closed"}), self.api.writes)
        self.assertEqual(len(self.api.comments), 1)

    def test_release_without_explicit_tag_marker_fails_closed(self):
        snapshot = Snapshot(
            control_count=1, authorized=True, has_pr=True, merged=True,
            head_sha="a" * 40, main_sha="c" * 40,
            resulting_main_sha="c" * 40, resulting_main_ci="success",
            release_required=True,
        )
        with self.assertRaises(GateError):
            self.run_execute(snapshot, pr=None)


if __name__ == "__main__":
    unittest.main()
