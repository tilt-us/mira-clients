import type { AppLocale } from "./i18n";

export const settingsStorageKey = "mira-client-settings";
export const defaultAccentColor = "#f2c45b";
export const defaultResolution = "1600x900";
export const defaultClientAnimation = "all";
export const defaultUiScale = 0.9;
export const defaultGameScreenMode = "borderless";
export const defaultBackgroundChampion = "yuna";
export const defaultFriendRequestPolicy = "allow";

export type AppResolution =
  | "1270x720"
  | "1400x800"
  | "1600x900"
  | "1920x1080"
  | "2600x1600"
  | "2140x1440";

export type ClientAnimation = "all" | "ui-elements" | "images" | "none";
export type UiScale = 0.5 | 0.6 | 0.7 | 0.8 | 0.9 | 1 | 1.1 | 1.25 | 1.5;
export type GameScreenMode = "full" | "window" | "borderless";
export type BackgroundChampion = "lira" | "ignara" | "yuna" | "sophia";
export type FriendRequestPolicy = "allow" | "disallow" | "vip";

export type StoredSettings = {
  accentColor?: string;
  allowFriendRequests?: boolean;
  backgroundChampion?: BackgroundChampion;
  clientAnimation?: ClientAnimation;
  friendRequestPolicy?: FriendRequestPolicy;
  gameScreenMode?: GameScreenMode;
  locale?: AppLocale;
  resolution?: AppResolution;
  uiScale?: UiScale;
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
    value === "2600x1600" ||
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

export function isUiScale(value: unknown): value is UiScale {
  return (
    value === 0.5 ||
    value === 0.6 ||
    value === 0.7 ||
    value === 0.8 ||
    value === 0.9 ||
    value === 1 ||
    value === 1.1 ||
    value === 1.25 ||
    value === 1.5
  );
}

export function isGameScreenMode(value: unknown): value is GameScreenMode {
  return value === "full" || value === "window" || value === "borderless";
}

export function isBackgroundChampion(value: unknown): value is BackgroundChampion {
  return value === "lira" || value === "ignara" || value === "yuna" || value === "sophia";
}

export function isFriendRequestPolicy(value: unknown): value is FriendRequestPolicy {
  return value === "allow" || value === "disallow" || value === "vip";
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

  if (roundedWidth === 2600 && roundedHeight === 1600) {
    return "2600x1600";
  }

  if (roundedWidth === 2140 && roundedHeight === 1440) {
    return "2140x1440";
  }

  return undefined;
}
