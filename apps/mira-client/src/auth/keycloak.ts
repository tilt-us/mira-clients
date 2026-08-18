import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  KEYCLOAK_AUTH_URL,
  KEYCLOAK_CLIENT_ID,
  KEYCLOAK_ENVIRONMENT,
  KEYCLOAK_ISSUER_URL,
  KEYCLOAK_PASSWORD_CLIENT_ID,
  KEYCLOAK_TOKEN_URL,
  NATIVE_LOOPBACK_REDIRECT_BASE,
  WEBSITE_URL,
  getRedirectUri,
} from "./config";
import { apiFetch } from "../api/http";
import { getAccentForegroundColor, isHexColor } from "../settings";
import type { AppLocale } from "../i18n";
import {
  defaultOAuthBrowserSecurity,
  type OAuthBrowserSecurity,
} from "./browserSecurity";
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
  id_token?: string;
  refresh_token?: string;
  expires_in?: number;
};

const accessTokenRefreshMarginMs = 60_000;
export const passwordResetSentParam = "mira_password_reset";

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

function getTokenPayload(token?: string) {
  try {
    const [, payload] = token?.split(".") ?? [];

    if (!payload) {
      return undefined;
    }

    return JSON.parse(base64UrlDecode(payload)) as Record<string, unknown>;
  } catch {
    return undefined;
  }
}

