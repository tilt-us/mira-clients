import { afterEach, describe, expect, test } from "vitest";
import {
  backgroundChampionNames,
  defaultAccentColor,
  defaultBackgroundChampion,
  defaultChatPosition,
  defaultClientAnimation,
  defaultFriendRequestPolicy,
  defaultGameScreenMode,
  defaultResolution,
  defaultShowEmailPublic,
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
  normalizeClientSettingsApiResponse,
  readStoredSettings,
  settingsStorageKey,
  toClientSettingsApiRequest,
  writeStoredSettings,
} from "../src/settings";

afterEach(() => {
  localStorage.clear();
});

describe("settings validators", () => {
  test("accepts known settings values", () => {
    expect(isLocale("de")).toBe(true);
    expect(isLocale("en")).toBe(true);
    expect(isHexColor("#F2c45b")).toBe(true);
    expect(isAppResolution("1600x900")).toBe(true);
    expect(isClientAnimation("ui-elements")).toBe(true);
    expect(isUiScale(1.25)).toBe(true);
    expect(isGameScreenMode("borderless")).toBe(true);
    expect(isBackgroundChampion("yuna")).toBe(true);
    expect(backgroundChampionNames).toEqual(["ignara", "lira", "sophia", "yuna"]);
    expect(isFriendRequestPolicy("vip")).toBe(true);
    expect(isChatPosition("left")).toBe(true);
  });

  test("rejects unknown settings values", () => {
    expect(isLocale("fr")).toBe(false);
    expect(isHexColor("#12345")).toBe(false);
    expect(isHexColor("123456")).toBe(false);
    expect(isAppResolution("1024x768")).toBe(false);
    expect(isClientAnimation("fast")).toBe(false);
    expect(isUiScale(0.75)).toBe(false);
    expect(isGameScreenMode("desktop")).toBe(false);
    expect(isBackgroundChampion("unknown")).toBe(false);
    expect(isFriendRequestPolicy("friends-only")).toBe(false);
    expect(isChatPosition("bottom")).toBe(false);
  });
});

describe("resolution helpers", () => {
  test("converts resolution ids to dimensions", () => {
    expect(getResolutionSize("1270x720")).toEqual({ height: 720, width: 1270 });
    expect(getResolutionSize("2140x1080")).toEqual({ height: 1080, width: 2140 });
  });

  test("maps rounded dimensions back to known resolution ids", () => {
    expect(getResolutionFromSize(1269.7, 720.2)).toBe("1270x720");
    expect(getResolutionFromSize(1400, 800)).toBe("1400x800");
    expect(getResolutionFromSize(1600, 900)).toBe(defaultResolution);
    expect(getResolutionFromSize(1920, 1080)).toBe("1920x1080");
    expect(getResolutionFromSize(2140, 1080)).toBe("2140x1080");
    expect(getResolutionFromSize(1111, 777)).toBeUndefined();
  });
});

describe("accent color helpers", () => {
  test("selects dark foreground for light accent colors", () => {
    expect(getAccentForegroundColor("#f2c45b")).toBe("#101216");
  });

  test("selects white foreground for dark accent colors", () => {
    expect(getAccentForegroundColor("#123456")).toBe("#ffffff");
  });
});

describe("stored settings", () => {
  test("returns an empty object when no settings are stored", () => {
    expect(readStoredSettings()).toEqual({});
  });

  test("reads written settings", () => {
    const settings = {
      accentColor: defaultAccentColor,
      allowFriendRequests: true,
      backgroundChampion: defaultBackgroundChampion,
      chatPosition: defaultChatPosition,
      clientAnimation: defaultClientAnimation,
      folders: [],
      friendRequestPolicy: defaultFriendRequestPolicy,
      gameScreenMode: defaultGameScreenMode,
      locale: "de" as const,
      resolution: defaultResolution,
      showEmailPublic: defaultShowEmailPublic,
      uiScale: defaultUiScale,
    };

    writeStoredSettings(settings);

    expect(readStoredSettings()).toEqual(settings);
  });

  test("falls back to an empty object for invalid JSON", () => {
    localStorage.setItem(settingsStorageKey, "{invalid");

    expect(readStoredSettings()).toEqual({});
  });
});

describe("remote settings mapping", () => {
  test("normalizes API settings and serializes updates", () => {
    const normalizedSettings = normalizeClientSettingsApiResponse({
      accent_color: "#5b78eb",
      allow_friend_request: "vip",
      background: "lira",
      chat_position: "left",
      client_animation: "none",
      folders: [
        {
          friendPublicIds: [9101, 9102],
          name: "Lane",
        },
      ],
      language: "en",
      resolution: "1400x800",
      screen_mode: "window",
      show_email_public: true,
      ui_scale: 1,
    });

    expect(normalizedSettings).toEqual({
      accentColor: "#5b78eb",
      backgroundChampion: "lira",
      chatPosition: "left",
      clientAnimation: "none",
      folders: [
        {
          friendPublicIds: [9101, 9102],
          name: "Lane",
        },
      ],
      friendRequestPolicy: "vip",
      gameScreenMode: "window",
      locale: "en",
      resolution: "1400x800",
      showEmailPublic: true,
      uiScale: 1,
    });

    expect(
      toClientSettingsApiRequest({
        accentColor: "#5b78eb",
        allowFriendRequests: false,
        backgroundChampion: "lira",
        chatPosition: "left",
        clientAnimation: "none",
        folders: [
          {
            friendPublicIds: [9101],
            name: "Lane",
          },
        ],
        friendRequestPolicy: "vip",
        gameScreenMode: "window",
        locale: "en",
        resolution: "1400x800",
        showEmailPublic: true,
        uiScale: 1,
      }),
    ).toEqual({
      accentColor: "#5b78eb",
      allowFriendRequest: "vip",
      background: "lira",
      chatPosition: "left",
      clientAnimation: "none",
      language: "en",
      resolution: "1400x800",
      screenMode: "window",
      show_email_public: true,
      showEmailPublic: true,
      uiScale: 1,
    });
  });

  test("normalizes partial and malformed API settings defensively", () => {
    expect(normalizeClientSettingsApiResponse(undefined)).toEqual({});
    expect(
      normalizeClientSettingsApiResponse({
        accentColor: "#5b78eb",
        allowFriendRequest: "allow",
        background: "unknown",
        chatPosition: "right",
        clientAnimation: "all",
        folders: [
          null,
          "invalid",
          { friendPublicIds: "invalid", name: "No IDs" },
          { friend_public_ids: ["9101", 9101, -1, 0, "bad", true], name: "  Duo  " },
          { friendPublicIds: [9102], name: "   " },
        ],
        language: "fr",
        resolution: "1024x768",
        screenMode: "borderless",
        showEmailPublic: true,
        uiScale: "bad",
      }),
    ).toEqual({
      accentColor: "#5b78eb",
      chatPosition: "right",
      clientAnimation: "all",
      folders: [
        {
          friendPublicIds: [],
          name: "No IDs",
        },
        {
          friendPublicIds: [9101],
          name: "Duo",
        },
      ],
      friendRequestPolicy: "allow",
      gameScreenMode: "borderless",
      showEmailPublic: true,
    });

    expect(
      normalizeClientSettingsApiResponse({
        folders: "invalid",
        show_email_public: "yes",
        ui_scale: {},
      }),
    ).toEqual({});
  });
});
