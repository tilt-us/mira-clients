import type { AppLocale } from "./i18n";

export const settingsStorageKey = "mira-client-settings";
export const defaultAccentColor = "#f2c45b";

export type StoredSettings = {
  accentColor?: string;
  locale?: AppLocale;
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
