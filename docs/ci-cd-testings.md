# CI/CD Testing Strategy for Aren

Status: implementation plan; the model-package fixtures, additional CI checks,
and dedicated local model-test script described below still need implementation.
This document does not configure or start any tests.

## Goal and execution boundaries

Catch model-package, configuration, and request-compatibility regressions early,
without downloading large model weights or running inference on GitHub runners.

| Test layer | Execution location | Trigger |
| --- | --- | --- |
| Model-package handling and regression fixtures | GitHub CI | Pull request / push |
| Bounded collection of model-package metadata | Developer machine | Explicit metadata-capture command |
| Heavy tests with real models and the release binary | Test PC `rmi`, user `tobi` | Explicit local script invocation only |

Heavy model tests must not be started by GitHub Actions, including
`workflow_dispatch`, a self-hosted runner job, or a workflow that connects to `rmi`.
They must not be started by cron, systemd timers, Git hooks, or release automation.
A release process may inspect a previously produced report; it must not start or
retry the heavy tests itself.

## Current repository baseline

- `.github/workflows/blocking-ci.yml` runs dependency checks, Rust formatting,
  and benchmark smoke tests. It does not run Rust functional tests.
- `.github/workflows/postmerge-ci.yml` invokes the broad Rust suite after pushes
  to `main`. Full CI can also be requested manually or through a `full-ci` branch.
- `.github/workflows/sdk.yml` contains SDK checks, but is a reusable workflow
  without a caller in the current repository workflows and expects custom runners.
- `codex-rs/core/tests/suite/oss_turn_compatibility.rs` already exercises an Ollama
  tool turn with simulated model responses. It does not load a real Qwen model.
- `scripts/quality/aren-live-quality.sh` is an existing manual Chrome/research
  test. It is not a model-package compatibility harness.

The first CI improvement should run relevant existing Rust regression tests
before merging, alongside the new package-handling tests. Reuse `just test` and
the repository's Nextest configuration. Select affected consumers as well as the
directly changed crate. Do not add a second set of hypothetical Python SDK tests;
use the SDK's existing `pyproject.toml`, lockfile, and test suite when SDK changes
are in scope.

## Model-package checks without model weights

Hugging Face is the artifact source. Compatibility also depends on the selected
GGUF file, architecture, tokenizer, prompt template, backend version, and tool
protocol. A common download host does not imply a common inference interface.

The initial reference packages are:

| Model identifier | Architecture reported by Hugging Face | Reported user experience |
| --- | --- | --- |
| `hf.co/Bahushruth/Qwen3.6-35B-A3B-abliterated-v4-GGUF:Q4_K_M` | `qwen35moe` | Failed in a released Aren version; exact error and versions still needed |
| `hf.co/BugTraceAI/BugTraceAI-Apex-G4-26B-Q4:latest` | `gemma4` | Worked in the user's environment |

These observations are starting points, not reproduced test results. Record the
actual resolved file and digest for `latest`; do not assume the alias identifies
a fixed quantization or an immutable artifact.

### Capture small, versioned reference inputs

An explicitly invoked capture step should collect only the data needed by tests:

- Requested model identifier, repository revision, resolved file path, file size,
  and publisher-provided artifact digest.
- GGUF version, architecture, quantization information, tensor descriptors, and
  tokenizer metadata relevant to parsing and template selection.
- Embedded chat template and any repository template, system prompt, or parameter
  overrides that affect the import.
- When available from the user's Ollama installation: backend version, installed
  model digest, effective template, parameters, and advertised capabilities.
- For a reproduced failure: the minimal failing request and backend response.
  Remove credentials and unrelated user data before committing a fixture.

Normal PR tests use checked-in fixtures and do not contact Hugging Face. Fixture
capture must record provenance and must never silently refresh expected results.
Keep captured headers separate from distilled fixtures so irrelevant vocabulary
data does not enlarge every test input.

GGUF headers can be inspected using bounded HTTP range requests. The capture
implementation must enforce a byte budget, request-count limit, and timeout,
including redirects and retries. Reject an unexpected full-file response before
consuming the body. If required metadata exceeds the limit, report an incomplete
inspection rather than increasing the limit or falling back to a full download.

A publisher-provided digest identifies the intended weights. Reading a header
does not verify the checksum of the complete weights, tensor contents, or runtime
loadability. Package-check reports must state which evidence was inspected.

### Test behavior derived from package inputs

Tests must exercise Aren's parsing, configuration, and request generation using
the reference inputs. Merely asserting constant metadata values or validating a
fixture against its own schema is insufficient.

| Area | Behavior to verify |
| --- | --- |
| Model identity | Preserve complete `hf.co/...` names and quantization tags through provider selection and request generation |
| File selection | Resolve aliases and quantization choices deterministically; distinguish weights from projector or draft-model files |
| Metadata parsing | Handle supported metadata shapes, missing optional fields, and malformed required fields with clear outcomes |
| Configuration | Derive valid context and reasoning settings from metadata and the configured backend; preserve explicit user overrides where valid |
| Tool requests | Generate the tool representations accepted by the configured API and preserve supported tools |
| Failure reporting | Preserve useful backend errors and stop retries for known permanent request incompatibilities |

