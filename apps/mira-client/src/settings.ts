import type { AppLocale } from "./i18n";

export const settingsStorageKey = "mira-client-settings";
export const defaultAccentColor = "#f2c45b";
export const defaultResolution = "1600x900";
export const defaultClientAnimation = "all";
export const defaultUiScale = 1.0;
export const defaultGameScreenMode = "borderless";
export const defaultLocale = "de";
export const defaultFriendRequestPolicy = "allow";
export const defaultChatPosition = "right";
export const defaultShowEmailPublic = false;

const characterAssetModules = import.meta.glob("../../../assets/characters/*.png", {
  eager: true,
  import: "default",
  query: "?url",
});

export const backgroundChampionNames = Object.keys(characterAssetModules)
  .map((path) => path.match(/\/([^/]+)\.png$/)?.[1])
  .filter((name): name is string => Boolean(name))
  .sort((left, right) => left.localeCompare(right));
export const defaultBackgroundChampion = backgroundChampionNames.includes("yuna")
  ? "yuna"
  : (backgroundChampionNames[0] ?? "yuna");

export type AppResolution =
  | "1270x720"
  | "1400x800"
  | "1600x900"
  | "1920x1080"
  | "2140x1080";

export type ClientAnimation = "all" | "ui-elements" | "images" | "none";
export type UiScale = 0.5 | 0.6 | 0.7 | 0.8 | 0.9 | 1 | 1.1 | 1.25 | 1.5;
export type GameScreenMode = "full" | "window" | "borderless";
export type BackgroundChampion = string;
export type FriendRequestPolicy = "allow" | "disallow" | "vip";
export type ChatPosition = "left" | "right";

export type StoredSettings = {
  accentColor?: string;
  allowFriendRequests?: boolean;
  backgroundChampion?: BackgroundChampion;
  chatPosition?: ChatPosition;
  clientAnimation?: ClientAnimation;
  folders?: ClientSettingsFolder[];
  friendRequestPolicy?: FriendRequestPolicy;
  gameScreenMode?: GameScreenMode;
  locale?: AppLocale;
  resolution?: AppResolution;
  showEmailPublic?: boolean;
  uiScale?: UiScale;
};

export type ClientSettingsFolder = {
  friendPublicIds: number[];
  name: string;
};

export type ClientSettingsApiResponse = {
  accent_color?: unknown;
  accentColor?: unknown;
  allow_friend_request?: unknown;
  allowFriendRequest?: unknown;
  background?: unknown;
  chat_position?: unknown;
  chatPosition?: unknown;
  client_animation?: unknown;
  clientAnimation?: unknown;
  experimental_features?: unknown;
  experimentalFeatures?: unknown;
  folders?: unknown;
  language?: unknown;
  resolution?: unknown;
  screen_mode?: unknown;
  screenMode?: unknown;
  show_email_public?: unknown;
  showEmailPublic?: unknown;
  ui_scale?: unknown;
  uiScale?: unknown;
  use_friend_colors?: unknown;
  useFriendColors?: unknown;
};

