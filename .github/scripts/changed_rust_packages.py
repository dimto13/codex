#!/usr/bin/env python3
"""Map changed files to the Cargo packages that own them.

The fork's blocking CI runs `cargo fmt`, `cargo-deny`, `cargo-shear` and a
benchmark smoke test, so a green pull request proves the workspace compiles and
is formatted — but it never runs a test. That gap matters most for the
autonomous worker, which writes tests it cannot execute. This script keeps the
resulting test job scoped: it resolves each changed path to the nearest
enclosing `Cargo.toml` and prints that package's name.

Changes outside any crate (workflows, docs, ops files) resolve to nothing, so
the caller can skip the job entirely.
"""

import argparse
import re
import sys
from pathlib import Path

_NAME = re.compile(r"^\s*name\s*=\s*\"([^\"]+)\"", re.MULTILINE)


def package_name(manifest):
    """Return the `[package] name` declared by ``manifest``, if it has one."""
    text = manifest.read_text(encoding="utf-8")
    package_section = text.split("[package]", 1)
    if len(package_section) != 2:
        return None
    # Stop at the next table header so a `name` from, say, `[[bench]]` cannot
    # be mistaken for the package name.
    body = re.split(r"^\[", package_section[1], maxsplit=1, flags=re.MULTILINE)[0]
    match = _NAME.search(body)
    return match.group(1) if match else None


def packages_for(paths, root):
    """Return the sorted package names owning ``paths``, relative to ``root``."""
    found = set()
    for raw in paths:
        candidate = (root / raw).parent
        while True:
            manifest = candidate / "Cargo.toml"
            if manifest.is_file():
                name = package_name(manifest)
                if name:
                    found.add(name)
                break
            if candidate == root or root not in candidate.parents:
                break
            candidate = candidate.parent
    return sorted(found)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".", help="repository root")
    parser.add_argument(
        "paths",
        nargs="*",
        help="changed paths, relative to the repository root; read from stdin when omitted",
    )
    args = parser.parse_args(argv)
    paths = args.paths or [line.strip() for line in sys.stdin if line.strip()]
    for name in packages_for(paths, Path(args.root).resolve()):
        print(name)
    return 0


if __name__ == "__main__":
    sys.exit(main())
