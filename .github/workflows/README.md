# Aren Workflow Strategy

GitHub Actions provides a deliberately small feedback loop for pull requests and
pushes to `main`. Builds, packaging and releases belong to the future Jenkins
pipeline.

## Active GitHub CI

`blocking-ci.yml` is the only event-driven CI workflow. It runs:

- the changed-blob size policy;
- `cargo-deny`;
- Rust formatting and the benchmark smoke test;
- `cargo shear`.

These checks were selected because they already run successfully on standard
GitHub-hosted Linux runners in this fork.

## Disabled Upstream Workflows

The remaining upstream workflows stay in the repository as implementation
references but are disabled in the GitHub repository. They depend on
OpenAI-specific infrastructure, large cross-platform matrices, release
credentials, or packaging paths that Aren does not currently use.

Releases remain independent of Actions and can be published by Jenkins through
the GitHub API after Jenkins has built and verified the release payload.
