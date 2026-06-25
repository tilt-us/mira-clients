#!/usr/bin/env bash
set -euo pipefail

bundle_dir="${1:?bundle dir is required}"
tool="${RUNNER_TEMP:-/tmp}/appimagetool-x86_64.AppImage"

mapfile -d '' appdirs < <(find "${bundle_dir}" -maxdepth 1 -type d -name '*.AppDir' -print0)
if [[ "${#appdirs[@]}" -eq 0 ]]; then
  echo "No AppDir found in ${bundle_dir}; skipping AppImage environment patch."
  exit 0
fi

if [[ ! -x "${tool}" ]]; then
  curl -fsSL \
    "https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage" \
    -o "${tool}"
  chmod +x "${tool}"
fi

for appdir in "${appdirs[@]}"; do
  desktop_file="$(find "${appdir}/usr/share/applications" "${appdir}" -type f -name '*.desktop' -print -quit)"
  binary_path="$(find "${appdir}/usr/bin" \
    -maxdepth 1 \
    -type f \
    -perm -111 \
    ! -name 'xdg-open' \
    ! -name '*-appimage-env' \
    -print \
    -quit)"

  if [[ -z "${desktop_file}" || -z "${binary_path}" ]]; then
    echo "Could not patch ${appdir}: missing desktop file or executable."
    exit 1
  fi

  binary_name="$(basename "${binary_path}")"
  wrapper_name="${binary_name}-appimage-env"
  wrapper_path="${appdir}/usr/bin/${wrapper_name}"

  cat > "${wrapper_path}" <<EOF
#!/bin/sh
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export GDK_BACKEND=x11
export LIBGL_ALWAYS_SOFTWARE=1
exec "\$(dirname "\$0")/${binary_name}" "\$@"
EOF
  chmod +x "${wrapper_path}"

  while IFS= read -r -d '' candidate_desktop_file; do
    sed -i -E "s|^Exec=.*$|Exec=${wrapper_name}|" "${candidate_desktop_file}"
  done < <(find "${appdir}/usr/share/applications" "${appdir}" -type f -name '*.desktop' -print0)

  icon_name="$(awk -F= '/^Icon=/{print $2; exit}' "${desktop_file}")"
  if [[ -n "${icon_name}" && "${icon_name}" != /* && ! -e "${appdir}/${icon_name}.png" ]]; then
    icon_source="$(find "${appdir}" -maxdepth 1 -type f -name '*.png' -print -quit)"
    if [[ -n "${icon_source}" ]]; then
      cp "${icon_source}" "${appdir}/${icon_name}.png"
    fi
  fi

  rm -f "${appdir%.AppDir}.AppImage"
  ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "${tool}" "${appdir}" "${appdir%.AppDir}.AppImage"
done
