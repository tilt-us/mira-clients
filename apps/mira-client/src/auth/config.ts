export type KeycloakRuntimeConfig = {
  keycloakBaseUrl?: string;
  keycloakRealm?: string;
  keycloakClientId?: string;
  keycloakPasswordClientId?: string;
};

function normalizeKeycloakBaseUrl(baseUrl: string) {
  try {
    const url = new URL(baseUrl);

    if (url.hostname === "127.0.0.1") {
      url.hostname = "localhost";
    }

    return url.toString().replace(/\/$/, "");
  } catch {
    return baseUrl.replace("127.0.0.1", "localhost").replace(/\/$/, "");
  }
}

export let KEYCLOAK_BASE_URL = normalizeKeycloakBaseUrl(
  import.meta.env.VITE_KEYCLOAK_BASE_URL ?? "http://localhost:8081",
);

export let KEYCLOAK_REALM = import.meta.env.VITE_KEYCLOAK_REALM ?? "mira";

export let KEYCLOAK_CLIENT_ID =
  import.meta.env.VITE_KEYCLOAK_CLIENT_ID ?? "mira-bevy";

export let KEYCLOAK_PASSWORD_CLIENT_ID =
  import.meta.env.VITE_KEYCLOAK_PASSWORD_CLIENT_ID ?? "mira-e2e";

export let KEYCLOAK_ISSUER_URL = getKeycloakIssuerUrl();

export let KEYCLOAK_AUTH_URL = getKeycloakAuthUrl();

export let KEYCLOAK_TOKEN_URL = getKeycloakTokenUrl();

export const DESKTOP_REDIRECT_URI = "http://localhost:1420/";

export function getRedirectUri() {
  return isTauriLocation() ? DESKTOP_REDIRECT_URI : getBrowserRedirectUri();
}

export function applyKeycloakRuntimeConfig(config: KeycloakRuntimeConfig) {
  KEYCLOAK_BASE_URL = normalizeKeycloakBaseUrl(
    config.keycloakBaseUrl ?? KEYCLOAK_BASE_URL,
  );
  KEYCLOAK_REALM = valueOrDefault(config.keycloakRealm, KEYCLOAK_REALM);
  KEYCLOAK_CLIENT_ID = valueOrDefault(
    config.keycloakClientId,
    KEYCLOAK_CLIENT_ID,
  );
  KEYCLOAK_PASSWORD_CLIENT_ID = valueOrDefault(
    config.keycloakPasswordClientId,
    KEYCLOAK_PASSWORD_CLIENT_ID,
  );
  KEYCLOAK_ISSUER_URL = getKeycloakIssuerUrl();
  KEYCLOAK_AUTH_URL = getKeycloakAuthUrl();
  KEYCLOAK_TOKEN_URL = getKeycloakTokenUrl();
}

function getKeycloakIssuerUrl() {
  return `${KEYCLOAK_BASE_URL}/realms/${KEYCLOAK_REALM}`;
}

function getKeycloakAuthUrl() {
  return `${KEYCLOAK_ISSUER_URL}/protocol/openid-connect/auth`;
}

function getKeycloakTokenUrl() {
  return `${KEYCLOAK_ISSUER_URL}/protocol/openid-connect/token`;
}

function getBrowserRedirectUri() {
  return window.location.origin + window.location.pathname;
}

function isTauriLocation() {
  return (
    window.location.protocol === "tauri:" ||
    window.location.hostname === "tauri.localhost"
  );
}

function valueOrDefault(value: string | undefined, defaultValue: string) {
  const normalizedValue = value?.trim();
  return normalizedValue ? normalizedValue : defaultValue;
}
