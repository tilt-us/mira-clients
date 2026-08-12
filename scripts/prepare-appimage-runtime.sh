#!/usr/bin/env bash
set -euo pipefail

readonly runtime_url="https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64"
readonly runtime_sha256="2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d"
runtime_path="${MIRA_APPIMAGE_RUNTIME_PATH:-${HOME}/.cache/mira-appimage/runtime-x86_64}"

mkdir -p "$(dirname "${runtime_path}")"

if [[ -f "${runtime_path}" ]] && printf '%s  %s\n' "${runtime_sha256}" "${runtime_path}" | sha256sum --check --status; then
  echo "Using cached AppImage runtime: ${runtime_path}"
else
  temporary_runtime="$(mktemp "${runtime_path}.XXXXXX")"
  trap 'rm -f "${temporary_runtime}"' EXIT

  curl --fail --location --retry 8 --retry-all-errors --retry-delay 3 \
    --output "${temporary_runtime}" \
    "${runtime_url}"
  printf '%s  %s\n' "${runtime_sha256}" "${temporary_runtime}" | sha256sum --check
  chmod 755 "${temporary_runtime}"
  mv "${temporary_runtime}" "${runtime_path}"
fi

: "${GITHUB_ENV:?GITHUB_ENV must be set to export LDAI_RUNTIME_FILE}"
printf 'LDAI_RUNTIME_FILE=%s\n' "${runtime_path}" >> "${GITHUB_ENV}"
