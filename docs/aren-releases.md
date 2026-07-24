# Aren builds and updates

`aren` is the personal Codex build from `dimto13/codex`. It is installed under a
different name so it can coexist with the official `codex` command.

## Current release process: GitHub Actions

For now, GitHub Actions and GitHub Releases are the primary Aren release path.
This keeps release compilation away from local machines and the resource-limited
Jenkins NAS. The public repository can use standard GitHub-hosted runners
without consuming paid Actions minutes.

The standard hosted runner is an ephemeral virtual machine and is discarded
after the job, so it does not pollute Jenkins with Rust, V8, linker, or packaging
dependencies. A separate build container is therefore not required for the
initial process. If a self-hosted runner becomes necessary later, its build
should run in a pinned container with the same two-job Cargo limit. Jenkins may
download and deploy a verified release, but is not the primary compiler.

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

The currently verified release is `aren-v0.1.1`. The initial `aren-v0.1.0`
release remains immutable; live validation found a cancelled Chrome tool
approval there, so the correction was published as a new patch release.

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

Use an isolated Chrome profile when several Codex/Aren processes or quality
runs can overlap:

```toml
[mcp_servers.chrome-devtools]
command = "npx"
args = [
  "-y",
  "chrome-devtools-mcp@1.6.0",
  "--executablePath",
  "/usr/bin/google-chrome-stable",
  "--headless",
  "true",
  "--isolated",
  "true",
]
```

The suite rejects missing sources and answers that report a blocked, cancelled,
failed, or unavailable Chrome path. A successful result is written only after
every case passes.

The GitHub release updater remains available for direct installation:

```shell
aren-update --tag aren-v0.1.1
```

The updater downloads only from `dimto13/codex` and writes only the `aren`
executable. It never replaces the official `codex` installation.

## Verified release record

The following chain was completed on 24 July 2026:

- GitHub Actions run:
  <https://github.com/dimto13/codex/actions/runs/30077082603>
- Immutable release:
  <https://github.com/dimto13/codex/releases/tag/aren-v0.1.1>
- Release commit:
  `36e71bffec126d1d13356e1608e66c117219448f`
- SHA-256 verification passed; archive and standalone binary were byte-identical.
- `BUILD-INFO.txt`, the annotated tag, and the downloaded binary all resolved to
  the same commit.
- Local quick result:
  `~/.local/state/aren/live-quality/20260724T085320Z/RESULT.txt`
- Local full result:
  `~/.local/state/aren/live-quality/20260724T085416Z/RESULT.txt`

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
