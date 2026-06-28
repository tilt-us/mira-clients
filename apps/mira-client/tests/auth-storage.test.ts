import { afterEach, describe, expect, test } from "vitest";
import {
  applyAuthStorageRuntimeConfig,
  clearOAuthRequest,
  clearTokens,
  readOAuthRequest,
  readTokens,
  saveOAuthRequest,
  saveTokens,
} from "../src/auth/storage";

afterEach(() => {
  applyAuthStorageRuntimeConfig({ noSharedAuth: false });
  localStorage.clear();
  sessionStorage.clear();
});

describe("OAuth request storage", () => {
  test("saves, reads, and clears OAuth request state", () => {
    expect(readOAuthRequest()).toEqual({
      codeVerifier: null,
      redirectUri: null,
      state: null,
    });

    saveOAuthRequest("state-value", "verifier-value", "http://localhost:1420/");

    expect(readOAuthRequest()).toEqual({
      codeVerifier: "verifier-value",
      redirectUri: "http://localhost:1420/",
      state: "state-value",
    });

    saveOAuthRequest("next-state", "next-verifier");

    expect(readOAuthRequest()).toEqual({
      codeVerifier: "next-verifier",
      redirectUri: null,
      state: "next-state",
    });

    clearOAuthRequest();

    expect(readOAuthRequest()).toEqual({
      codeVerifier: null,
      redirectUri: null,
      state: null,
    });
  });
});

describe("auth token storage", () => {
  test("saves, reads, and clears auth tokens", () => {
    const tokens = {
      accessToken: "access-token",
      clientId: "mira-e2e",
      expiresAt: 1_782_384_000_000,
      refreshToken: "refresh-token",
    };

    saveTokens(tokens);
    expect(readTokens()).toEqual(tokens);

    clearTokens();
    expect(readTokens()).toBeUndefined();
  });

  test("removes invalid token JSON", () => {
    localStorage.setItem("mira.auth.tokens", "{invalid");

    expect(readTokens()).toBeUndefined();
    expect(localStorage.getItem("mira.auth.tokens")).toBeNull();
  });

  test("uses session storage when shared auth is disabled", () => {
    const tokens = {
      accessToken: "isolated-access-token",
      clientId: "mira-bevy",
    };

    applyAuthStorageRuntimeConfig({ noSharedAuth: true });
    saveTokens(tokens);

    expect(readTokens()).toEqual(tokens);
    expect(localStorage.getItem("mira.auth.tokens")).toBeNull();
    expect(sessionStorage.getItem("mira.auth.tokens")).toBe(JSON.stringify(tokens));

    clearTokens();
    expect(readTokens()).toBeUndefined();
    expect(sessionStorage.getItem("mira.auth.tokens")).toBeNull();
  });
});
