import { useCallback, useEffect, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  currentMonitor,
  getCurrentWindow,
  LogicalSize,
  primaryMonitor,
} from "@tauri-apps/api/window";
import { translate } from "./i18n";
import type { AppLocale } from "./i18n";
import ignaraWallpaper from "../../../assets/wallpapers/ignara-wallpaper.png";
import liraWallpaper from "../../../assets/wallpapers/lira-wallpaper.png";
import sophiaWallpaper from "../../../assets/wallpapers/sophia-wallpaper.png";
import yunaWallpaper from "../../../assets/wallpapers/yuna-wallpaper.png";
import {
  defaultAccentColor,
  defaultBackgroundChampion,
  defaultClientAnimation,
  defaultFriendRequestPolicy,
  defaultGameScreenMode,
  defaultResolution,
  defaultUiScale,
  getResolutionFromSize,
  getResolutionSize,
  isAppResolution,
  isBackgroundChampion,
  isClientAnimation,
  isFriendRequestPolicy,
  isGameScreenMode,
  isHexColor,
  isLocale,
  isUiScale,
  readStoredSettings,
  type AppResolution,
  type BackgroundChampion,
  type ClientAnimation,
  type FriendRequestPolicy,
  type GameScreenMode,
  type UiScale,
  writeStoredSettings,
} from "./settings";

const twoKResolutionMinimum = {
  height: 1080,
  width: 1920,
};

const fourKResolutionMinimum = {
  height: 2160,
  width: 3840,
};