Test template selection and rendering in the component that owns them. If Ollama
owns a transformation, record its effective output for regression fixtures;
do not claim an Aren unit test has validated Ollama's entire implementation.
Advertised capabilities and matching architecture names alone do not prove that
a model can execute tools.

Start with the two reference packages and add representative Qwen, Gemma, and
Llama variants as real compatibility cases are captured. Cover useful differences
such as quantization selection, missing metadata, reasoning support, template
overrides, and aliases. Avoid an exhaustive cross-product of model sizes,
quantizations, and backends.

## Heavy tests on `rmi`: manual invocation only

Target access is `ssh tobi@rmi`. SSH connectivity was observed, but authentication
with the available local key failed. Hardware, backend installation, available
models, and local execution are therefore unverified.

Implement a separate local script for model compatibility. Its invocation must
explicitly select the candidate binary and model list. A help or inspection mode
must not start inference. There is no automatic SSH dispatch from GitHub.

Before inference, the script should:

1. Confirm the binary exists and record its version, SHA-256, and available build
   commit metadata. Test the extracted release candidate when evaluating a release.
2. Confirm the requested backend is reachable and record its version.
3. Resolve the selected installed models and record their digests, effective
   templates, capabilities, quantization, and relevant runtime settings.
4. Fail clearly if a requested model is absent. Model installation or pulling is
   a separate deliberate action, not a fallback inside the test script.
5. Create an isolated test workspace and explicitly controlled configuration.
   Record the tested entrypoint so an exec-only result is not presented as proof
   of an interactive-search or TUI path.

Use a small suite with measurable outcomes:

| Case | Required evidence |
| --- | --- |
| Basic response | A completed turn and nonempty final answer within the deadline |
| File read | An actual tool call reads a test file containing a fresh marker; the marker reaches the final answer |
| File change | An actual tool call makes the requested change inside the test workspace; the filesystem result is checked |
| Error handling | A controlled tool error is reported and the turn terminates within the budget |

Enforce time, output-size, and model/tool-call limits outside the model. Set a
whole-run deadline as well as per-case deadlines. Limits must include backend
retries and process shutdown. A failure must produce a report even when later
cases cannot run. Never rerun a test until it passes or let the model increase
its own budgets.

A smaller model can be selected for frequent manual runs. It is a separate test
target, not evidence that the original 35B artifact is compatible. The two user
reference models should be available as an explicitly selected comparison suite
once the environment and original failure have been established.

### Local result report

Keep a run directory on `rmi` with a machine-readable summary, stdout/stderr, and
the minimal tool/event trace needed to validate the cases. The summary records:

- Run identifier and start/end time.
- Binary digest, version, and available commit metadata.
- Backend version, model identifiers and digests, effective settings, and tested
  entrypoint.
- Per-case `passed`, `failed`, `timed_out`, or `not_run` status and the reason.
- Overall result and enforced budgets.

Unavailable prerequisites, skipped required cases, and exhausted budgets must
never produce a passing report. Metadata checks should report package inspection
separately from runtime verification.

If a manual model-test report is required for a release decision, check that it
belongs to the exact candidate binary and selected model/backend configuration.
A missing or mismatched report is missing evidence, not a reason to start the
tests automatically or wait indefinitely. Report collection and release gating
still need implementation; the current release workflow does not enforce this.

## Coverage and bounded agent corrections

Use coverage first to expose untested changed branches in package handling,
configuration, and request generation. Do not impose a repository-wide percentage
that encourages low-value tests or repeated workspace runs.

For bug fixes, require a regression test that detects the original behavior and
passes with the fix. Prefer existing integration-test helpers for agent behavior.
Coverage does not replace assertions about tool calls or completed turns.

Allow at most two automatic correction rounds per task after the initial checks.
The orchestration must enforce this limit across the task, not reset it for each
file or test. After the budget is exhausted, report unresolved failures and
coverage gaps for human review. Do not weaken assertions, disable required tests,
or change coverage exclusions to obtain a green result.

## Implementation order

1. Capture small reference inputs from the Qwen and BugTraceAI packages and obtain
   the original Qwen error, Aren version, and backend version.
2. Add behavioral regression tests in the components that already own the relevant
   parsing and transformations; run them in pre-merge CI without model weights.
3. Implement and validate the explicit local script on `rmi`, including its failure
   paths and report format. Validate harness behavior with small simulations before
   a user explicitly starts a heavy run.
4. Add coverage reporting for the relevant changed code and optional consumption
   of manually produced reports in release decisions.

## References

- [Hugging Face GGUF metadata and remote parsing](https://huggingface.co/docs/hub/en/gguf)
- [Hugging Face GGUF import, quantization selection, and templates in Ollama](https://huggingface.co/docs/hub/en/ollama)
- [Qwen reference package](https://huggingface.co/Bahushruth/Qwen3.6-35B-A3B-abliterated-v4-GGUF)
- [BugTraceAI reference package](https://huggingface.co/BugTraceAI/BugTraceAI-Apex-G4-26B-Q4)
