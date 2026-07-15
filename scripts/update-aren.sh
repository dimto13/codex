#!/usr/bin/env bash

set -euo pipefail

repository="${AREN_GITHUB_REPOSITORY:-dimto13/codex}"
release_tag="${AREN_RELEASE_TAG:-aren-edge}"
install_dir="${AREN_INSTALL_DIR:-${HOME}/.local/bin}"
archive_name="aren-linux-x86_64.tar.gz"

usage() {
  cat <<'EOF'
Usage: aren-update [--tag TAG] [--repo OWNER/REPO] [--install-dir DIR]

Install an Aren release without modifying the official `codex` command.

Options:
  --tag TAG          Release tag to install (default: aren-edge)
  --repo OWNER/REPO  GitHub repository (default: dimto13/codex)
  --install-dir DIR  Installation directory (default: ~/.local/bin)
  -h, --help         Show this help
EOF
}

while (($# > 0)); do
  case "$1" in
    --tag)
      [[ $# -ge 2 ]] || { echo "Missing value for --tag" >&2; exit 2; }
      release_tag="$2"
      shift 2
      ;;
    --repo)
      [[ $# -ge 2 ]] || { echo "Missing value for --repo" >&2; exit 2; }
      repository="$2"
      shift 2
      ;;
    --install-dir)
      [[ $# -ge 2 ]] || { echo "Missing value for --install-dir" >&2; exit 2; }
      install_dir="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ "$(uname -s)" != "Linux" || "$(uname -m)" != "x86_64" ]]; then
  echo "Aren releases currently support Linux x86_64 only." >&2
  exit 1
fi

temporary_dir="$(mktemp -d)"
trap 'rm -rf "${temporary_dir}"' EXIT

checksum_name="${archive_name}.sha256"
release_url="https://github.com/${repository}/releases/download/${release_tag}"

download_with_curl() {
  curl --fail --silent --show-error --location \
    "${release_url}/${archive_name}" \
    --output "${temporary_dir}/${archive_name}"
  curl --fail --silent --show-error --location \
    "${release_url}/${checksum_name}" \
    --output "${temporary_dir}/${checksum_name}"
}

if command -v gh >/dev/null 2>&1 \
  && { [[ -n "${GH_TOKEN:-}" ]] || [[ -n "${GITHUB_TOKEN_CLASSIC:-}" ]] || gh auth status >/dev/null 2>&1; }; then
  if [[ -z "${GH_TOKEN:-}" && -n "${GITHUB_TOKEN_CLASSIC:-}" ]]; then
    export GH_TOKEN="${GITHUB_TOKEN_CLASSIC}"
  fi
  gh release download "${release_tag}" \
    --repo "${repository}" \
    --pattern "${archive_name}" \
    --pattern "${checksum_name}" \
    --dir "${temporary_dir}"
else
  download_with_curl
fi

(cd "${temporary_dir}" && sha256sum --check "${checksum_name}")
tar -C "${temporary_dir}" -xzf "${temporary_dir}/${archive_name}"
[[ -x "${temporary_dir}/aren" ]] || { echo "Release does not contain an executable Aren binary." >&2; exit 1; }
"${temporary_dir}/aren" --version

mkdir -p "${install_dir}"
temporary_target="${install_dir}/.aren.$$.new"
trap 'rm -rf "${temporary_dir}"; rm -f "${temporary_target:-}"' EXIT
install -m 0755 "${temporary_dir}/aren" "${temporary_target}"
mv -f "${temporary_target}" "${install_dir}/aren"

echo "Aren ${release_tag} installed at ${install_dir}/aren"
