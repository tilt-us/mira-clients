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
  defaultChatPosition,
  defaultClientAnimation,
  defaultFriendRequestPolicy,
  defaultGameScreenMode,
  defaultResolution,
  defaultUiScale,
  getAccentForegroundColor,
  getResolutionFromSize,
  getResolutionSize,
  isAppResolution,
  isBackgroundChampion,
  isChatPosition,
  isClientAnimation,
  isFriendRequestPolicy,
  isGameScreenMode,
  isHexColor,
  isLocale,
  isUiScale,
  readStoredSettings,
  type AppResolution,
  type BackgroundChampion,
  type ChatPosition,
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
  const [chatPosition, setChatPosition] = useState<ChatPosition>(() => {
    const storedSettings = readStoredSettings();
    return isChatPosition(storedSettings.chatPosition)
      ? storedSettings.chatPosition
      : defaultChatPosition;
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

    if (!isResolutionSupportedByMonitor(resolution, monitorResolutionSupport)) {
      setResolution(getMinimumResolutionForMonitor(monitorResolutionSupport));
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
      chatPosition,
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
    chatPosition,
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

    if (!isResolutionSupportedByMonitor(resolution, monitorResolutionSupport)) {
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
    chatPosition,
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
    setChatPosition,
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
  document.documentElement.style.removeProperty("zoom");
  document.documentElement.style.setProperty("font-size", `${16 * uiScale}px`);

  if (runsInTauriLikeShell()) {
    try {
      await getCurrentWebview().setZoom(1);
    } catch (caughtError) {
      console.error("Webview zoom could not be applied.", caughtError);
    }
  }
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
    case "2140x1080":
      return 1.5;
  }
}

function getMinimumResolutionForMonitor(
  monitorResolutionSupport: MonitorResolutionSupport,
): AppResolution {
  if (monitorResolutionSupport.fourK) {
    return "1600x900";
  }

  if (monitorResolutionSupport.twoK) {
    return "1400x800";
  }

  return "1270x720";
}

function isResolutionSupportedByMonitor(
  resolution: AppResolution,
  monitorResolutionSupport: MonitorResolutionSupport,
) {
  if (
    getResolutionRank(resolution) <
    getResolutionRank(getMinimumResolutionForMonitor(monitorResolutionSupport))
  ) {
    return false;
  }

  if (resolution === "1920x1080" && !monitorResolutionSupport.twoK) {
    return false;
  }

  if (resolution === "2140x1080" && !monitorResolutionSupport.fourK) {
    return false;
  }

  return true;
}

function getResolutionRank(resolution: AppResolution) {
  switch (resolution) {
    case "1270x720":
      return 0;
    case "1400x800":
      return 1;
    case "1600x900":
      return 2;
    case "1920x1080":
      return 3;
    case "2140x1080":
      return 4;
  }
}
