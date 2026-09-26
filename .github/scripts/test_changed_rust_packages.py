#!/usr/bin/env python3
"""Tests for the changed-package resolver used by the blocking test job."""

import tempfile
import unittest
from pathlib import Path

from changed_rust_packages import package_name
from changed_rust_packages import packages_for

CLI_MANIFEST = """[package]
name = "codex-cli"
version = "0.1.0"

[[bench]]
name = "not-the-package"
"""

DAEMON_MANIFEST = """[package]
name = "codex-app-server-daemon"
"""

VIRTUAL_MANIFEST = """[workspace]
members = ["cli"]
"""


class ChangedRustPackagesTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        (self.root / "codex-rs").mkdir()
        (self.root / "codex-rs/Cargo.toml").write_text(VIRTUAL_MANIFEST, encoding="utf-8")
        for name, manifest in (("cli", CLI_MANIFEST), ("app-server-daemon", DAEMON_MANIFEST)):
            crate = self.root / "codex-rs" / name / "src"
            crate.mkdir(parents=True)
            (crate.parent / "Cargo.toml").write_text(manifest, encoding="utf-8")
        self.addCleanup(self._tmp.cleanup)

    def test_reads_package_name_and_ignores_later_tables(self):
        self.assertEqual(package_name(self.root / "codex-rs/cli/Cargo.toml"), "codex-cli")

    def test_virtual_manifest_has_no_package_name(self):
        self.assertIsNone(package_name(self.root / "codex-rs/Cargo.toml"))

    def test_resolves_and_deduplicates_owning_packages(self):
        changed = [
            "codex-rs/cli/src/remote_control_cmd.rs",
            "codex-rs/cli/src/lib.rs",
            "codex-rs/app-server-daemon/src/client.rs",
        ]
        self.assertEqual(
            packages_for(changed, self.root),
            ["codex-app-server-daemon", "codex-cli"],
        )

    def test_ignores_paths_outside_any_crate(self):
        changed = [".github/workflows/agent-exec.yml", "ops/control-plane.md", "README.md"]
        self.assertEqual(packages_for(changed, self.root), [])


if __name__ == "__main__":
    unittest.main()
