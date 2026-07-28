#!/usr/bin/env bash

set -euo pipefail

repository="${AREN_GITHUB_REPOSITORY:-dimto13/codex}"
release_tag="${AREN_RELEASE_TAG:-}"
install_dir="${AREN_INSTALL_DIR:-${HOME}/.local/bin}"

usage() {
  cat <<'EOF'
Usage: aren-update [--tag TAG] [--repo OWNER/REPO] [--install-dir DIR]

Install an Aren release without modifying the official `codex` command.

Options:
  --tag TAG          Release tag to install (default: latest stable release)
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

case "$(uname -s):$(uname -m)" in
  Linux:x86_64)
    release_platform="linux-x86_64"
    ;;
  Linux:aarch64|Linux:arm64)
    release_platform="linux-aarch64"
    ;;
  *)
    echo "Aren releases currently support Linux x86_64 and Linux ARM64." >&2
    exit 1
    ;;
esac

temporary_dir="$(mktemp -d)"
trap 'rm -rf "${temporary_dir}"' EXIT

resolve_latest_release_tag() {
  if command -v gh >/dev/null 2>&1 \
    && { [[ -n "${GH_TOKEN:-}" ]] || [[ -n "${GITHUB_TOKEN_CLASSIC:-}" ]] || gh auth status >/dev/null 2>&1; }; then
    if [[ -z "${GH_TOKEN:-}" && -n "${GITHUB_TOKEN_CLASSIC:-}" ]]; then
      export GH_TOKEN="${GITHUB_TOKEN_CLASSIC}"
    fi
    gh release view \
      --repo "${repository}" \
      --json tagName \
      --jq .tagName
    return
  fi

  curl --fail --silent --show-error --location \
    "https://api.github.com/repos/${repository}/releases/latest" \
    | sed -n 's/^[[:space:]]*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' \
    | head -n 1
}

if [[ -z "${release_tag}" ]]; then
  release_tag="$(resolve_latest_release_tag)"
fi
[[ "${release_tag}" =~ ^aren-v[0-9]+\.[0-9]+\.[0-9]+([.-][A-Za-z0-9.-]+)?$ ]] || {
  echo "Invalid or unavailable Aren release tag: ${release_tag:-<empty>}" >&2
  exit 1
}

archive_name="aren-${release_platform}.tar.gz"
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
[[ -x "${temporary_dir}/aren-update" ]] || { echo "Release does not contain the Aren updater." >&2; exit 1; }
expected_version="${release_tag#aren-v}"
[[ "$("${temporary_dir}/aren" --version)" == "aren ${expected_version}" ]] || {
  echo "Downloaded Aren binary does not match ${release_tag}." >&2
  exit 1
}

mkdir -p "${install_dir}"
temporary_target="${install_dir}/.aren.$$.new"
temporary_updater="${install_dir}/.aren-update.$$.new"
trap 'rm -rf "${temporary_dir}"; rm -f "${temporary_target:-}" "${temporary_updater:-}"' EXIT
install -m 0755 "${temporary_dir}/aren" "${temporary_target}"
install -m 0755 "${temporary_dir}/aren-update" "${temporary_updater}"
mv -f "${temporary_target}" "${install_dir}/aren"
mv -f "${temporary_updater}" "${install_dir}/aren-update"

echo "Aren ${release_tag} installed at ${install_dir}/aren"
