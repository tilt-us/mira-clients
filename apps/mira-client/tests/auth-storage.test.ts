import { afterEach, describe, expect, test } from "vitest";
import {
  clearOAuthRequest,
  clearTokens,
  readOAuthRequest,
  readTokens,
  saveOAuthRequest,
  saveTokens,
} from "../src/auth/storage";

afterEach(() => {
  localStorage.clear();
  sessionStorage.clear();
});

describe("OAuth request storage", () => {
  test("saves, reads, and clears OAuth request state", () => {
    expect(readOAuthRequest()).toEqual({
      codeVerifier: null,
      state: null,
    });

    saveOAuthRequest("state-value", "verifier-value");

    expect(readOAuthRequest()).toEqual({
      codeVerifier: "verifier-value",
      state: "state-value",
    });

    clearOAuthRequest();

    expect(readOAuthRequest()).toEqual({
      codeVerifier: null,
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
});
