#!/usr/bin/env bash
set -euo pipefail

bundle_dir="${1:?bundle dir is required}"
tool="${RUNNER_TEMP:-/tmp}/appimagetool-x86_64.AppImage"

if [[ ! -x "${tool}" ]]; then
  curl -fsSL \
    "https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage" \
    -o "${tool}"
  chmod +x "${tool}"
fi

find "${bundle_dir}" -maxdepth 1 -type f -name '*.AppImage' -delete

while IFS= read -r -d '' appdir; do
  desktop_file="$(find "${appdir}/usr/share/applications" "${appdir}" -type f -name '*.desktop' -print -quit)"
  binary_path="$(find "${appdir}/usr/bin" -maxdepth 1 -type f -perm -111 -print -quit)"

  if [[ -z "${desktop_file}" || -z "${binary_path}" ]]; then
    echo "Could not patch ${appdir}: missing desktop file or executable."
    exit 1
  fi

  binary_name="$(basename "${binary_path}")"
  sed -i -E \
    "s|^Exec=.*$|Exec=env WEBKIT_DISABLE_DMABUF_RENDERER=1 WEBKIT_DISABLE_COMPOSITING_MODE=1 GDK_BACKEND=x11 LIBGL_ALWAYS_SOFTWARE=1 ${binary_name}|" \
    "${desktop_file}"

  icon_name="$(awk -F= '/^Icon=/{print $2; exit}' "${desktop_file}")"
  if [[ -n "${icon_name}" && "${icon_name}" != /* && ! -e "${appdir}/${icon_name}.png" ]]; then
    icon_source="$(find "${appdir}" -maxdepth 1 -type f -name '*.png' -print -quit)"
    if [[ -n "${icon_source}" ]]; then
      cp "${icon_source}" "${appdir}/${icon_name}.png"
    fi
  fi

  ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "${tool}" "${appdir}" "${appdir%.AppDir}.AppImage"
done < <(find "${bundle_dir}" -maxdepth 1 -type d -name '*.AppDir' -print0)
