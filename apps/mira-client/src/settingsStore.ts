import { useCallback, useEffect, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import {
  currentMonitor,
  getCurrentWindow,
  LogicalSize,
  primaryMonitor,
} from "@tauri-apps/api/window";
import { translate } from "./i18n";
import type { AppLocale } from "./i18n";
import {
  defaultAccentColor,
  defaultClientAnimation,
  defaultGameScreenMode,
  defaultResolution,
  getResolutionFromSize,
  getResolutionSize,
  isAppResolution,
  isClientAnimation,
  isGameScreenMode,
  isHexColor,
  isLocale,
  readStoredSettings,
  type AppResolution,
  type ClientAnimation,
  type GameScreenMode,
  writeStoredSettings,
} from "./settings";

const twoKResolutionMinimum = {
  height: 1440,
  width: 2560,
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
  const [allowFriendRequests, setAllowFriendRequests] = useState(() => {
    const storedSettings = readStoredSettings();
    return typeof storedSettings.allowFriendRequests === "boolean"
      ? storedSettings.allowFriendRequests
      : true;
  });
  const [clientAnimation, setClientAnimation] = useState<ClientAnimation>(() => {
    const storedSettings = readStoredSettings();
    return isClientAnimation(storedSettings.clientAnimation)
      ? storedSettings.clientAnimation
      : defaultClientAnimation;
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
      (resolution === "2140x1440" && !monitorResolutionSupport.fourK)
    ) {
      setResolution(defaultResolution);
    }
  }, [monitorResolutionSupport, resolution]);

  useEffect(() => {
    document.documentElement.style.setProperty("--accent-color", accentColor);
    document.documentElement.style.setProperty(
      "--accent-foreground-color",
      getAccentForegroundColor(accentColor),
    );
    writeStoredSettings({
      accentColor,
      allowFriendRequests,
      clientAnimation,
      gameScreenMode,
      locale,
      resolution,
    });
  }, [accentColor, allowFriendRequests, clientAnimation, gameScreenMode, locale, resolution]);

  useEffect(() => {
    if (!runsInTauriLikeShell()) {
      return;
    }

    if (!monitorResolutionSupport) {
      return;
    }

    if (
      (resolution === "1920x1080" && !monitorResolutionSupport.twoK) ||
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
    allowFriendRequests,
    clientAnimation,
    gameScreenMode,
    locale,
    resolution,
    supportsFourKResolution: monitorResolutionSupport?.fourK === true,
    supportsTwoKResolution: monitorResolutionSupport?.twoK === true,
    t,
    setAccentColor,
    setAllowFriendRequests,
    setClientAnimation,
    setGameScreenMode,
    setLocale,
    setResolution,
  };
}

async function applyWindowResolution(
  resolution: AppResolution,
  setResolution: (resolution: AppResolution) => void,
) {
  const appWindow = getCurrentWindow();
  const { height, width } = getResolutionSize(resolution);

  if (await appWindow.isMaximized()) {
    await appWindow.unmaximize();
  }

  await appWindow.setSize(new LogicalSize(width, height));

  const scaleFactor = await appWindow.scaleFactor();
  const currentSize = await appWindow.innerSize();
  const currentResolution = getResolutionFromSize(
    currentSize.width / scaleFactor,
    currentSize.height / scaleFactor,
  );

  if (currentResolution && currentResolution !== resolution) {
    setResolution(currentResolution);
  }
}

type MonitorResolutionSupport = {
  fourK: boolean;
  twoK: boolean;
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
