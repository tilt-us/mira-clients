import {
  getBuildEnvironmentConfig,
  getEnvironmentConfig,
  type MiraEnvironment,
} from "../environment";

export type KeycloakRuntimeConfig = {
  environment: MiraEnvironment;
};

export let KEYCLOAK_CLIENT_ID =
  import.meta.env.VITE_KEYCLOAK_CLIENT_ID ?? "mira-bevy";

export let KEYCLOAK_PASSWORD_CLIENT_ID =
  import.meta.env.VITE_KEYCLOAK_PASSWORD_CLIENT_ID ?? "mira-e2e";

const defaultConfig = getBuildEnvironmentConfig();

export let WEBSITE_URL = defaultConfig.websiteUrl;

export let KEYCLOAK_ISSUER_URL = defaultConfig.authIssuerUrl;

export let KEYCLOAK_AUTH_URL = getKeycloakAuthUrl();

export let KEYCLOAK_TOKEN_URL = getKeycloakTokenUrl();

export const DESKTOP_REDIRECT_URI = "http://localhost:1420/";

export function getRedirectUri() {
  return isTauriLocation() ? DESKTOP_REDIRECT_URI : getBrowserRedirectUri();
}

export function applyKeycloakRuntimeConfig(config: KeycloakRuntimeConfig) {
  const environment = getEnvironmentConfig(config.environment);

  WEBSITE_URL = environment.websiteUrl;
  KEYCLOAK_ISSUER_URL = environment.authIssuerUrl;
  KEYCLOAK_AUTH_URL = getKeycloakAuthUrl();
  KEYCLOAK_TOKEN_URL = getKeycloakTokenUrl();
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
