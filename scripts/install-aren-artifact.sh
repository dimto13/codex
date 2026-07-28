#!/usr/bin/env bash

set -euo pipefail

if (($# != 2)); then
  echo "Usage: $0 ARCHIVE CHECKSUM" >&2
  exit 2
fi

archive_path="$(realpath "$1")"
checksum_path="$(realpath "$2")"
install_root="${AREN_INSTALL_ROOT:-${HOME}/.local/lib/aren}"
bin_dir="${AREN_BIN_DIR:-${HOME}/.local/bin}"

case "$(uname -s):$(uname -m)" in
  Linux:x86_64)
    expected_archive="aren-linux-x86_64.tar.gz"
    ;;
  Linux:aarch64|Linux:arm64)
    expected_archive="aren-linux-aarch64.tar.gz"
    ;;
  *)
    echo "Aren artifacts currently support Linux x86_64 and Linux ARM64." >&2
    exit 1
    ;;
esac
[[ "$(basename "${archive_path}")" == "${expected_archive}" ]] || {
  echo "Expected ${expected_archive} on this platform." >&2
  exit 1
}
[[ -s "${archive_path}" ]] || {
  echo "Archive is missing or empty: ${archive_path}" >&2
  exit 1
}
[[ -s "${checksum_path}" ]] || {
  echo "Checksum is missing or empty: ${checksum_path}" >&2
  exit 1
}

temporary_dir="$(mktemp -d)"
trap 'rm -rf "${temporary_dir}"' EXIT

cp "${archive_path}" "${temporary_dir}/$(basename "${archive_path}")"
cp "${checksum_path}" "${temporary_dir}/$(basename "${checksum_path}")"
(
  cd "${temporary_dir}"
  sha256sum --check "$(basename "${checksum_path}")"
)
tar -C "${temporary_dir}" -xzf "${temporary_dir}/$(basename "${archive_path}")"

[[ -x "${temporary_dir}/aren" ]] || {
  echo "Archive does not contain an executable Aren binary." >&2
  exit 1
}
[[ -x "${temporary_dir}/aren-update" ]] || {
  echo "Archive does not contain the Aren updater." >&2
  exit 1
}
[[ -s "${temporary_dir}/BUILD-INFO.txt" ]] || {
  echo "Archive does not contain BUILD-INFO.txt." >&2
  exit 1
}

release_name="$(
  sed -n 's/^release=//p' "${temporary_dir}/BUILD-INFO.txt" | head -n 1
)"
[[ "${release_name}" =~ ^[A-Za-z0-9._-]+$ ]] || {
  echo "Invalid release name in BUILD-INFO.txt." >&2
  exit 1
}

version_dir="${install_root}/${release_name}"
mkdir -p "${version_dir}" "${bin_dir}"
install -m 0755 "${temporary_dir}/aren" "${version_dir}/aren"
install -m 0755 "${temporary_dir}/aren-update" "${version_dir}/aren-update"
install -m 0644 "${temporary_dir}/BUILD-INFO.txt" "${version_dir}/BUILD-INFO.txt"
"${version_dir}/aren" --version
"${version_dir}/aren" interactive-search --help >/dev/null

link_path="${bin_dir}/aren"
temporary_link="${bin_dir}/.aren-link.$$"
ln -s "${version_dir}/aren" "${temporary_link}"
mv -Tf "${temporary_link}" "${link_path}"
install -m 0755 "${version_dir}/aren-update" "${bin_dir}/aren-update"

echo "Installed ${release_name} at ${version_dir}/aren"
echo "Activated ${link_path} -> ${version_dir}/aren"
