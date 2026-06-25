import { afterEach, describe, expect, test } from "vitest";
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
  settingsStorageKey,
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
    expect(isFriendRequestPolicy("vip")).toBe(true);
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
  });
});

describe("resolution helpers", () => {
  test("converts resolution ids to dimensions", () => {
    expect(getResolutionSize("1270x720")).toEqual({ height: 720, width: 1270 });
    expect(getResolutionSize("2140x1440")).toEqual({ height: 1440, width: 2140 });
  });

  test("maps rounded dimensions back to known resolution ids", () => {
    expect(getResolutionFromSize(1269.7, 720.2)).toBe("1270x720");
    expect(getResolutionFromSize(1400, 800)).toBe("1400x800");
    expect(getResolutionFromSize(1600, 900)).toBe(defaultResolution);
    expect(getResolutionFromSize(1920, 1080)).toBe("1920x1080");
    expect(getResolutionFromSize(2140, 1440)).toBe("2140x1440");
    expect(getResolutionFromSize(1111, 777)).toBeUndefined();
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
      clientAnimation: defaultClientAnimation,
      friendRequestPolicy: defaultFriendRequestPolicy,
      gameScreenMode: defaultGameScreenMode,
      locale: "de" as const,
      resolution: defaultResolution,
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
