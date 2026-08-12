#!/usr/bin/env bash
set -euo pipefail

readonly app_run_url="https://github.com/tauri-apps/binary-releases/releases/download/apprun-old/AppRun-x86_64"
readonly app_run_sha256="f30140a43a0a59e46db21bdefdf749b9e9f2c6946e92afabbacf98b8ae73fb4f"
readonly runtime_url="https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64"
readonly runtime_sha256="2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d"

download_verified() {
  local url="$1"
  local expected_sha256="$2"
  local destination="$3"

  mkdir -p "$(dirname "${destination}")"
  if [[ -f "${destination}" ]] && printf '%s  %s\n' "${expected_sha256}" "${destination}" | sha256sum --check --status; then
    echo "Using cached $(basename "${destination}"): ${destination}"
    return
  fi

  local temporary_file
  temporary_file="$(mktemp "${destination}.XXXXXX")"
  curl --fail --location --retry 8 --retry-all-errors --retry-delay 3 \
    --output "${temporary_file}" \
    "${url}"
  printf '%s  %s\n' "${expected_sha256}" "${temporary_file}" | sha256sum --check
  chmod 755 "${temporary_file}"
  mv "${temporary_file}" "${destination}"
}

tauri_tools_path="${HOME}/.cache/tauri"
app_run_path="${tauri_tools_path}/AppRun-x86_64"
runtime_path="${MIRA_APPIMAGE_RUNTIME_PATH:-${HOME}/.cache/mira-appimage/runtime-x86_64}"

# tauri-bundler 2.9 downloads this before it invokes linuxdeploy. Supplying it
# here prevents a transient GitHub disconnect from failing both Linux bundles.
download_verified "${app_run_url}" "${app_run_sha256}" "${app_run_path}"
download_verified "${runtime_url}" "${runtime_sha256}" "${runtime_path}"

: "${GITHUB_ENV:?GITHUB_ENV must be set to export LDAI_RUNTIME_FILE}"
printf 'LDAI_RUNTIME_FILE=%s\n' "${runtime_path}" >> "${GITHUB_ENV}"
