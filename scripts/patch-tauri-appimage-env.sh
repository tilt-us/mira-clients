#!/usr/bin/env bash
set -euo pipefail

bundle_dir="${1:?bundle dir is required}"
tool="${RUNNER_TEMP:-/tmp}/appimagetool-x86_64.AppImage"

mapfile -d '' appdirs < <(find "${bundle_dir}" -maxdepth 1 -type d -name '*.AppDir' -print0)
if [[ "${#appdirs[@]}" -eq 0 ]]; then
  echo "No AppDir found in ${bundle_dir}; skipping AppImage environment patch."
  exit 0
fi

find "${bundle_dir}" -maxdepth 1 -type f -name '*.AppImage' -delete

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
  plugin_staging_dir="$(mktemp -d)"
  trap 'rm -rf "${plugin_staging_dir}"' EXIT

  plugins_dir="$(pkg-config --variable=pluginsdir gstreamer-1.0 2>/dev/null || true)"
  scanner_dir="$(pkg-config --variable=pluginscannerdir gstreamer-1.0 2>/dev/null || true)"
  gst_app_plugin=""
  gst_plugin_scanner=""
  if [[ -n "${plugins_dir}" && -f "${plugins_dir}/libgstapp.so" ]]; then
    gst_app_plugin="${plugins_dir}/libgstapp.so"
  else
    while IFS= read -r candidate; do
      gst_app_plugin="${candidate}"
      break
    done < <(find /usr/lib /usr/lib64 -path '*/gstreamer-1.0/libgstapp.so' -type f 2>/dev/null)
  fi

  if [[ -n "${scanner_dir}" && -x "${scanner_dir}/gst-plugin-scanner" ]]; then
    gst_plugin_scanner="${scanner_dir}/gst-plugin-scanner"
  else
    while IFS= read -r candidate; do
      gst_plugin_scanner="${candidate}"
      break
    done < <(find /usr/lib /usr/lib64 -path '*/gstreamer-1.0/gst-plugin-scanner' -type f -perm -111 2>/dev/null)
  fi

  if [[ -z "${gst_app_plugin}" ]]; then
    echo "Could not find GStreamer appsink plugin libgstapp.so."
    exit 1
  fi
  if [[ -z "${gst_plugin_scanner}" ]]; then
    echo "Could not find GStreamer plugin scanner."
    exit 1
  fi

  cp "${gst_app_plugin}" "${plugin_staging_dir}/libgstapp.so"
  cp "${gst_plugin_scanner}" "${plugin_staging_dir}/gst-plugin-scanner"

  # Keep the AppImage on the host WebKitGTK/GTK graphics stack. Bundled copies
  # can be stable but noticeably slower on some Linux desktops.
  rm -rf "${appdir}/usr/lib" "${appdir}/usr/lib64"
  mkdir -p "${appdir}/usr/lib/gstreamer-1.0"
  cp "${plugin_staging_dir}/libgstapp.so" "${appdir}/usr/lib/gstreamer-1.0/libgstapp.so"
  cp "${plugin_staging_dir}/gst-plugin-scanner" "${appdir}/usr/lib/gstreamer-1.0/gst-plugin-scanner"

cat > "${wrapper_path}" <<EOF
#!/bin/sh
script_dir="\$(dirname "\$0")"
unset WEBKIT_DISABLE_DMABUF_RENDERER
unset WEBKIT_DISABLE_COMPOSITING_MODE
unset GDK_BACKEND
unset LIBGL_ALWAYS_SOFTWARE
export GST_PLUGIN_PATH="\${script_dir}/../lib/gstreamer-1.0\${GST_PLUGIN_PATH:+:\${GST_PLUGIN_PATH}}"
export GST_PLUGIN_SCANNER="\${script_dir}/../lib/gstreamer-1.0/gst-plugin-scanner"
exec "\${script_dir}/${binary_name}" "\$@"
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
