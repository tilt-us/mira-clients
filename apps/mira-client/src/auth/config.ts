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

export let KEYCLOAK_ENVIRONMENT = defaultConfig.environment;
export let WEBSITE_URL = defaultConfig.websiteUrl;

export let KEYCLOAK_ISSUER_URL = defaultConfig.authIssuerUrl;

export let KEYCLOAK_AUTH_URL = getKeycloakAuthUrl();

export let KEYCLOAK_TOKEN_URL = getKeycloakTokenUrl();

export const NATIVE_LOOPBACK_REDIRECT_BASE = "http://127.0.0.1";

export function getRedirectUri() {
  return getBrowserRedirectUri();
}

export function applyKeycloakRuntimeConfig(config: KeycloakRuntimeConfig) {
  const environment = getEnvironmentConfig(config.environment);

  KEYCLOAK_ENVIRONMENT = environment.environment;
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

export function getBrowserRedirectUri(
  location: Pick<Location, "origin" | "pathname"> = window.location,
) {
  return location.origin + location.pathname;
}
