import { beforeEach, describe, expect, test, vi } from "vitest";

const tauri = vi.hoisted(() => ({
  invoke: vi.fn(),
  isTauri: vi.fn(() => true),
}));
const http = vi.hoisted(() => ({ apiFetch: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => tauri);
vi.mock("../src/api/http", () => http);

import {
  cancelNativeOAuthLogin,
  completeNativeOAuthAttempt,
  completeRedirectLogin,
  isNativeOAuthInFlight,
  startDiscordLogin,
  startGithubLogin,
  startGoogleLogin,
  startKeycloakLogout,
} from "../src/auth/keycloak";
import { clearOAuthRequest, readOAuthRequest } from "../src/auth/storage";

const redirectUri = "http://127.0.0.1:52743";

function prepareTauriMocks() {
  let attemptId = 0;
  tauri.invoke.mockImplementation((command: string, args?: { request?: Record<string, unknown> }) => {
    if (command === "prepare_oauth_redirect_uri") {
      attemptId += 1;
      return Promise.resolve({ attemptId, redirectUri });
    }
    if (command === "start_oauth_window") {
      return Promise.resolve({ modal: false, redirectUri: args?.request?.redirectUri });
    }
    return Promise.resolve(undefined);
  });
}

function authorizationRequest() {
  const startCall = tauri.invoke.mock.calls.find(([command]) => command === "start_oauth_window");
  const request = startCall?.[1]?.request as {
    authUrl: string;
    browserSecurity: string;
    redirectUri: string;
  };
  return {
    browserSecurity: request.browserSecurity,
    redirectUri: request.redirectUri,
    url: new URL(request.authUrl),
  };
}

describe("native OAuth", () => {
  beforeEach(() => {
    completeNativeOAuthAttempt();
    clearOAuthRequest();
    tauri.invoke.mockReset();
    tauri.isTauri.mockReturnValue(true);
    http.apiFetch.mockReset();
    prepareTauriMocks();
  });

  test("allocates one slash-free redirect and opens native OAuth once", async () => {
    await startGoogleLogin();

    const { redirectUri: authorizationRedirectUri, url } = authorizationRequest();
    expect(tauri.invoke.mock.calls.filter(([command]) => command === "prepare_oauth_redirect_uri")).toHaveLength(1);
    expect(tauri.invoke.mock.calls.filter(([command]) => command === "start_oauth_window")).toHaveLength(1);
    expect(tauri.invoke.mock.calls.some(([command]) => command === "open_system_browser")).toBe(false);
    expect(authorizationRedirectUri).toBe(redirectUri);
    expect(authorizationRedirectUri.endsWith("/")).toBe(false);
    expect(url.searchParams.get("redirect_uri")).toBe(redirectUri);
    expect(url.searchParams.get("code_challenge_method")).toBe("S256");
    expect(url.searchParams.get("kc_idp_hint")).toBe("google");
    expect(url.searchParams.get("prompt")).toBe("select_account");
    expect(url.searchParams.get("max_age")).toBe("0");
    expect(readOAuthRequest().redirectUri).toBe(redirectUri);
    completeNativeOAuthAttempt();
    expect(isNativeOAuthInFlight()).toBe(false);
  });

  test("ignores a second provider click while an OAuth attempt is active", async () => {
    const first = startGoogleLogin();
    const second = await startDiscordLogin();
    await first;

    expect(second).toEqual({ ignored: true, modal: false });
    expect(tauri.invoke.mock.calls.filter(([command]) => command === "prepare_oauth_redirect_uri")).toHaveLength(1);
    expect(tauri.invoke.mock.calls.filter(([command]) => command === "start_oauth_window")).toHaveLength(1);
    expect(isNativeOAuthInFlight()).toBe(true);
  });

  test("cancels a waiting native OAuth attempt and clears its PKCE request", async () => {
    await startGoogleLogin();

    await cancelNativeOAuthLogin();

    expect(tauri.invoke).toHaveBeenCalledWith("cancel_oauth_attempt", {
      request: { attemptId: 1 },
    });
    expect(readOAuthRequest()).toEqual({
      state: null,
      codeVerifier: null,
      redirectUri: null,
    });
    expect(isNativeOAuthInFlight()).toBe(false);
  });

  test("forwards the locally selected browser security mode to native OAuth", async () => {
    await startGoogleLogin({
      accentColor: "#f2c45b",
      browserSecurity: "browser:firefox",
      locale: "en",
    });

    expect(authorizationRequest().browserSecurity).toBe("browser:firefox");
  });

  test("never opens a browser tab for desktop logout", async () => {
    await startKeycloakLogout();

    expect(tauri.invoke).not.toHaveBeenCalled();
  });

  test.each([
    ["discord", startDiscordLogin, "select_account"],
    ["github", startGithubLogin, "select_account"],
  ] as const)("uses the expected %s provider prompt", async (provider, start, prompt) => {
    await start();
    const { url } = authorizationRequest();

    expect(url.searchParams.get("kc_idp_hint")).toBe(provider);
    expect(url.searchParams.get("prompt")).toBe(prompt);
    expect(url.searchParams.get("max_age")).toBe("0");
  });

  test("uses the byte-identical stored redirect URI for token exchange", async () => {
    await startGoogleLogin();
    const savedRequest = readOAuthRequest();
    const accessToken = [
      "header",
      btoa(JSON.stringify({ iss: "https://dev-api.tilt-us.com/keycloak/realms/mira" })),
      "signature",
    ].join(".");
    http.apiFetch.mockResolvedValue(
      new Response(JSON.stringify({ access_token: accessToken }), { status: 200 }),
    );

    await completeRedirectLogin(
      `${redirectUri}/?code=authorization-code&state=${savedRequest.state}`,
    );

    const tokenBody = http.apiFetch.mock.calls[0][1].body as URLSearchParams;
    expect(tokenBody.get("redirect_uri")).toBe(savedRequest.redirectUri);
    expect(tokenBody.get("redirect_uri")).toBe(redirectUri);
    expect(tokenBody.get("redirect_uri")?.endsWith("/")).toBe(false);
  });

  test("reports a token-endpoint 404 without masking it as a JSON parsing error", async () => {
    await startGoogleLogin();
    const savedRequest = readOAuthRequest();
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    http.apiFetch.mockResolvedValue(new Response("<h1>Not Found</h1>", { status: 404 }));

    await expect(
      completeRedirectLogin(`${redirectUri}/?code=authorization-code&state=${savedRequest.state}`),
    ).rejects.toThrow("Anmeldung fehlgeschlagen.");

    expect(error).toHaveBeenCalledWith(
      expect.stringContaining(
        "stage=tokenExchange method=POST url=https://dev-api.tilt-us.com/keycloak/realms/mira/protocol/openid-connect/token status=404",
      ),
    );
    error.mockRestore();
  });

  test("releases the single-flight slot after a startup failure", async () => {
    tauri.invoke.mockImplementation((command: string) => {
      if (command === "prepare_oauth_redirect_uri") {
        return Promise.resolve({ attemptId: 1, redirectUri });
      }
      if (command === "start_oauth_window") {
        return Promise.reject(new Error("browser unavailable"));
      }
      return Promise.resolve(undefined);
    });

    await expect(startGoogleLogin()).rejects.toThrow("browser unavailable");
    expect(isNativeOAuthInFlight()).toBe(false);
  });
});
