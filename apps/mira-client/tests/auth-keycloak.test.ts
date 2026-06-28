import { describe, expect, test } from "vitest";
import { hasDesktopSessionClaims } from "../src/auth/keycloak";

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
