#!/usr/bin/env python3
"""Tests for the agent-exec comment parser."""

import unittest

from agent_exec_payload import PayloadError
from agent_exec_payload import parse_comment

SETTINGS = """```yaml
branch: feature/x
message: "feat: x"
checks: [fmt, clippy, test]
packages: [codex-cli]
```
"""

EDITS = """```json
[{"op": "delete", "path": "a.rs"}]
```
"""


class ParseCommentTest(unittest.TestCase):
    def test_ignores_non_command_comments(self):
        self.assertIsNone(parse_comment("just a normal handoff comment"))
        self.assertIsNone(parse_comment(None))

    def test_parses_edit_request(self):
        request = parse_comment(f"/agent-exec\n{SETTINGS}{EDITS}")
        self.assertEqual(
            request,
            {
                "branch": "feature/x",
                "base": "main",
                "message": "feat: x",
                "checks": ["fmt", "clippy", "test"],
                "packages": ["codex-cli"],
                "push": True,
                "mode": "edits",
                "payload": '[{"op": "delete", "path": "a.rs"}]\n',
            },
        )

    def test_parses_diff_request_without_push(self):
        comment = "/agent-exec\n```yaml\nbranch: feature/x\npush: false\nchecks: [fmt]\n```\n```diff\n--- a\n+++ b\n```\n"
        request = parse_comment(comment)
        self.assertEqual(
            request,
            {
                "branch": "feature/x",
                "base": "main",
                "message": "",
                "checks": ["fmt"],
                "packages": [],
                "push": False,
                "mode": "diff",
                "payload": "--- a\n+++ b\n",
            },
        )

    def test_rejects_executable_payload_block(self):
        comment = f"/agent-exec\n{SETTINGS}```bash\nrm -rf /\n```\n"
        with self.assertRaises(PayloadError):
            parse_comment(comment)

    def test_rejects_malformed_edit_json(self):
        with self.assertRaises(PayloadError):
            parse_comment(f"/agent-exec\n{SETTINGS}```json\nnot json\n```\n")

    def test_rejects_main_as_target_branch(self):
        with self.assertRaises(PayloadError):
            parse_comment(f"/agent-exec\n```yaml\nbranch: main\nmessage: m\nchecks: [fmt]\n```\n{EDITS}")

    def test_rejects_unknown_check(self):
        with self.assertRaises(PayloadError):
            parse_comment(f"/agent-exec\n```yaml\nbranch: feature/x\nmessage: m\nchecks: [deploy]\n```\n{EDITS}")

    def test_rejects_missing_packages_for_test_check(self):
        with self.assertRaises(PayloadError):
            parse_comment(f"/agent-exec\n```yaml\nbranch: feature/x\nmessage: m\nchecks: [test]\n```\n{EDITS}")

    def test_rejects_missing_message_when_pushing(self):
        with self.assertRaises(PayloadError):
            parse_comment(f"/agent-exec\n```yaml\nbranch: feature/x\nchecks: [fmt]\n```\n{EDITS}")

    def test_rejects_two_payload_blocks(self):
        with self.assertRaises(PayloadError):
            parse_comment(f"/agent-exec\n{SETTINGS}{EDITS}{EDITS}")

    def test_rejects_missing_payload_block(self):
        with self.assertRaises(PayloadError):
            parse_comment(f"/agent-exec\n{SETTINGS}")


if __name__ == "__main__":
    unittest.main()
