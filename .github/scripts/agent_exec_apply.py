#!/usr/bin/env python3
"""Apply a structured agent-exec edit payload to the worktree.

The payload is a JSON list of operations rather than a script, so nothing the
worker writes is ever executed. Every operation is declarative and fails closed:
a `replace` whose anchor is missing, or present a different number of times than
stated, aborts the whole payload before anything is written. That is the
discipline the control plane asks for, enforced instead of trusted.

Operations::

    {"op": "replace", "path": "a/b.rs", "find": "...", "replace": "...", "count": 1}
    {"op": "create",  "path": "a/c.rs", "content": "..."}
    {"op": "delete",  "path": "a/d.rs"}

`replace` is also how you insert: match an anchor and put it back alongside the
new text. There is deliberately no whole-file overwrite for an existing file —
reconstructing a file the worker only partially read is how source gets lost.
"""

import argparse
import json
import sys
from pathlib import Path


class ApplyError(ValueError):
    """Raised when a payload is malformed or does not match the worktree."""


def _resolve(root, raw_path):
    if not isinstance(raw_path, str) or not raw_path:
        raise ApplyError(f"`path` must be a non-empty string, got {raw_path!r}")
    candidate = Path(raw_path)
    if candidate.is_absolute() or ".." in candidate.parts:
        raise ApplyError(f"`path` must be repository-relative without '..': {raw_path!r}")
    resolved = (root / candidate).resolve()
    if root not in resolved.parents and resolved != root:
        raise ApplyError(f"`path` escapes the repository: {raw_path!r}")
    return resolved


def _plan_replace(root, operation):
    path = _resolve(root, operation.get("path"))
    find = operation.get("find")
    replace = operation.get("replace")
    if not isinstance(find, str) or not find:
        raise ApplyError(f"{operation.get('path')!r}: `find` must be a non-empty string")
    if not isinstance(replace, str):
        raise ApplyError(f"{operation.get('path')!r}: `replace` must be a string")
    count = operation.get("count", 1)
    if not isinstance(count, int) or count < 1:
        raise ApplyError(f"{operation.get('path')!r}: `count` must be a positive integer")
    if not path.is_file():
        raise ApplyError(f"{operation.get('path')!r}: file does not exist")
    source = path.read_text(encoding="utf-8")
    found = source.count(find)
    if found != count:
        raise ApplyError(
            f"{operation.get('path')!r}: anchor occurs {found} time(s), payload declared {count}. "
            "The file has moved on; re-read it and rebuild the payload."
        )
    return path, source.replace(find, replace)


def _plan_create(root, operation):
    path = _resolve(root, operation.get("path"))
    content = operation.get("content")
    if not isinstance(content, str):
        raise ApplyError(f"{operation.get('path')!r}: `content` must be a string")
    if path.exists():
        raise ApplyError(
            f"{operation.get('path')!r}: already exists. Use `replace` to edit an existing file."
        )
    return path, content


def apply_operations(operations, root):
    """Validate every operation against ``root``, then write. Returns the paths touched.

    Nothing is written until all operations validate, so a payload with a stale
    anchor in its last step leaves the worktree untouched rather than half
    applied.
    """
    if not isinstance(operations, list) or not operations:
        raise ApplyError("payload must be a non-empty JSON list of operations")

    writes = []
    deletes = []
    for index, operation in enumerate(operations):
        if not isinstance(operation, dict):
            raise ApplyError(f"operation {index} is not an object")
        op = operation.get("op")
        if op == "replace":
            writes.append(_plan_replace(root, operation))
        elif op == "create":
            writes.append(_plan_create(root, operation))
        elif op == "delete":
            path = _resolve(root, operation.get("path"))
            if not path.is_file():
                raise ApplyError(f"{operation.get('path')!r}: file does not exist")
            deletes.append(path)
        else:
            raise ApplyError(f"operation {index}: unknown op {op!r}; expected replace, create or delete")

    touched = []
    for path, content in writes:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
        touched.append(path)
    for path in deletes:
        path.unlink()
        touched.append(path)
    return [str(path.relative_to(root)) for path in touched]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("payload", help="JSON file holding the operation list")
    parser.add_argument("--root", default=".", help="repository root")
    args = parser.parse_args(argv)
    root = Path(args.root).resolve()
    try:
        operations = json.loads(Path(args.payload).read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        print(f"::error::agent-exec payload is not valid JSON: {error}")
        return 1
    try:
        for path in apply_operations(operations, root):
            print(f"touched {path}")
    except ApplyError as error:
        print(f"::error::agent-exec payload rejected: {error}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
