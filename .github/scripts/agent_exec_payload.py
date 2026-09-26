#!/usr/bin/env python3
"""Parse an ``/agent-exec`` request out of a GitHub issue comment body.

The Aren fork is operated by autonomous chat sessions that reach GitHub only
through the REST API: they can read files and write comments, but they have no
shell, no checkout and no Rust toolchain. ``agent-exec`` is the bridge. A worker
posts a command comment describing an edit, and CI applies it, formats it, runs
the repo-mandated checks and pushes the result back.

Comment shape::

    /agent-exec
    ```yaml
    branch: feature/my-branch
    base: main
    message: "feat(#28): add the thing"
    checks: [fmt, clippy, test]
    packages: [codex-cli]
    push: true
    ```
    ```json
    [{"op": "replace", "path": "codex-rs/cli/src/main.rs", "find": "...", "replace": "..."}]
    ```

Exactly one payload block must follow the settings block: ``json`` (declarative
edit operations, see ``agent_exec_apply.py``) or ``diff`` (applied with
``git apply --3way``). Neither is executable: the runner never runs
worker-authored code, only the repository's own toolchain.
"""

import json
import re
import sys

COMMAND = "/agent-exec"
VALID_CHECKS = ("fmt", "clippy", "test", "bench-smoke", "bazel-lock")
DEFAULT_CHECKS = ("fmt", "clippy", "test")

_FENCE = re.compile(
    r"^[ \t]*```[ \t]*(?P<lang>[A-Za-z0-9_+-]*)[ \t]*\n(?P<body>.*?)(?:^[ \t]*```[ \t]*$)",
    re.MULTILINE | re.DOTALL,
)


class PayloadError(ValueError):
    """Raised when a comment is addressed to agent-exec but is not usable."""


def _parse_settings(text):
    """Parse the deliberately tiny YAML subset used by the settings block.

    A full YAML parser is not available to every runner without an extra
    install step, and the accepted shape is fixed, so the subset is spelled out
    here: ``key: scalar`` and ``key: [a, b]``.
    """
    settings = {}
    for raw_line in text.splitlines():
        line = raw_line.split("#", 1)[0].strip() if not raw_line.strip().startswith("#") else ""
        if not line:
            continue
        if ":" not in line:
            raise PayloadError(f"settings line is not `key: value`: {raw_line!r}")
        key, _, value = line.partition(":")
        key = key.strip()
        value = value.strip()
        if value.startswith("[") and value.endswith("]"):
            items = [item.strip().strip("\"'") for item in value[1:-1].split(",")]
            settings[key] = [item for item in items if item]
        else:
            settings[key] = value.strip("\"'")
    return settings


def _as_bool(value, *, field):
    if isinstance(value, bool):
        return value
    lowered = str(value).strip().lower()
    if lowered in ("true", "yes", "1"):
        return True
    if lowered in ("false", "no", "0"):
        return False
    raise PayloadError(f"`{field}` must be a boolean, got {value!r}")


def parse_comment(body):
    """Return the normalized agent-exec request described by ``body``.

    Returns ``None`` when the comment is not an agent-exec command at all, so
    callers can silently ignore ordinary conversation on the exec issue.
    """
    if body is None:
        return None
    stripped = body.strip()
    if not stripped.startswith(COMMAND):
        return None

    blocks = [(match.group("lang").lower(), match.group("body")) for match in _FENCE.finditer(body)]
    if not blocks:
        raise PayloadError("no fenced blocks found; expected a settings block and a payload block")

    settings_blocks = [text for lang, text in blocks if lang in ("yaml", "yml")]
    if len(settings_blocks) != 1:
        raise PayloadError(f"expected exactly one ```yaml settings block, found {len(settings_blocks)}")
    settings = _parse_settings(settings_blocks[0])

    payload_blocks = [(lang, text) for lang, text in blocks if lang in ("json", "diff", "patch")]
    if len(payload_blocks) != 1:
        raise PayloadError(
            f"expected exactly one ```json or ```diff payload block, found {len(payload_blocks)}"
        )
    payload_lang, payload = payload_blocks[0]
    mode = "diff" if payload_lang in ("diff", "patch") else "edits"
    if not payload.strip():
        raise PayloadError("payload block is empty")
    if mode == "edits":
        try:
            json.loads(payload)
        except json.JSONDecodeError as error:
            raise PayloadError(f"edit payload is not valid JSON: {error}") from error

    branch = settings.get("branch", "").strip()
    if not branch:
        raise PayloadError("`branch` is required")
    if branch in ("main", "master") or branch.startswith("-"):
        raise PayloadError(f"`branch` must be a work branch, not {branch!r}")

    message = settings.get("message", "").strip()
    push = _as_bool(settings.get("push", True), field="push")
    if push and not message:
        raise PayloadError("`message` is required unless `push: false`")

    checks = settings.get("checks", list(DEFAULT_CHECKS))
    if isinstance(checks, str):
        checks = [checks] if checks else []
    unknown = [check for check in checks if check not in VALID_CHECKS]
    if unknown:
        raise PayloadError(f"unknown checks {unknown}; valid checks are {list(VALID_CHECKS)}")

    packages = settings.get("packages", [])
    if isinstance(packages, str):
        packages = [packages] if packages else []
    bad_packages = [package for package in packages if not re.fullmatch(r"[A-Za-z0-9_.-]+", package)]
    if bad_packages:
        raise PayloadError(f"invalid package names: {bad_packages}")
    if ("clippy" in checks or "test" in checks) and not packages:
        raise PayloadError("`packages` is required when `clippy` or `test` is requested")

    return {
        "branch": branch,
        "base": settings.get("base", "main").strip() or "main",
        "message": message,
        "checks": list(checks),
        "packages": list(packages),
        "push": push,
        "mode": mode,
        "payload": payload,
    }


def main(argv):
    if len(argv) != 3:
        print(f"usage: {argv[0]} <comment-body-file> <output-dir>", file=sys.stderr)
        return 2
    body = open(argv[1], encoding="utf-8").read()
    try:
        request = parse_comment(body)
    except PayloadError as error:
        print(f"::error::agent-exec payload rejected: {error}")
        open(f"{argv[2]}/error.txt", "w", encoding="utf-8").write(str(error))
        return 1
    if request is None:
        print("comment is not an agent-exec command; nothing to do")
        return 3
    payload_name = "payload.diff" if request["mode"] == "diff" else "payload.json"
    open(f"{argv[2]}/{payload_name}", "w", encoding="utf-8").write(request.pop("payload"))
    open(f"{argv[2]}/request.json", "w", encoding="utf-8").write(json.dumps(request, indent=2))
    print(json.dumps(request, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
