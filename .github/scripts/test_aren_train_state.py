import dataclasses
import unittest

from aren_train_state import Snapshot, State, decide


class TrainStateTests(unittest.TestCase):
    def setUp(self):
        self.base = Snapshot(control_count=1, authorized=True, has_pr=True,
                             head_sha="a" * 40, base_sha="b" * 40,
                             main_sha="b" * 40, head_ci="success",
                             external_ci="success", reviews_clean=True,
                             threads_clean=True, mergeable=True,
                             scope_accepted=True, preservation_verified=True)

    def check(self, expected, **changes):
        self.assertEqual(decide(dataclasses.replace(self.base, **changes)).state, expected)

    def test_ready_only_with_full_lane(self):
        self.check(State.READY_TO_MERGE)
        self.check(State.ACTIVE_IMPLEMENTATION, preservation_verified=None)
        self.check(State.REVIEW_FIX_LOOP, threads_clean=None)
        self.check(State.REVIEW_FIX_LOOP, reviews_clean=False)
        self.check(State.CI_FIX_LOOP, head_ci="failure")
        self.check(State.ACTIVE_IMPLEMENTATION, head_ci="pending")
        self.check(State.ACTIVE_IMPLEMENTATION, external_ci=None)
        self.check(State.ACTIVE_IMPLEMENTATION, base_sha="c" * 40)

    def test_control_and_owner_gates(self):
        self.check(State.HARD_BLOCKED, control_count=0)
        self.check(State.HARD_BLOCKED, control_count=2)
        self.check(State.HARD_BLOCKED, authorized=False)
        self.check(State.HARD_BLOCKED, owner_gate="credentials")

    def test_resulting_main_exactness(self):
        changes = dict(merged=True, resulting_main_sha="b" * 40)
        self.check(State.MERGED_PENDING_MAIN_CI, **changes)
        self.check(State.MERGED_PENDING_MAIN_CI, **changes, resulting_main_ci="success", main_sha="c" * 40)
        self.check(State.DONE, **changes, resulting_main_ci="success")
        self.check(State.READY_TO_RELEASE, **changes, resulting_main_ci="success", release_required=True)
        self.check(State.RELEASING, **changes, resulting_main_ci="success", release_required=True,
                   release_tag_qualified=True, release_started=True)
        self.check(State.ACCEPTANCE_SMOKE, **changes, resulting_main_ci="success", acceptance_required=True)
        self.check(State.DONE, **changes, resulting_main_ci="success", acceptance_required=True,
                   acceptance_verified=True)


if __name__ == "__main__":
    unittest.main()