export type ClientSettingsApiRequest = {
  accentColor?: string;
  allowFriendRequest?: FriendRequestPolicy;
  background?: BackgroundChampion;
  chatPosition?: ChatPosition;
  clientAnimation?: ClientAnimation;
  folders?: ClientSettingsFolder[];
  language?: AppLocale;
  resolution?: AppResolution;
  screenMode?: GameScreenMode;
  show_email_public?: boolean;
  showEmailPublic?: boolean;
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

export function normalizeClientSettingsApiResponse(
  response: ClientSettingsApiResponse | undefined,
): StoredSettings {
  if (!response) {
    return {};
  }

  const accentColor = getApiField(response, "accent_color", "accentColor");
  const allowFriendRequest = getApiField(
    response,
    "allow_friend_request",
    "allowFriendRequest",
  );
  const background = getApiField(response, "background");
  const chatPosition = getApiField(response, "chat_position", "chatPosition");
  const clientAnimation = getApiField(
    response,
    "client_animation",
    "clientAnimation",
  );
  const language = getApiField(response, "language");
  const folders = normalizeClientSettingsFolders(response.folders);
  const resolution = getApiField(response, "resolution");
  const screenMode = getApiField(response, "screen_mode", "screenMode");
  const showEmailPublic = getApiField(
    response,
    "show_email_public",
    "showEmailPublic",
  );
  const uiScale = normalizeUiScale(getApiField(response, "ui_scale", "uiScale"));

  return {
    ...(isHexColor(accentColor) ? { accentColor } : {}),
    ...(isFriendRequestPolicy(allowFriendRequest)
      ? { friendRequestPolicy: allowFriendRequest }
      : {}),
    ...(isBackgroundChampion(background) ? { backgroundChampion: background } : {}),
    ...(isChatPosition(chatPosition) ? { chatPosition } : {}),
    ...(isClientAnimation(clientAnimation) ? { clientAnimation } : {}),
    ...(folders ? { folders } : {}),
    ...(isLocale(language) ? { locale: language } : {}),
    ...(isAppResolution(resolution) ? { resolution } : {}),
    ...(isGameScreenMode(screenMode) ? { gameScreenMode: screenMode } : {}),
    ...(typeof showEmailPublic === "boolean" ? { showEmailPublic } : {}),
    ...(isUiScale(uiScale) ? { uiScale } : {}),
  };
}

export function toClientSettingsApiRequest(
  settings: Required<StoredSettings>,
): ClientSettingsApiRequest {
  return {
    accentColor: settings.accentColor,
    allowFriendRequest: settings.friendRequestPolicy,
    background: settings.backgroundChampion,
    chatPosition: settings.chatPosition,
    clientAnimation: settings.clientAnimation,
    folders: settings.folders,
    language: settings.locale,
    resolution: settings.resolution,
    screenMode: settings.gameScreenMode,
    show_email_public: settings.showEmailPublic,
    showEmailPublic: settings.showEmailPublic,
    uiScale: settings.uiScale,
  };
}

export function isLocale(value: unknown): value is AppLocale {
  return value === "de" || value === "en";
}

export function isHexColor(value: unknown): value is string {
  return typeof value === "string" && /^#[0-9a-fA-F]{6}$/.test(value);
}

export function getAccentForegroundColor(hexColor: string) {
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

export function isAppResolution(value: unknown): value is AppResolution {
  return (
    value === "1270x720" ||
    value === "1400x800" ||
    value === "1600x900" ||
    value === "1920x1080" ||
    value === "2140x1080"
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
  return typeof value === "string" && backgroundChampionNames.includes(value);
}

export function isFriendRequestPolicy(value: unknown): value is FriendRequestPolicy {
  return value === "allow" || value === "disallow" || value === "vip";
}

export function isChatPosition(value: unknown): value is ChatPosition {
  return value === "left" || value === "right";
}

function getApiField(
  response: ClientSettingsApiResponse,
  snakeName: keyof ClientSettingsApiResponse,
  camelName?: keyof ClientSettingsApiResponse,
) {
  return response[snakeName] ?? (camelName ? response[camelName] : undefined);
}

function normalizeUiScale(value: unknown) {
  if (typeof value === "number") {
    return value;
  }

  if (typeof value === "string") {
    const parsedValue = Number(value);

    return Number.isFinite(parsedValue) ? parsedValue : undefined;
  }

  return undefined;
}

function normalizeClientSettingsFolders(value: unknown) {
  if (!Array.isArray(value)) {
    return undefined;
  }

  return value
    .map((folder): ClientSettingsFolder | undefined => {
      if (!folder || typeof folder !== "object") {
        return undefined;
      }

      const record = folder as {
        friendPublicIds?: unknown;
        friend_public_ids?: unknown;
        name?: unknown;
      };
      const name = typeof record.name === "string" ? record.name.trim() : "";
      const publicIds = record.friendPublicIds ?? record.friend_public_ids;

      if (!name) {
        return undefined;
      }

      return {
        friendPublicIds: normalizeFriendPublicIds(publicIds),
        name,
      };
    })
    .filter((folder): folder is ClientSettingsFolder => Boolean(folder));
}

function normalizeFriendPublicIds(value: unknown) {
  if (!Array.isArray(value)) {
    return [];
  }

  return [
    ...new Set(
      value
        .map((publicId) => {
          if (typeof publicId === "number") {
            return publicId;
          }

          if (typeof publicId === "string") {
            return Number.parseInt(publicId, 10);
          }

          return Number.NaN;
        })
        .filter((publicId) => Number.isInteger(publicId) && publicId > 0),
    ),
  ];
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

  if (roundedWidth === 2140 && roundedHeight === 1080) {
    return "2140x1080";
  }

  return undefined;
}
