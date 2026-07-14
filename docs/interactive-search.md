# Headless interactive search

`codex interactive-search "<PROMPT>"` runs a single-turn interactive session without the TUI. It reuses the interactive agent configuration and enables the built-in `web_search` tool, then exits after the final assistant response.

## Usage

```bash
codex interactive-search "What is the current price of XAUUSD?"
```

### Options

- `--json`: emit a single JSON object with `answer`, `sources`, `timestamp`, and `model`.
- `--timeout <seconds>`: abort the session if it exceeds the time limit.

## Notes

- If the working directory is not trusted yet, run the interactive CLI once to approve it.
- Approval prompts for commands or patches are denied automatically in headless mode.
