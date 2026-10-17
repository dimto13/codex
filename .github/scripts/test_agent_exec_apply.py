#!/usr/bin/env python3
"""Tests for the structured agent-exec edit applier."""

import tempfile
import unittest
from pathlib import Path

from agent_exec_apply import ApplyError
from agent_exec_apply import apply_operations


class ApplyOperationsTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name).resolve()
        (self.root / "src").mkdir()
        self.existing = self.root / "src/lib.rs"
        self.existing.write_text("mod a;\nmod b;\n", encoding="utf-8")
        self.addCleanup(self._tmp.cleanup)

    def test_replace_inserts_alongside_its_anchor(self):
        touched = apply_operations(
            [{"op": "replace", "path": "src/lib.rs", "find": "mod b;", "replace": "mod b;\nmod c;"}],
            self.root,
        )
        self.assertEqual(touched, ["src/lib.rs"])
        self.assertEqual(self.existing.read_text(encoding="utf-8"), "mod a;\nmod b;\nmod c;\n")

    def test_create_writes_a_new_file_and_its_parents(self):
        apply_operations(
            [{"op": "create", "path": "src/deep/new.rs", "content": "fn main() {}\n"}],
            self.root,
        )
        self.assertEqual((self.root / "src/deep/new.rs").read_text(encoding="utf-8"), "fn main() {}\n")

    def test_delete_removes_the_file(self):
        apply_operations([{"op": "delete", "path": "src/lib.rs"}], self.root)
        self.assertFalse(self.existing.exists())

    def test_missing_anchor_fails_closed(self):
        with self.assertRaises(ApplyError):
            apply_operations(
                [{"op": "replace", "path": "src/lib.rs", "find": "mod z;", "replace": "x"}],
                self.root,
            )

    def test_ambiguous_anchor_fails_closed(self):
        self.existing.write_text("mod a;\nmod a;\n", encoding="utf-8")
        with self.assertRaises(ApplyError):
            apply_operations(
                [{"op": "replace", "path": "src/lib.rs", "find": "mod a;", "replace": "mod q;"}],
                self.root,
            )

    def test_declared_count_allows_a_repeated_anchor(self):
        self.existing.write_text("mod a;\nmod a;\n", encoding="utf-8")
        apply_operations(
            [{"op": "replace", "path": "src/lib.rs", "find": "mod a;", "replace": "mod q;", "count": 2}],
            self.root,
        )
        self.assertEqual(self.existing.read_text(encoding="utf-8"), "mod q;\nmod q;\n")

    def test_nothing_is_written_when_a_later_operation_fails(self):
        with self.assertRaises(ApplyError):
            apply_operations(
                [
                    {"op": "replace", "path": "src/lib.rs", "find": "mod a;", "replace": "mod q;"},
                    {"op": "replace", "path": "src/lib.rs", "find": "absent", "replace": "x"},
                ],
                self.root,
            )
        self.assertEqual(self.existing.read_text(encoding="utf-8"), "mod a;\nmod b;\n")

    def test_create_refuses_to_overwrite_an_existing_file(self):
        with self.assertRaises(ApplyError):
            apply_operations([{"op": "create", "path": "src/lib.rs", "content": ""}], self.root)

    def test_rejects_paths_escaping_the_repository(self):
        for path in ("../outside.rs", "/etc/passwd", "src/../../outside.rs"):
            with self.assertRaises(ApplyError):
                apply_operations([{"op": "delete", "path": path}], self.root)

    def test_rejects_unknown_operation(self):
        with self.assertRaises(ApplyError):
            apply_operations([{"op": "exec", "path": "src/lib.rs"}], self.root)

    def test_rejects_empty_payload(self):
        with self.assertRaises(ApplyError):
            apply_operations([], self.root)


if __name__ == "__main__":
    unittest.main()
