#!/usr/bin/env bash

set -euo pipefail

mode="${1:-quick}"
aren_bin="${AREN_BIN:-aren}"
timeout_seconds="${AREN_LIVE_TEST_TIMEOUT:-240}"
state_root="${AREN_LIVE_TEST_STATE_DIR:-${XDG_STATE_HOME:-${HOME}/.local/state}/aren/live-quality}"
run_id="$(date -u +%Y%m%dT%H%M%SZ)"
run_dir="${state_root}/${run_id}"

case "${mode}" in
  quick)
    queries=(
      "Öffne mit Chrome die offizielle IANA-Seite zu reservierten Beispieldomains unter https://www.iana.org/help/example-domains, nenne ihren Seitentitel und gib die geprüfte URL als Quelle an."
    )
    ;;
  full)
    queries=(
      "Öffne mit Chrome die offizielle IANA-Seite zu reservierten Beispieldomains unter https://www.iana.org/help/example-domains, nenne ihren Seitentitel und gib die geprüfte URL als Quelle an."
      "Recherchiere mit Chrome auf https://www.timeanddate.com/worldclock/germany/berlin das heutige Datum in Deutschland sowie die aktuelle UTC-Abweichung für Berlin und nenne die geprüfte URL als Quelle."
      "Recherchiere mit Chrome die aktuelle stabile Rust-Version. Gleiche Version und Veröffentlichungsdatum mit https://blog.rust-lang.org/releases/latest/ ab und nenne die geprüfte URL als Quelle."
    )
    ;;
  *)
    echo "Usage: $0 [quick|full]" >&2
    exit 2
    ;;
esac

command -v "${aren_bin}" >/dev/null 2>&1 || {
  echo "Aren binary not found: ${aren_bin}" >&2
  exit 1
}
command -v python3 >/dev/null 2>&1 || {
  echo "python3 is required to validate JSON output." >&2
  exit 1
}

mkdir -p "${run_dir}"
"${aren_bin}" --version | tee "${run_dir}/version.txt"
"${aren_bin}" interactive-search --help > "${run_dir}/interactive-search-help.txt"
"${aren_bin}" mcp list > "${run_dir}/mcp-list.txt"

if ! grep -Eq 'chrome[-_]devtools' "${run_dir}/mcp-list.txt"; then
  echo "Chrome DevTools MCP is not configured; refusing to report a valid live-quality run." >&2
  exit 1
fi

for index in "${!queries[@]}"; do
  case_number="$((index + 1))"
  output_path="${run_dir}/case-${case_number}.json"
  error_path="${run_dir}/case-${case_number}.stderr.log"

  "${aren_bin}" interactive-search \
    --json \
    --timeout "${timeout_seconds}" \
    "${queries[${index}]}" \
    > "${output_path}" \
    2> "${error_path}"

  python3 - "${output_path}" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
payload = json.loads(path.read_text(encoding="utf-8"))
answer = payload.get("answer")
sources = payload.get("sources")
if not isinstance(answer, str) or len(answer.strip()) < 40:
    raise SystemExit(f"{path}: answer is missing or too short")
if not isinstance(sources, list) or not sources:
    raise SystemExit(f"{path}: no sources were returned")
if not all(isinstance(source, str) and source.startswith(("http://", "https://")) for source in sources):
    raise SystemExit(f"{path}: malformed source URL")
PY
done

printf '%s\n' \
  "status=passed" \
  "mode=${mode}" \
  "cases=${#queries[@]}" \
  "completed_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  > "${run_dir}/RESULT.txt"

echo "Aren live-quality ${mode} passed: ${run_dir}"
