import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core";

declare const __MIRA_UI_ASSET_DEV_ROOT__: string;

let uiAssetRoot: string | undefined = __MIRA_UI_ASSET_DEV_ROOT__;

/** Initializes external UI asset resolution before React renders. */
export async function initializeUiAssets() {
  if (!isTauri()) {
    return true;
  }

  try {
    uiAssetRoot = await invoke<string>("ui_asset_root_path");
    return true;
  } catch (error) {
    uiAssetRoot = undefined;
    console.error("[mira-client] External UI assets are unavailable", error);
    return false;
  }
}

/** Returns an external UI asset URL without making Vite bundle the file. */
export function uiAssetUrl(relativePath: string) {
  const normalized = relativePath.replace(/\\/g, "/").replace(/^\/+/, "");
  if (normalized.split("/").includes("..")) {
    throw new Error(`Invalid UI asset path: ${relativePath}`);
  }
  if (!uiAssetRoot) {
    return "";
  }
  const path = `${uiAssetRoot.replace(/[\\/]$/, "")}/${normalized}`;
  return isTauri() ? convertFileSrc(path) : `${path}`;
}

export function uiCharacterUrl(name: string) {
  return uiAssetUrl(`characters/${name}.png`);
}

export function uiWallpaperUrl(name: string) {
  return uiAssetUrl(`wallpapers/${name}-wallpaper.png`);
}
