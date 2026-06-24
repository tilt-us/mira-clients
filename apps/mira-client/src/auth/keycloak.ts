import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  KEYCLOAK_AUTH_URL,
  KEYCLOAK_CLIENT_ID,
  KEYCLOAK_ISSUER_URL,
  KEYCLOAK_PASSWORD_CLIENT_ID,
  KEYCLOAK_TOKEN_URL,
  REDIRECT_URI,
} from "./config";
import { apiFetch } from "../api/http";
import {
  clearTokens,
  clearOAuthRequest,
  readOAuthRequest,
  readTokens,
  saveOAuthRequest,
  saveTokens,
  type AuthTokens,
} from "./storage";

type TokenResponse = {
  access_token: string;
  refresh_token?: string;
  expires_in?: number;
};

const accessTokenRefreshMarginMs = 60_000;

let refreshPromise: Promise<AuthTokens | undefined> | undefined;

function createRandomString(byteLength = 32) {
  const bytes = new Uint8Array(byteLength);
  crypto.getRandomValues(bytes);
  return base64UrlEncode(bytes);
}

function base64UrlEncode(bytes: Uint8Array) {
  let value = "";

  for (const byte of bytes) {
    value += String.fromCharCode(byte);
  }

  return btoa(value).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function base64UrlDecode(value: string) {
  const paddedValue = value.padEnd(value.length + ((4 - (value.length % 4)) % 4), "=");
  const normalizedValue = paddedValue.replace(/-/g, "+").replace(/_/g, "/");

  return decodeURIComponent(
    Array.from(atob(normalizedValue))
      .map((character) => {
        return `%${character.charCodeAt(0).toString(16).padStart(2, "0")}`;
      })
      .join(""),
  );
}

export function getAccessTokenIssuer(accessToken: string) {
  try {
    const [, payload] = accessToken.split(".");

    if (!payload) {
      return undefined;
    }

    const parsedPayload = JSON.parse(base64UrlDecode(payload)) as {
      iss?: unknown;
    };

    return typeof parsedPayload.iss === "string" ? parsedPayload.iss : undefined;
  } catch {
    return undefined;
  }
}

export function assertAccessTokenIssuer(accessToken: string) {
  const issuer = getAccessTokenIssuer(accessToken);

  if (issuer && issuer !== KEYCLOAK_ISSUER_URL) {
    throw new Error(
      `Keycloak Issuer passt nicht. Erwartet ${KEYCLOAK_ISSUER_URL}, erhalten ${issuer}. Bitte neu einloggen.`,
    );
  }
}

async function createCodeChallenge(codeVerifier: string) {
  const data = new TextEncoder().encode(codeVerifier);
  const hash = await crypto.subtle.digest("SHA-256", data);
  return base64UrlEncode(new Uint8Array(hash));
}

function toAuthTokens(
  tokenResponse: TokenResponse,
  clientId: string,
  fallbackRefreshToken?: string,
): AuthTokens {
  assertAccessTokenIssuer(tokenResponse.access_token);

  return {
    accessToken: tokenResponse.access_token,
    clientId,
    refreshToken: tokenResponse.refresh_token ?? fallbackRefreshToken,
    expiresAt: tokenResponse.expires_in
      ? Date.now() + tokenResponse.expires_in * 1000
      : undefined,
  };
}

async function requestToken(
  body: URLSearchParams,
  clientId: string,
  fallbackRefreshToken?: string,
) {
  const response = await apiFetch(KEYCLOAK_TOKEN_URL, {
    method: "POST",
    headers: {
      "Content-Type": "application/x-www-form-urlencoded",
    },
    body,
  });

  const responseText = await response.text();
  const parsedResponse = responseText
    ? (JSON.parse(responseText) as Partial<TokenResponse> & {
        error?: string;
        error_description?: string;
      })
    : {};

  if (!response.ok || !parsedResponse.access_token) {
    throw new Error(
      normalizeKeycloakError(
        parsedResponse.error_description ??
          parsedResponse.error ??
          "Anmeldung fehlgeschlagen.",
      ),
    );
  }

  return toAuthTokens(parsedResponse as TokenResponse, clientId, fallbackRefreshToken);
}

function normalizeKeycloakError(error: string) {
  if (error === "Account is not fully set up") {
    return "Account ist noch nicht vollständig eingerichtet. Bitte Email verifizieren oder Required Actions in Keycloak abschließen.";
  }

  if (error === "Client not allowed for direct access grants") {
    return "Dieser Keycloak-Client erlaubt keinen Login mit Benutzername und Passwort.";
  }

  if (error === "Invalid client or Invalid client credentials") {
    return "Keycloak-Client ist falsch konfiguriert.";
  }

  return error;
}

export async function startGoogleLogin() {
  const state = createRandomString(24);
  const codeVerifier = createRandomString(64);
  const codeChallenge = await createCodeChallenge(codeVerifier);
  const searchParams = new URLSearchParams({
    client_id: KEYCLOAK_CLIENT_ID,
    code_challenge: codeChallenge,
    code_challenge_method: "S256",
    kc_idp_hint: "google",
    prompt: "select_account",
    redirect_uri: REDIRECT_URI,
    response_type: "code",
    scope: "openid email profile",
    state,
  });

  saveOAuthRequest(state, codeVerifier);
  const authUrl = `${KEYCLOAK_AUTH_URL}?${searchParams.toString()}`;
  console.info("[mira-client] Starting Google login", {
    authUrl,
    keycloakClientRedirectUri: REDIRECT_URI,
    expectedGoogleRedirectUri: `${KEYCLOAK_ISSUER_URL}/broker/google/endpoint`,
  });

  if (isTauri()) {
    await invoke("start_oauth_window", {
      request: {
        authUrl,
        redirectUri: REDIRECT_URI,
      },
    });
    return;
  }

  window.location.assign(authUrl);
}

export async function completeRedirectLogin(callbackUrl?: string) {
  const url = new URL(callbackUrl ?? window.location.href);
  const code = url.searchParams.get("code");
  const state = url.searchParams.get("state");
  const error = url.searchParams.get("error_description") ?? url.searchParams.get("error");

  if (error) {
    clearOAuthRequest();
    if (!callbackUrl) {
      window.history.replaceState({}, document.title, REDIRECT_URI);
    }
    throw new Error(error);
  }

  if (!code || !state) {
    return undefined;
  }

  const savedRequest = readOAuthRequest();

  if (state !== savedRequest.state || !savedRequest.codeVerifier) {
    clearOAuthRequest();
    if (!callbackUrl) {
      window.history.replaceState({}, document.title, REDIRECT_URI);
    }
    throw new Error("OAuth-Antwort konnte nicht validiert werden.");
  }

  const tokens = await requestToken(
    new URLSearchParams({
      client_id: KEYCLOAK_CLIENT_ID,
      code,
      code_verifier: savedRequest.codeVerifier,
      grant_type: "authorization_code",
      redirect_uri: REDIRECT_URI,
    }),
    KEYCLOAK_CLIENT_ID,
  );

  clearOAuthRequest();
  if (!callbackUrl) {
    window.history.replaceState({}, document.title, REDIRECT_URI);
  }
  return tokens;
}

export function loginWithPassword(username: string, password: string) {
  return requestToken(
    new URLSearchParams({
      client_id: KEYCLOAK_PASSWORD_CLIENT_ID,
      grant_type: "password",
      password,
      scope: "openid email profile",
      username,
    }),
    KEYCLOAK_PASSWORD_CLIENT_ID,
  );
}

export async function getValidAccessToken() {
  const tokens = readTokens();

  if (!tokens?.accessToken) {
    return undefined;
  }

  assertAccessTokenIssuer(tokens.accessToken);

  if (!shouldRefreshAccessToken(tokens)) {
    return tokens.accessToken;
  }

  const refreshedTokens = await refreshStoredAccessToken(tokens);

  return refreshedTokens?.accessToken ?? tokens.accessToken;
}

function shouldRefreshAccessToken(tokens: AuthTokens) {
  return Boolean(
    tokens.refreshToken &&
      tokens.expiresAt &&
      tokens.expiresAt - accessTokenRefreshMarginMs <= Date.now(),
  );
}

async function refreshStoredAccessToken(tokens: AuthTokens) {
  refreshPromise ??= refreshAccessToken(tokens).finally(() => {
    refreshPromise = undefined;
  });

  return refreshPromise;
}

async function refreshAccessToken(tokens: AuthTokens) {
  if (!tokens.refreshToken) {
    return undefined;
  }

  const clientIds = tokens.clientId
    ? [tokens.clientId]
    : [KEYCLOAK_CLIENT_ID, KEYCLOAK_PASSWORD_CLIENT_ID];

  for (const clientId of clientIds) {
    try {
      const refreshedTokens = await requestToken(
        new URLSearchParams({
          client_id: clientId,
          grant_type: "refresh_token",
          refresh_token: tokens.refreshToken,
        }),
        clientId,
        tokens.refreshToken,
      );

      saveTokens(refreshedTokens);

      return refreshedTokens;
    } catch {
      continue;
    }
  }

  clearTokens();
  return undefined;
}
