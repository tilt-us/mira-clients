#!/usr/bin/env bash
set -euo pipefail

install_dir="/Applications"
allow_unsigned="false"
dmgs=()

usage() {
  cat <<'USAGE'
Usage:
  ./install-macos.sh [--allow-unsigned] [--install-dir /Applications] <dmg>...

Installs Mira .app bundles from one or more DMG files.

Examples:
  ./install-macos.sh mira-installer.dmg mira-client.dmg
  ./install-macos.sh --allow-unsigned *.dmg

Options:
  --allow-unsigned        Remove quarantine after copying. This does not sign or
                          notarize the app; it only makes unsigned test builds
                          easier to open.
  --install-dir <path>    Destination for .app bundles. Default: /Applications.
USAGE
}

while (($# > 0)); do
  case "$1" in
    --allow-unsigned)
      allow_unsigned="true"
      shift
      ;;
    --install-dir)
      install_dir="${2:-}"
      if [[ -z "${install_dir}" ]]; then
        echo "--install-dir requires a path." >&2
        exit 2
      fi
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    -*)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      dmgs+=("$1")
      shift
      ;;
  esac
done

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This installer script must be run on macOS." >&2
  exit 1
fi

if ((${#dmgs[@]} == 0)); then
  while IFS= read -r -d '' dmg; do
    dmgs+=("$dmg")
  done < <(find . -maxdepth 1 -type f -iname '*.dmg' -print0 | sort -z)
fi

if ((${#dmgs[@]} == 0)); then
  echo "No DMG files provided or found in the current directory." >&2
  usage >&2
  exit 2
fi

copy_app() {
  local source_app="$1"
  local target_app="${install_dir}/$(basename "${source_app}")"

  echo "Installing $(basename "${source_app}") -> ${target_app}"
  if [[ -w "${install_dir}" ]]; then
    rm -rf "${target_app}"
    ditto "${source_app}" "${target_app}"
  else
    sudo rm -rf "${target_app}"
    sudo ditto "${source_app}" "${target_app}"
  fi

  if [[ "${allow_unsigned}" == "true" ]]; then
    xattr -dr com.apple.quarantine "${target_app}" 2>/dev/null || true
  fi
}

for dmg in "${dmgs[@]}"; do
  if [[ ! -f "${dmg}" ]]; then
    echo "DMG not found: ${dmg}" >&2
    exit 1
  fi

  mount_dir="$(mktemp -d "${TMPDIR:-/tmp}/mira-dmg.XXXXXX")"
  attached="false"

  cleanup() {
    if [[ "${attached}" == "true" ]]; then
      hdiutil detach "${mount_dir}" -quiet || true
    fi
    rmdir "${mount_dir}" 2>/dev/null || true
  }

  trap cleanup EXIT

  echo "Mounting ${dmg}"
  hdiutil attach "${dmg}" -nobrowse -readonly -mountpoint "${mount_dir}" -quiet
  attached="true"

  app_count=0
  while IFS= read -r -d '' app; do
    copy_app "${app}"
    app_count=$((app_count + 1))
  done < <(find "${mount_dir}" -maxdepth 2 -type d -name '*.app' -print0)

  if ((app_count == 0)); then
    echo "No .app bundle found in ${dmg}" >&2
    exit 1
  fi

  cleanup
  trap - EXIT
done

echo "Done."