function getTokenStringClaim(token: string | undefined, claim: string) {
  const value = getTokenPayload(token)?.[claim];

  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

function getTokenNumberClaim(token: string | undefined, claim: string) {
  const value = getTokenPayload(token)?.[claim];

  return typeof value === "number" ? value : undefined;
}

export function hasDesktopSessionClaims(token?: string) {
  return Boolean(
    getTokenStringClaim(token, "sub") &&
      (getTokenStringClaim(token, "sid") || getTokenStringClaim(token, "jti")) &&
      getTokenNumberClaim(token, "exp"),
  );
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
  fallbackIdToken?: string,
): AuthTokens {
  assertAccessTokenIssuer(tokenResponse.access_token);

  return {
    accessToken: tokenResponse.access_token,
    clientId,
    idToken: tokenResponse.id_token ?? fallbackIdToken,
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
  fallbackIdToken?: string,
) {
  if (body.get("grant_type") === "authorization_code") {
    const redirectUri = body.get("redirect_uri") ?? "missing";
    if (
      isTauri() &&
      (!nativeOAuthAttempt || nativeOAuthAttempt.redirectUri !== redirectUri)
    ) {
      throw new Error("Native OAuth token exchange redirect URI does not match its login attempt.");
    }

    if (nativeOAuthAttempt) {
      nativeOAuthAttempt.phase = "exchangingToken";
    }
    console.info(
      `[mira-client][oauth] attempt=${nativeOAuthAttempt?.attemptId ?? "browser"} tokenExchange=start method=POST url=${KEYCLOAK_TOKEN_URL} redirectUri=${redirectUri}`,
    );
  }

  let response: Response;
  try {
    response = await apiFetch(KEYCLOAK_TOKEN_URL, {
      method: "POST",
      headers: {
        "Content-Type": "application/x-www-form-urlencoded",
      },
      body,
    });
  } catch (error) {
    if (body.get("grant_type") === "authorization_code") {
      console.error(
        `[mira-client][oauth] attempt=${nativeOAuthAttempt?.attemptId ?? "browser"} stage=tokenExchange method=POST url=${KEYCLOAK_TOKEN_URL} status=network-error`,
      );
    }
    throw error;
  }

  const responseText = await response.text();
  let parsedResponse: Partial<TokenResponse> & {
    error?: string;
    error_description?: string;
  } = {};

  if (responseText) {
    try {
      parsedResponse = JSON.parse(responseText) as typeof parsedResponse;
    } catch {
      // Proxies commonly return HTML for a 404. Keep the HTTP status available
      // for the safe OAuth diagnostic below instead of masking it as JSON parsing.
    }
  }

  if (!response.ok || !parsedResponse.access_token) {
    if (body.get("grant_type") === "authorization_code") {
      console.error(
        `[mira-client][oauth] attempt=${nativeOAuthAttempt?.attemptId ?? "browser"} stage=tokenExchange method=POST url=${KEYCLOAK_TOKEN_URL} status=${response.status}`,
      );
    }
    throw new Error(
      normalizeKeycloakError(
        parsedResponse.error_description ??
          parsedResponse.error ??
          "Anmeldung fehlgeschlagen.",
      ),
    );
  }

  const tokens = toAuthTokens(
    parsedResponse as TokenResponse,
    clientId,
    fallbackRefreshToken,
    fallbackIdToken,
  );

  if (body.get("grant_type") === "authorization_code") {
    console.info(
      `[mira-client][oauth] attempt=${nativeOAuthAttempt?.attemptId ?? "browser"} tokenExchange=success status=${response.status}`,
    );
  }

  return tokens;
}

function normalizeKeycloakError(error: string) {
  const normalizedError = error.trim();
  const lowerError = normalizedError.toLowerCase();

  if (error === "Account is not fully set up") {
    return "Account ist noch nicht vollständig eingerichtet. Bitte Email verifizieren oder Required Actions in Keycloak abschließen.";
  }

  if (error === "Client not allowed for direct access grants") {
    return "Dieser Keycloak-Client erlaubt keinen Login mit Benutzername und Passwort.";
  }

  if (error === "Invalid client or Invalid client credentials") {
    return "Keycloak-Client ist falsch konfiguriert.";
  }

  if (
    lowerError === "invalid_grant" ||
    lowerError === "invalid user credentials" ||
    lowerError === "invalid username or password" ||
    lowerError === "invalid credentials"
  ) {
    return "invalid_credentials";
  }

  return error;
}

type OAuthProvider = {
  forceAccountSelection?: true;
  googleLanguage?: true;
  idpHint: string;
  name: string;
  prompt?: string;
};

type KeycloakThemeOptions = {
  accentColor: string;
  browserSecurity?: OAuthBrowserSecurity;
  locale: AppLocale;
};

export type OAuthStartResult = {
  ignored?: boolean;
  modal?: boolean;
  redirectUri?: string;
};

type NativeOAuthPreparation = {
  attemptId: number;
  redirectUri: string;
};

type NativeOAuthAttempt = NativeOAuthPreparation & {
  phase: "starting" | "waitingForCallback" | "exchangingToken";
  provider: string;
};

let nativeOAuthAttempt: NativeOAuthAttempt | undefined;

export function isNativeOAuthInFlight() {
  return Boolean(nativeOAuthAttempt);
}

function isNativeLoopbackRedirectUri(value: string) {
  return /^http:\/\/127\.0\.0\.1:[1-9]\d*$/.test(value);
}

async function beginNativeOAuthAttempt(provider: string) {
  if (!isTauri()) {
    return undefined;
  }

  if (nativeOAuthAttempt) {
    console.warn(
      `[mira-client][oauth] attempt=${nativeOAuthAttempt.attemptId || "pending"} provider=${provider} ignored=activeAttempt`,
    );
    return undefined;
  }

  // Claim the single-flight slot before awaiting the Tauri command so two
  // click handlers cannot allocate two listeners or overwrite PKCE state.
  nativeOAuthAttempt = {
    attemptId: 0,
    phase: "starting",
    provider,
    redirectUri: "",
  };

  let preparation: NativeOAuthPreparation | undefined;
  try {
    preparation = await invoke<NativeOAuthPreparation>("prepare_oauth_redirect_uri", {
      request: { provider },
    });
    if (!isNativeLoopbackRedirectUri(preparation.redirectUri)) {
      throw new Error(
        `Native OAuth callback must be an ephemeral slash-free loopback URI, received ${preparation.redirectUri}.`,
      );
    }

    nativeOAuthAttempt = {
      ...preparation,
      phase: "starting",
      provider,
    };
    console.info(
      `[mira-client][oauth] attempt=${preparation.attemptId} provider=${provider} state=starting redirectUri=${preparation.redirectUri}`,
    );
    return preparation;
  } catch (error) {
    if (preparation?.attemptId) {
      await invoke("cancel_oauth_attempt", {
        request: { attemptId: preparation.attemptId },
      }).catch(() => undefined);
    }
    nativeOAuthAttempt = undefined;
    throw error;
  }
}

async function cancelNativeOAuthAttempt(preparation?: NativeOAuthPreparation) {
  const attempt = preparation ?? nativeOAuthAttempt;

  if (attempt?.attemptId) {
    await invoke("cancel_oauth_attempt", {
      request: { attemptId: attempt.attemptId },
    }).catch(() => undefined);
  }

  if (!attempt || nativeOAuthAttempt?.attemptId === attempt.attemptId) {
    nativeOAuthAttempt = undefined;
  }
}

/** Cancels a waiting desktop-browser sign-in and releases its loopback port. */
export async function cancelNativeOAuthLogin() {
  const attempt = nativeOAuthAttempt;

  if (!attempt) {
    return;
  }

  console.info(`[mira-client][oauth] attempt=${attempt.attemptId} cancelled=user`);
  try {
    await cancelNativeOAuthAttempt(attempt);
  } finally {
    // A browser tab can still finish navigating after the user cancels. Do
    // not leave its PKCE data around for a later, unrelated login attempt.
    clearOAuthRequest();
  }
}

export function isNativeOAuthAttemptCurrent(attemptId: number) {
  return nativeOAuthAttempt?.attemptId === attemptId;
}

export function completeNativeOAuthAttempt(attemptId?: number) {
  if (attemptId !== undefined && !isNativeOAuthAttemptCurrent(attemptId)) {
    return false;
  }

  if (nativeOAuthAttempt) {
    console.info(`[mira-client][oauth] attempt=${nativeOAuthAttempt.attemptId} complete`);
  }
  nativeOAuthAttempt = undefined;
  return true;
}

function addKeycloakThemeParams(
  searchParams: URLSearchParams,
  options?: KeycloakThemeOptions,
) {
  if (!options) {
    return;
  }

  if (isHexColor(options.accentColor)) {
    searchParams.set("accent", options.accentColor.slice(1));
    searchParams.set(
      "fontColor",
      getAccentForegroundColor(options.accentColor) === "#ffffff" ? "white" : "black",
    );
  }

  const localeCode = options.locale === "de" ? "de" : "en";
  searchParams.set("kc_locale", localeCode);
  searchParams.set("lang", options.locale === "de" ? "german" : "english");
  searchParams.set("ui_locales", localeCode);
}

function getPasswordResetRedirectUri(redirectUri: string) {
  const redirectUrl = new URL(redirectUri);
  redirectUrl.searchParams.set(passwordResetSentParam, "sent");
  return redirectUrl.toString();
}

function getProviderErrorRedirectUri(redirectUri: string) {
  if (isTauri()) {
    // Keycloak validates this auxiliary redirect as well. Native OAuth has one
    // registered callback URI; adding a query parameter here would create a
    // second, unregistered redirect contract.
    return redirectUri;
  }

  const callbackUrl = new URL(redirectUri);
  const redirectUrl = new URL("/", WEBSITE_URL);

  if (callbackUrl.hostname === "localhost" || callbackUrl.hostname === "127.0.0.1") {
    redirectUrl.protocol = callbackUrl.protocol;
    redirectUrl.hostname = callbackUrl.hostname;
    redirectUrl.port = callbackUrl.port;
  }

  redirectUrl.searchParams.set("kc_error", "1");

  return redirectUrl.toString();
}

async function startProviderLogin(
  provider: OAuthProvider,
  options?: KeycloakThemeOptions,
) {
  const preparation = await beginNativeOAuthAttempt(provider.idpHint);
  if (isTauri() && !preparation) {
    return { ignored: true, modal: false } satisfies OAuthStartResult;
  }

  const redirectUri = preparation?.redirectUri ?? getRedirectUri();

  try {
    const state = createRandomString(24);
    const codeVerifier = createRandomString(64);
    const codeChallenge = await createCodeChallenge(codeVerifier);
    const errorRedirectUri = getProviderErrorRedirectUri(redirectUri);
    const searchParams = new URLSearchParams({
      client_id: KEYCLOAK_CLIENT_ID,
      code_challenge: codeChallenge,
      code_challenge_method: "S256",
      kc_idp_hint: provider.idpHint,
      redirect_uri: redirectUri,
      kc_error_redirect_uri: errorRedirectUri,
      error_redirect_uri: errorRedirectUri,
      fallback_uri: errorRedirectUri,
      returnTo: errorRedirectUri,
      response_type: "code",
      scope: "openid email profile",
      state,
    });

    if (provider.prompt) {
      searchParams.set("prompt", provider.prompt);
    }

    if (provider.forceAccountSelection) {
      // `prompt=select_account` is forwarded to the social provider, but an
      // existing Keycloak SSO session can otherwise complete the broker flow
      // before that provider gets a chance to show its account chooser.
      searchParams.set("max_age", "0");
    }

    addKeycloakThemeParams(searchParams, options);

    if (provider.googleLanguage && options) {
      searchParams.set("hl", options.locale === "de" ? "de" : "en");
    }

    saveOAuthRequest(state, codeVerifier, redirectUri);
    const authUrl = `${KEYCLOAK_AUTH_URL}?${searchParams.toString()}`;
    console.info(
      `[mira-client][oauth] attempt=${preparation?.attemptId ?? "browser"} environment=${KEYCLOAK_ENVIRONMENT} provider=${provider.idpHint} accountSelection=${Boolean(provider.forceAccountSelection)} prompt=${provider.prompt ?? "none"} maxAge=${searchParams.get("max_age") ?? "none"} clientId=${KEYCLOAK_CLIENT_ID} issuer=${KEYCLOAK_ISSUER_URL} redirectUri=${redirectUri} brokerEndpoint=${KEYCLOAK_ISSUER_URL}/broker/${provider.idpHint}/endpoint`,
    );

    if (preparation) {
      const result = await invoke<OAuthStartResult>("start_oauth_window", {
        request: {
          attemptId: preparation.attemptId,
          authUrl,
          browserSecurity: options?.browserSecurity ?? defaultOAuthBrowserSecurity(),
          redirectUri,
        },
      });

      if (result.redirectUri !== redirectUri) {
        clearOAuthRequest();
        throw new Error(
          `Native OAuth callback mismatch. Expected ${redirectUri}, received ${result.redirectUri ?? "missing"}.`,
        );
      }

      if (nativeOAuthAttempt) {
        nativeOAuthAttempt.phase = "waitingForCallback";
      }
      return result;
    }

    window.location.assign(authUrl);
    return undefined;
  } catch (error) {
    clearOAuthRequest();
    await cancelNativeOAuthAttempt(preparation);
    throw error;
  }
}

export function startGoogleLogin(options?: KeycloakThemeOptions) {
  return startProviderLogin(
    {
      googleLanguage: true,
      forceAccountSelection: true,
      idpHint: "google",
      name: "Google",
      prompt: "select_account",
    },
    options,
  );
}

export function startGithubLogin(options?: KeycloakThemeOptions) {
  return startProviderLogin(
    {
      forceAccountSelection: true,
      idpHint: "github",
      name: "GitHub",
      prompt: "select_account",
    },
    options,
  );
}

export function startDiscordLogin(options?: KeycloakThemeOptions) {
  return startProviderLogin(
    {
      forceAccountSelection: true,
      idpHint: "discord",
      name: "Discord",
      prompt: "select_account",
    },
    options,
  );
}

export async function startPasswordReset(options?: KeycloakThemeOptions) {
  const preparation = await beginNativeOAuthAttempt("password-reset");
  if (isTauri() && !preparation) {
    return { ignored: true, modal: false } satisfies OAuthStartResult;
  }

  const redirectUri = preparation?.redirectUri ?? getRedirectUri();
  const passwordResetRedirectUri = preparation
    ? redirectUri
    : getPasswordResetRedirectUri(redirectUri);

  try {
    const searchParams = new URLSearchParams({
      client_id: KEYCLOAK_CLIENT_ID,
      redirect_uri: passwordResetRedirectUri,
    });

    addKeycloakThemeParams(searchParams, options);

    const resetUrl = `${KEYCLOAK_ISSUER_URL}/login-actions/reset-credentials?${searchParams.toString()}`;
    console.info(
      `[mira-client][oauth] attempt=${preparation?.attemptId ?? "browser"} passwordReset redirectUri=${passwordResetRedirectUri}`,
    );

    if (preparation) {
      const result = await invoke<OAuthStartResult>("start_oauth_window", {
        request: {
          attemptId: preparation.attemptId,
          authUrl: resetUrl,
          browserSecurity: options?.browserSecurity ?? defaultOAuthBrowserSecurity(),
          passwordReset: true,
          redirectUri,
        },
      });
      if (result.redirectUri !== redirectUri) {
        throw new Error("Native password-reset redirect URI does not match its login attempt.");
      }
      if (nativeOAuthAttempt) {
        nativeOAuthAttempt.phase = "waitingForCallback";
      }
      return result;
    }

    window.location.assign(resetUrl);
    return undefined;
  } catch (error) {
    await cancelNativeOAuthAttempt(preparation);
    throw error;
  }
}

async function getLogoutIdToken() {
  const tokens = readTokens();

  if (!tokens?.accessToken) {
    return undefined;
  }

  if (tokens.idToken) {
    return tokens.idToken;
  }

  const refreshedTokens = await refreshStoredAccessToken(tokens);

  return refreshedTokens?.idToken;
}

export async function startKeycloakLogout() {
  if (isTauri()) {
    // Do not open a provider or Keycloak logout tab from the desktop client.
    // Provider account selection is explicitly requested for every later
    // OAuth login, so a local Mira logout does not need to navigate away.
    console.info("[mira-client][oauth] logout browser=skipped");
    return;
  }

  const idToken = await getLogoutIdToken();
  const searchParams = new URLSearchParams({
    client_id: KEYCLOAK_CLIENT_ID,
  });

  if (idToken) {
    searchParams.set("id_token_hint", idToken);
  }

  const logoutUrl = `${KEYCLOAK_ISSUER_URL}/protocol/openid-connect/logout?${searchParams.toString()}`;

  window.location.assign(logoutUrl);
}

export async function completeRedirectLogin(callbackUrl?: string) {
  const url = new URL(callbackUrl ?? window.location.href);
  const code = url.searchParams.get("code");
  const state = url.searchParams.get("state");
  const error = url.searchParams.get("error_description") ?? url.searchParams.get("error");
  const savedRequest = readOAuthRequest();

  if (error) {
    clearOAuthRequest();
    if (isTauri() && error.toLowerCase().includes("redirect_uri")) {
      console.error(
        `[mira-client][oauth] Keycloak rejected redirect URI: ${savedRequest.redirectUri ?? "missing"}. Expected Keycloak Valid Redirect URI: ${NATIVE_LOOPBACK_REDIRECT_BASE}`,
      );
    }
    if (!callbackUrl) {
      window.history.replaceState({}, document.title, getRedirectUri());
    }
    throw new Error(error);
  }

  if (!code || !state) {
    return undefined;
  }

  console.info(
    `[mira-client][oauth] attempt=${nativeOAuthAttempt?.attemptId ?? "browser"} stateValidation=start`,
  );
  if (state !== savedRequest.state || !savedRequest.codeVerifier) {
    console.error(
      `[mira-client][oauth] attempt=${nativeOAuthAttempt?.attemptId ?? "browser"} stage=stateValidation result=failed`,
    );
    clearOAuthRequest();
    if (!callbackUrl) {
      window.history.replaceState({}, document.title, getRedirectUri());
    }
    throw new Error("OAuth-Antwort konnte nicht validiert werden.");
  }

  console.info(
    `[mira-client][oauth] attempt=${nativeOAuthAttempt?.attemptId ?? "browser"} stateValidation=success`,
  );

  const redirectUri = savedRequest.redirectUri ?? getRedirectUri();
  if (isTauri() && !isNativeLoopbackRedirectUri(redirectUri)) {
    clearOAuthRequest();
    throw new Error("Native OAuth callback redirect URI is missing or invalid.");
  }
  const tokens = await requestToken(
    new URLSearchParams({
      client_id: KEYCLOAK_CLIENT_ID,
      code,
      code_verifier: savedRequest.codeVerifier,
      grant_type: "authorization_code",
      redirect_uri: redirectUri,
    }),
    KEYCLOAK_CLIENT_ID,
  );

  clearOAuthRequest();
  if (!callbackUrl) {
    window.history.replaceState({}, document.title, redirectUri);
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

export async function getValidDesktopApiToken() {
  const accessToken = await getValidAccessToken();

  if (!accessToken || hasDesktopSessionClaims(accessToken)) {
    return accessToken;
  }

  const idToken = readTokens()?.idToken;

  if (idToken && hasDesktopSessionClaims(idToken)) {
    assertAccessTokenIssuer(idToken);
    return idToken;
  }

  return accessToken;
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
        tokens.idToken,
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
