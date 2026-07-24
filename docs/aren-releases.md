# Aren builds and updates

`aren` is the personal Codex build from `dimto13/codex`. It is installed under a
different name so it can coexist with the official `codex` command.

## Current release process: GitHub Actions

For now, GitHub Actions and GitHub Releases are the primary Aren release path.
This keeps release compilation away from local machines and the resource-limited
Jenkins NAS. The public repository can use standard GitHub-hosted runners
without consuming paid Actions minutes.

Tags beginning with `aren-v` build Linux x86_64 with two Cargo jobs, smoke-test
the result, and publish an immutable GitHub Release containing the standalone
binary, compressed archive, `BUILD-INFO.txt`, and SHA-256 checksum. Actions
artifacts expire after seven days; the corresponding GitHub Release assets are
the durable download location.

The initial release is created with:

```shell
git tag -a aren-v0.1.0 -m "Aren 0.1.0"
git push origin aren-v0.1.0
```

The workflow can also be started manually to test a revision without publishing
a release. Normal branch pushes do not start a release build.

The current workflow targets Linux x86_64. Windows artifacts are the next
platform goal and will use a standard GitHub-hosted Windows runner so the native
Windows sandbox binaries are built, packaged, and smoke-tested in their actual
runtime environment.

## Install and update

Install a downloaded GitHub Release artifact with:

```shell
scripts/install-aren-artifact.sh \
  aren-linux-x86_64.tar.gz \
  aren-linux-x86_64.tar.gz.sha256
```

This installs the versioned binary under `~/.local/lib/aren/` and atomically
activates `~/.local/bin/aren`. Run the repeatable live quality suite with:

```shell
scripts/quality/aren-live-quality.sh quick
scripts/quality/aren-live-quality.sh full
```

The live suite requires Chrome DevTools MCP, exercises real
`interactive-search` requests, and stores JSON answers, sources, and error logs
under `~/.local/state/aren/live-quality/`.

The GitHub release updater remains available for direct installation:

```shell
aren-update --tag aren-v0.1.0
```

The updater downloads only from `dimto13/codex` and writes only the `aren`
executable. It never replaces the official `codex` installation.

## Pull upstream changes

Rebase the personal branch onto the official repository and push the updated
branch. Create a new `aren-v*` tag only when an immutable release is intended:

```shell
git fetch upstream
git rebase upstream/main
git push --force-with-lease origin custom/interactive-search
```

Use `--force-with-lease` after a rebase so Git refuses to overwrite unexpected
remote work.
