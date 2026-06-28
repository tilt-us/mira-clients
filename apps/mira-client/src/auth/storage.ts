const AUTH_STATE_KEY = "mira.auth.state";
const AUTH_CODE_VERIFIER_KEY = "mira.auth.codeVerifier";
const AUTH_REDIRECT_URI_KEY = "mira.auth.redirectUri";
const AUTH_TOKENS_KEY = "mira.auth.tokens";

export type AuthTokens = {
  accessToken: string;
  clientId?: string;
  idToken?: string;
  refreshToken?: string;
  expiresAt?: number;
};

export function saveOAuthRequest(state: string, codeVerifier: string, redirectUri?: string) {
  sessionStorage.setItem(AUTH_STATE_KEY, state);
  sessionStorage.setItem(AUTH_CODE_VERIFIER_KEY, codeVerifier);

  if (redirectUri) {
    sessionStorage.setItem(AUTH_REDIRECT_URI_KEY, redirectUri);
  } else {
    sessionStorage.removeItem(AUTH_REDIRECT_URI_KEY);
  }
}

export function readOAuthRequest() {
  return {
    state: sessionStorage.getItem(AUTH_STATE_KEY),
    codeVerifier: sessionStorage.getItem(AUTH_CODE_VERIFIER_KEY),
    redirectUri: sessionStorage.getItem(AUTH_REDIRECT_URI_KEY),
  };
}

export function clearOAuthRequest() {
  sessionStorage.removeItem(AUTH_STATE_KEY);
  sessionStorage.removeItem(AUTH_CODE_VERIFIER_KEY);
  sessionStorage.removeItem(AUTH_REDIRECT_URI_KEY);
}

export function saveTokens(tokens: AuthTokens) {
  localStorage.setItem(AUTH_TOKENS_KEY, JSON.stringify(tokens));
}

export function readTokens(): AuthTokens | undefined {
  const rawTokens = localStorage.getItem(AUTH_TOKENS_KEY);

  if (!rawTokens) {
    return undefined;
  }

  try {
    return JSON.parse(rawTokens) as AuthTokens;
  } catch {
    localStorage.removeItem(AUTH_TOKENS_KEY);
    return undefined;
  }
}

export function clearTokens() {
  localStorage.removeItem(AUTH_TOKENS_KEY);
}
