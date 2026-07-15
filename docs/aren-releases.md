# Aren builds and updates

`aren` is the personal Codex build from `dimto13/codex`. It is installed under a
different name so it can coexist with the official `codex` command.

## Automated builds

The `aren-release` GitHub Actions workflow builds a Linux x86_64 binary on every
branch push. Every build is retained as a GitHub Actions artifact for 30 days.

Pushes to `custom/interactive-search` additionally replace the rolling
`aren-edge` prerelease. Tags beginning with `aren-v` create immutable releases:

```shell
git tag -a aren-v0.1.0 -m "Aren 0.1.0"
git push origin aren-v0.1.0
```

The workflow packages the compiled `codex` executable as `aren` and publishes a
SHA-256 checksum with it. The current release pipeline targets Linux x86_64.

## Install and update

Install the rolling development build with:

```shell
scripts/update-aren.sh
```

After `scripts/update-aren.sh` is linked as `~/.local/bin/aren-update`, update
from any directory with:

```shell
aren-update
# or, from the Aren CLI:
aren update
```

Install a fixed release instead:

```shell
aren-update --tag aren-v0.1.0
```

The updater downloads only from `dimto13/codex` and writes only the `aren`
executable. It never replaces the official `codex` installation.

## Pull upstream changes

Rebase the personal branch onto the official repository and push the updated
branch to trigger a new Aren build:

```shell
git fetch upstream
git rebase upstream/main
git push --force-with-lease origin custom/interactive-search
```

Use `--force-with-lease` after a rebase so Git refuses to overwrite unexpected
remote work.
