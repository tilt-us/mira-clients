import type { AppLocale } from "./i18n";

export const settingsStorageKey = "mira-client-settings";
export const defaultAccentColor = "#f2c45b";
export const defaultResolution = "1600x900";
export const defaultClientAnimation = "all";
export const defaultGameScreenMode = "borderless";

export type AppResolution =
  | "1270x720"
  | "1400x800"
  | "1600x900"
  | "1920x1080"
  | "2140x1440";

export type ClientAnimation = "all" | "ui-elements" | "images" | "none";
export type GameScreenMode = "full" | "window" | "borderless";

export type StoredSettings = {
  accentColor?: string;
  allowFriendRequests?: boolean;
  clientAnimation?: ClientAnimation;
  gameScreenMode?: GameScreenMode;
  locale?: AppLocale;
  resolution?: AppResolution;
};

export function readStoredSettings() {
  try {
    const storedSettings = localStorage.getItem(settingsStorageKey);

    if (!storedSettings) {
      return {};
    }

    return JSON.parse(storedSettings) as StoredSettings;
  } catch {
    return {};
  }
}

export function writeStoredSettings(settings: Required<StoredSettings>) {
  localStorage.setItem(settingsStorageKey, JSON.stringify(settings));
}

export function isLocale(value: unknown): value is AppLocale {
  return value === "de" || value === "en";
}

export function isHexColor(value: unknown): value is string {
  return typeof value === "string" && /^#[0-9a-fA-F]{6}$/.test(value);
}

export function isAppResolution(value: unknown): value is AppResolution {
  return (
    value === "1270x720" ||
    value === "1400x800" ||
    value === "1600x900" ||
    value === "1920x1080" ||
    value === "2140x1440"
  );
}

export function isClientAnimation(value: unknown): value is ClientAnimation {
  return (
    value === "all" ||
    value === "ui-elements" ||
    value === "images" ||
    value === "none"
  );
}

export function isGameScreenMode(value: unknown): value is GameScreenMode {
  return value === "full" || value === "window" || value === "borderless";
}

export function getResolutionSize(resolution: AppResolution) {
  const [width, height] = resolution.split("x").map(Number);

  return { height, width };
}

export function getResolutionFromSize(
  width: number,
  height: number,
): AppResolution | undefined {
  const roundedWidth = Math.round(width);
  const roundedHeight = Math.round(height);

  if (roundedWidth === 1270 && roundedHeight === 720) {
    return "1270x720";
  }

  if (roundedWidth === 1400 && roundedHeight === 800) {
    return "1400x800";
  }

  if (roundedWidth === 1600 && roundedHeight === 900) {
    return "1600x900";
  }

  if (roundedWidth === 1920 && roundedHeight === 1080) {
    return "1920x1080";
  }

  if (roundedWidth === 2140 && roundedHeight === 1440) {
    return "2140x1440";
  }

  return undefined;
}
