import { invoke, isTauri } from "@tauri-apps/api/core";

const browserSecurityStorageKey = "mira.oauth.browserSecurity";

export const smartScreenBrowserSecurity = "smart-screen";
export const systemBrowserSecurity = "system-browser";

export type OAuthBrowserSecurity =
  | typeof smartScreenBrowserSecurity
  | typeof systemBrowserSecurity
  | `browser:${string}`;

export type OAuthBrowserOption = {
  id: OAuthBrowserSecurity;
  kind: "installed" | "smart-screen" | "system-browser";
  name?: string;
};

type NativeOAuthBrowserOption = {
  id: string;
  name: string;
};

type NativeOAuthBrowserConfiguration = {
  defaultBrowserSecurity: string;
  installedBrowsers: NativeOAuthBrowserOption[];
  smartScreenAvailable?: boolean;
};

export function defaultOAuthBrowserSecurity(): OAuthBrowserSecurity {
  return isWindowsOAuthPlatform()
    ? systemBrowserSecurity
    : smartScreenBrowserSecurity;
}

function isWindowsOAuthPlatform() {
  return typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent);
}

export function isOAuthBrowserSecurity(value: unknown): value is OAuthBrowserSecurity {
  return (
    value === smartScreenBrowserSecurity ||
    value === systemBrowserSecurity ||
    (typeof value === "string" && /^browser:[a-z0-9-]+$/.test(value))
  );
}

export function readOAuthBrowserSecurity() {
  try {
    const value = localStorage.getItem(browserSecurityStorageKey);
    return isOAuthBrowserSecurity(value) ? value : undefined;
  } catch {
    return undefined;
  }
}

export function writeOAuthBrowserSecurity(value: OAuthBrowserSecurity) {
  try {
    localStorage.setItem(browserSecurityStorageKey, value);
  } catch {
    // Browser selection is a local convenience setting. OAuth itself still
    // falls back to the platform default if persistence is unavailable.
  }
}

export function defaultOAuthBrowserOptions(): OAuthBrowserOption[] {
  return oauthBrowserOptions(!isWindowsOAuthPlatform());
}

function oauthBrowserOptions(smartScreenAvailable: boolean): OAuthBrowserOption[] {
  const systemBrowser: OAuthBrowserOption = {
    id: systemBrowserSecurity,
    kind: "system-browser",
  };

  // Smart Screen is deliberately not offered on Windows: System Browser is
  // the supported Windows login surface for existing accounts and Windows
  // Hello. Linux and macOS retain both choices.
  if (!smartScreenAvailable) {
    return [systemBrowser];
  }

  return [
    { id: smartScreenBrowserSecurity, kind: "smart-screen" },
    systemBrowser,
  ];
}

export async function loadOAuthBrowserConfiguration() {
  const fallback = {
    defaultBrowserSecurity: defaultOAuthBrowserSecurity(),
    options: defaultOAuthBrowserOptions(),
  };

  if (!isTauri()) {
    return fallback;
  }

  try {
    const configuration = await invoke<NativeOAuthBrowserConfiguration>(
      "oauth_browser_options",
    );
    const installedBrowsers = Array.isArray(configuration.installedBrowsers)
      ? configuration.installedBrowsers
          .filter(
            (browser): browser is NativeOAuthBrowserOption =>
              typeof browser?.id === "string" &&
              /^browser:[a-z0-9-]+$/.test(browser.id) &&
              typeof browser.name === "string" &&
              browser.name.trim().length > 0,
          )
          .map((browser) => ({
            id: browser.id as OAuthBrowserSecurity,
            kind: "installed" as const,
            name: browser.name.trim(),
          }))
      : [];
    const requestedDefault = isOAuthBrowserSecurity(
      configuration.defaultBrowserSecurity,
    )
      ? configuration.defaultBrowserSecurity
      : fallback.defaultBrowserSecurity;
    const smartScreenAvailable =
      typeof configuration.smartScreenAvailable === "boolean"
        ? configuration.smartScreenAvailable
        : !isWindowsOAuthPlatform();
    const options = [...oauthBrowserOptions(smartScreenAvailable), ...installedBrowsers];
    const defaultBrowserSecurity = options.some(
      (option) => option.id === requestedDefault,
    )
      ? requestedDefault
      : systemBrowserSecurity;

    return {
      defaultBrowserSecurity,
      options,
    };
  } catch (error) {
    console.warn("[mira-client][oauth] browserSecurity=optionsUnavailable", error);
    return fallback;
  }
}
