# Aren builds, installation and updates

`aren` is the local-model build from `dimto13/codex`. It coexists with the
official `codex` command and stores its state under `~/.aren`.

## Release architecture

GitHub Actions has two narrowly scoped responsibilities:

- `aren-ci` provides fast pull-request feedback.
- `aren-release` runs only for an explicit `aren-v*` tag or a manual build
  rehearsal.

Normal branch pushes do not build or publish releases. The remaining inherited
workflows are disabled. A tag-triggered release builds on the actual target
runner, smoke-tests the binary, creates a platform archive and SHA-256 checksum,
and publishes immutable GitHub Release assets.

The release targets are:

| Platform | Archive | Standalone binary |
| --- | --- | --- |
| Linux x86_64 | `aren-linux-x86_64.tar.gz` | `aren-linux-x86_64` |
| Linux ARM64 | `aren-linux-aarch64.tar.gz` | `aren-linux-aarch64` |
| Windows x86_64 | `aren-windows-x86_64.zip` | `aren-windows-x86_64.exe` |

Linux archives contain `aren`, `aren-update` and `BUILD-INFO.txt`. The Windows
archive contains `aren.exe`, `aren-update.ps1`, `aren-update.cmd` and
`BUILD-INFO.txt`.

## Install on Linux

The standalone updater detects x86_64 versus ARM64, resolves the latest stable
GitHub Release, verifies its checksum and atomically installs both Aren and the
updater:

```shell
mkdir -p "$HOME/.local/bin"
curl -fsSL \
  https://github.com/dimto13/codex/releases/latest/download/aren-update \
  -o "$HOME/.local/bin/aren-update"
chmod 0755 "$HOME/.local/bin/aren-update"
export PATH="$HOME/.local/bin:$PATH"
aren-update
```

Install a specific immutable release with:

```shell
aren-update --tag aren-v0.1.2
```

A manually downloaded archive can be installed with the repository helper:

```shell
scripts/install-aren-artifact.sh \
  aren-linux-x86_64.tar.gz \
  aren-linux-x86_64.tar.gz.sha256
```

## Install on Windows

Run in PowerShell on Windows x86_64:

```powershell
$installDir = Join-Path $HOME ".local\bin"
New-Item -ItemType Directory -Force -Path $installDir | Out-Null
Invoke-WebRequest `
  https://github.com/dimto13/codex/releases/latest/download/aren-update.ps1 `
  -OutFile (Join-Path $installDir "aren-update.ps1")
Invoke-WebRequest `
  https://github.com/dimto13/codex/releases/latest/download/aren-update.cmd `
  -OutFile (Join-Path $installDir "aren-update.cmd")
& (Join-Path $installDir "aren-update.ps1")
```

Add `$HOME\.local\bin` to the user `PATH`. A particular release can be selected
with:

```powershell
aren-update -Tag aren-v0.1.2
```

## Local prerequisites

The release contains the Aren application, not the model server or personal
configuration. Each target machine needs Ollama and the default model:

```shell
ollama pull gemma4:e4b
```

Run `aren` once Ollama is available. Personal configuration, MCP registrations,
skills and plugins live under `~/.aren/`. Executor must be installed separately
on every machine where its MCP tools should be available. Do not casually copy
credential files such as `auth.json`.

## Create a release

First verify the intended commit and the lightweight CI:

```shell
git status --short
gh pr checks 1 --repo dimto13/codex
```

The release version is taken from the annotated tag and embedded into
`aren --version` and the TUI. For example:

```shell
git tag -a aren-v0.1.2 -m "Aren 0.1.2"
git push origin aren-v0.1.2
```

Watch the real release path:

```shell
gh run list \
  --repo dimto13/codex \
  --workflow aren-release.yml \
  --branch aren-v0.1.2 \
  --limit 1
gh run watch RUN_ID --repo dimto13/codex --exit-status
gh release view aren-v0.1.2 --repo dimto13/codex
```

A manual rehearsal builds and uploads temporary Actions artifacts but does not
publish a GitHub Release:

```shell
gh workflow run aren-release.yml \
  --repo dimto13/codex \
  --ref custom/interactive-search
```

Never move or reuse a tag after its GitHub Release has been published. Correct
a released defect with a new patch version.

## Release verification

After publication, download the release into an empty directory and verify all
checksums:

```shell
gh release download aren-v0.1.2 \
  --repo dimto13/codex \
  --pattern 'aren-*.tar.gz' \
  --pattern 'aren-*.zip' \
  --pattern '*.sha256'
sha256sum --check ./*.sha256
```

On Linux, test the public unauthenticated installation path in an isolated
directory before activating it for daily use. Then run:

```shell
aren --version
aren interactive-search --help
scripts/quality/aren-live-quality.sh quick
scripts/quality/aren-live-quality.sh full
```

The live suite requires Chrome DevTools MCP and stores evidence under
`~/.local/state/aren/live-quality/`.

## Verified release history

`aren-v0.1.1` was published on 24 July 2026 from commit
`36e71bffec126d1d13356e1608e66c117219448f`. Its Linux x86_64 checksum,
archive contents, installed binary and live quality suite were verified. The
release remains immutable as a rollback target.

## Pull upstream changes

Rebase the personal branch onto the official repository and push the updated
branch:

```shell
git fetch upstream
git rebase upstream/main
git push --force-with-lease origin custom/interactive-search
```

Create an `aren-v*` tag only when a new immutable release is intended.
