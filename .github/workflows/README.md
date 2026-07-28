# Aren Workflow Strategy

GitHub Actions provides a deliberately small feedback loop for pull requests and
pushes to `main`. The separate release workflow runs only for explicit Aren
release tags or a manually requested rehearsal.

## Active GitHub CI

`blocking-ci.yml` is the only event-driven CI workflow. It runs:

- the changed-blob size policy;
- `cargo-deny`;
- Rust formatting and the benchmark smoke test;
- `cargo shear`.

These checks were selected because they already run successfully on standard
GitHub-hosted Linux runners in this fork.

## Aren Releases

`aren-release.yml` builds Linux x86_64, Linux ARM64 and Windows x86_64 packages
on their native GitHub-hosted runners. A manual run uploads temporary test
artifacts. Only a tag matching `aren-v*` publishes an immutable GitHub Release.

## Disabled Upstream Workflows

The remaining upstream workflows stay in the repository as implementation
references but are disabled in the GitHub repository. They depend on
OpenAI-specific infrastructure, large cross-platform matrices, release
credentials, or packaging paths that Aren does not currently use.
