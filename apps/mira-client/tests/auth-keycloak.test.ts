import { describe, expect, test } from "vitest";
import { hasDesktopSessionClaims } from "../src/auth/keycloak";
import {
  KEYCLOAK_CLIENT_ID,
  KEYCLOAK_ISSUER_URL,
  KEYCLOAK_PASSWORD_CLIENT_ID,
  NATIVE_LOOPBACK_REDIRECT_BASE,
  WEBSITE_URL,
  applyKeycloakRuntimeConfig,
  getBrowserRedirectUri,
} from "../src/auth/config";

function createUnsignedJwt(payload: Record<string, unknown>) {
  return [
    btoa(JSON.stringify({ alg: "none", typ: "JWT" })),
    btoa(JSON.stringify(payload)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, ""),
    "signature",
  ].join(".");
}

describe("Keycloak token helpers", () => {
  test("accepts tokens with desktop session identity claims", () => {
    const token = createUnsignedJwt({
      exp: 1_782_384_000,
      sid: "session-1",
      sub: "user-1",
    });

    expect(hasDesktopSessionClaims(token)).toBe(true);
  });

  test("rejects tokens without a subject", () => {
    const token = createUnsignedJwt({
      exp: 1_782_384_000,
      sid: "session-1",
    });

    expect(hasDesktopSessionClaims(token)).toBe(false);
  });

  test("accepts jti as session id fallback", () => {
    const token = createUnsignedJwt({
      exp: 1_782_384_000,
      jti: "token-1",
      sub: "user-1",
    });

    expect(hasDesktopSessionClaims(token)).toBe(true);
  });
});

describe("OAuth redirect configuration", () => {
  test("uses mira-bevy as the public desktop OAuth client", () => {
    expect(KEYCLOAK_CLIENT_ID).toBe("mira-bevy");
    expect(KEYCLOAK_PASSWORD_CLIENT_ID).toBe("mira-e2e");
  });

  test.each([
    ["dev", "https://dev.tilt-us.com", "https://dev-api.tilt-us.com/keycloak/realms/mira"],
    [
      "staging",
      "https://staging.tilt-us.com",
      "https://staging-api.tilt-us.com/keycloak/realms/mira",
    ],
    ["prod", "https://tilt-us.com", "https://api.tilt-us.com/keycloak/realms/mira"],
  ] as const)("keeps browser %s redirects and issuer unchanged", (environment, websiteUrl, issuer) => {
    applyKeycloakRuntimeConfig({ environment });
    expect(WEBSITE_URL).toBe(websiteUrl);
    expect(KEYCLOAK_ISSUER_URL).toBe(issuer);
    expect(getBrowserRedirectUri({ origin: websiteUrl, pathname: "/login" })).toBe(
      `${websiteUrl}/login`,
    );
  });

  test("keeps browser environment redirect origins distinct from the native callback", () => {
    applyKeycloakRuntimeConfig({ environment: "dev" });
    expect(WEBSITE_URL).toBe("https://dev.tilt-us.com");
    applyKeycloakRuntimeConfig({ environment: "staging" });
    expect(WEBSITE_URL).toBe("https://staging.tilt-us.com");
    applyKeycloakRuntimeConfig({ environment: "prod" });
    expect(WEBSITE_URL).toBe("https://tilt-us.com");
  });

  test("uses Keycloak's registered authority-only native loopback URI", () => {
    expect(NATIVE_LOOPBACK_REDIRECT_BASE).toBe("http://127.0.0.1");
    expect(NATIVE_LOOPBACK_REDIRECT_BASE).not.toContain("localhost");
    expect(NATIVE_LOOPBACK_REDIRECT_BASE.endsWith("/")).toBe(false);
  });
});