export function useClientSettings() {
  const [accentColor, setAccentColor] = useState(() => {
    const storedSettings = readStoredSettings();
    return isHexColor(storedSettings.accentColor)
      ? storedSettings.accentColor
      : defaultAccentColor;
  });
  const [backgroundChampion, setBackgroundChampion] = useState<BackgroundChampion>(() => {
    const storedSettings = readStoredSettings();
    return isBackgroundChampion(storedSettings.backgroundChampion)
      ? storedSettings.backgroundChampion
      : defaultBackgroundChampion;
  });
  const [locale, setLocale] = useState<AppLocale>(() => {
    const storedSettings = readStoredSettings();
    return isLocale(storedSettings.locale) ? storedSettings.locale : "de";
  });
  const [resolution, setResolution] = useState<AppResolution>(() => {
    const storedSettings = readStoredSettings();
    return isAppResolution(storedSettings.resolution)
      ? storedSettings.resolution
      : defaultResolution;
  });
  const [friendRequestPolicy, setFriendRequestPolicy] = useState<FriendRequestPolicy>(() => {
    const storedSettings = readStoredSettings();
    if (isFriendRequestPolicy(storedSettings.friendRequestPolicy)) {
      return storedSettings.friendRequestPolicy;
    }

    if (typeof storedSettings.allowFriendRequests === "boolean") {
      return storedSettings.allowFriendRequests ? "allow" : "disallow";
    }

    return defaultFriendRequestPolicy;
  });
  const [clientAnimation, setClientAnimation] = useState<ClientAnimation>(() => {
    const storedSettings = readStoredSettings();
    return isClientAnimation(storedSettings.clientAnimation)
      ? storedSettings.clientAnimation
      : defaultClientAnimation;
  });
  const [uiScale, setUiScale] = useState<UiScale>(() => {
    const storedSettings = readStoredSettings();
    return isUiScale(storedSettings.uiScale)
      ? storedSettings.uiScale
      : defaultUiScale;
  });
  const [gameScreenMode, setGameScreenMode] = useState<GameScreenMode>(() => {
    const storedSettings = readStoredSettings();
    return isGameScreenMode(storedSettings.gameScreenMode)
      ? storedSettings.gameScreenMode
      : defaultGameScreenMode;
  });
  const [monitorResolutionSupport, setMonitorResolutionSupport] = useState<
    MonitorResolutionSupport | undefined
  >(() =>
    runsInTauriLikeShell() ? undefined : detectBrowserMonitorResolutionSupport(),
  );
  const t = useCallback((id: string) => translate(locale, id), [locale]);

  useEffect(() => {
    let cancelled = false;

    async function detectMonitor() {
      if (runsInTauriLikeShell()) {
        void getCurrentWindow().setResizable(false);
      }

      const resolutionSupport = runsInTauriLikeShell()
        ? await detectTauriMonitorResolutionSupport()
        : detectBrowserMonitorResolutionSupport();

      if (!cancelled) {
        setMonitorResolutionSupport(resolutionSupport);
      }
    }

    void detectMonitor();

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!monitorResolutionSupport) {
      return;
    }

    if (
      (resolution === "1920x1080" && !monitorResolutionSupport.twoK) ||
      (resolution === "2600x1600" && !monitorResolutionSupport.fourK) ||
      (resolution === "2140x1440" && !monitorResolutionSupport.fourK)
    ) {
      setResolution(defaultResolution);
    }
  }, [monitorResolutionSupport, resolution]);

  useEffect(() => {
    const maxUiScale = getMaxUiScaleForResolution(resolution);

    if (uiScale > maxUiScale) {
      setUiScale(maxUiScale);
    }
  }, [resolution, uiScale]);

  useEffect(() => {
    document.documentElement.style.setProperty("--accent-color", accentColor);
    document.documentElement.style.setProperty(
      "--accent-foreground-color",
      getAccentForegroundColor(accentColor),
    );
    document.documentElement.style.setProperty(
      "--app-background-wallpaper",
      `url(${backgroundChampionWallpapers[backgroundChampion]})`,
    );
    writeStoredSettings({
      accentColor,
      allowFriendRequests: friendRequestPolicy === "allow",
      backgroundChampion,
      clientAnimation,
      friendRequestPolicy,
      gameScreenMode,
      locale,
      resolution,
      uiScale,
    });
  }, [
    accentColor,
    backgroundChampion,
    clientAnimation,
    friendRequestPolicy,
    gameScreenMode,
    locale,
    resolution,
    uiScale,
  ]);

  useEffect(() => {
    void applyUiScale(uiScale);
  }, [uiScale]);

  useEffect(() => {
    if (!runsInTauriLikeShell()) {
      return;
    }

    if (!monitorResolutionSupport) {
      return;
    }

    if (
      (resolution === "1920x1080" && !monitorResolutionSupport.twoK) ||
      (resolution === "2600x1600" && !monitorResolutionSupport.fourK) ||
      (resolution === "2140x1440" && !monitorResolutionSupport.fourK)
    ) {
      return;
    }

    void applyWindowResolution(resolution, setResolution);
  }, [monitorResolutionSupport, resolution]);

  useEffect(() => {
    if (!runsInTauriLikeShell()) {
      return;
    }

    let cancelled = false;
    let unlisten: (() => void) | undefined;

    async function listenForWindowResize() {
      const appWindow = getCurrentWindow();
      const scaleFactor = await appWindow.scaleFactor();

      unlisten = await appWindow.onResized(({ payload: size }) => {
        const currentResolution = getResolutionFromSize(
          size.width / scaleFactor,
          size.height / scaleFactor,
        );

        if (!cancelled && currentResolution) {
          setResolution(currentResolution);
        }
      });
    }

    void listenForWindowResize();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return {
    accentColor,
    backgroundChampion,
    clientAnimation,
    friendRequestPolicy,
    gameScreenMode,
    locale,
    resolution,
    supportsFourKResolution: monitorResolutionSupport?.fourK === true,
    supportsTwoKResolution: monitorResolutionSupport?.twoK === true,
    t,
    uiScale,
    setAccentColor,
    setBackgroundChampion,
    setClientAnimation,
    setFriendRequestPolicy,
    setGameScreenMode,
    setLocale,
    setResolution,
    setUiScale,
  };
}

async function applyWindowResolution(
  resolution: AppResolution,
  _setResolution: (resolution: AppResolution) => void,
) {
  const appWindow = getCurrentWindow();
  const { height, width } = getResolutionSize(resolution);

  if (await appWindow.isMaximized()) {
    await appWindow.unmaximize();
  }

  await appWindow.setSize(new LogicalSize(width, height));
}

async function applyUiScale(uiScale: UiScale) {
  if (runsInTauriLikeShell()) {
    try {
      await getCurrentWebview().setZoom(uiScale);
      document.documentElement.style.removeProperty("zoom");
      return;
    } catch (caughtError) {
      console.error("Webview zoom could not be applied.", caughtError);
    }
  }

  document.documentElement.style.setProperty("zoom", String(uiScale));
}

type MonitorResolutionSupport = {
  fourK: boolean;
  twoK: boolean;
};

const backgroundChampionWallpapers: Record<BackgroundChampion, string> = {
  ignara: ignaraWallpaper,
  lira: liraWallpaper,
  sophia: sophiaWallpaper,
  yuna: yunaWallpaper,
};

async function detectTauriMonitorResolutionSupport() {
  const monitor = (await currentMonitor()) ?? (await primaryMonitor());
  const size = monitor?.size;

  if (!size) {
    return detectBrowserMonitorResolutionSupport();
  }

  return getMonitorResolutionSupport(size.width, size.height);
}

function detectBrowserMonitorResolutionSupport() {
  return getMonitorResolutionSupport(window.screen.width, window.screen.height);
}

function runsInTauriLikeShell() {
  const location = window.location;

  return (
    isTauri() ||
    "isTauri" in globalThis ||
    "__TAURI_INTERNALS__" in globalThis ||
    "__TAURI__" in globalThis ||
    location.protocol === "tauri:" ||
    location.hostname === "tauri.localhost" ||
    location.origin === "null"
  );
}

function getMonitorResolutionSupport(
  width: number,
  height: number,
): MonitorResolutionSupport {
  const longSide = Math.max(width, height);
  const shortSide = Math.min(width, height);

  return {
    fourK:
      longSide >= fourKResolutionMinimum.width &&
      shortSide >= fourKResolutionMinimum.height,
    twoK:
      longSide >= twoKResolutionMinimum.width &&
      shortSide >= twoKResolutionMinimum.height,
  };
}

function getMaxUiScaleForResolution(resolution: AppResolution): UiScale {
  switch (resolution) {
    case "1270x720":
      return 1;
    case "1400x800":
      return 1.1;
    case "1600x900":
      return 1.25;
    case "1920x1080":
    case "2600x1600":
    case "2140x1440":
      return 1.5;
  }
}

function getAccentForegroundColor(hexColor: string) {
  const red = Number.parseInt(hexColor.slice(1, 3), 16) / 255;
  const green = Number.parseInt(hexColor.slice(3, 5), 16) / 255;
  const blue = Number.parseInt(hexColor.slice(5, 7), 16) / 255;

  const toLinear = (value: number) =>
    value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;

  const luminance =
    0.2126 * toLinear(red) + 0.7152 * toLinear(green) + 0.0722 * toLinear(blue);
  const whiteContrast = (1 + 0.05) / (luminance + 0.05);
  const darkContrast = (luminance + 0.05) / 0.055;

  return whiteContrast > darkContrast ? "#ffffff" : "#101216";
}
