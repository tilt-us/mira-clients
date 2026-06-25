import { describe, expect, test } from "vitest";
import {
  getAvatarUrl,
  getProfileAvatarUrl,
  getProfileInitials,
  getProfileName,
  getPublicDisplayName,
} from "../src/utils/profile";

function createUnsignedJwt(payload: Record<string, unknown>) {
  return [
    btoa(JSON.stringify({ alg: "none", typ: "JWT" })),
    btoa(JSON.stringify(payload)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, ""),
    "signature",
  ].join(".");
}

describe("profile display helpers", () => {
  test("uses display name, username, and fallback in priority order", () => {
    expect(getProfileName({ displayName: "Mira Player" })).toBe("Mira Player");
    expect(getProfileName({ preferredUsername: "test@mira.de" })).toBe("test");
    expect(getProfileName({})).toBe("User");
  });

  test("normalizes public display names and initials", () => {
    expect(getPublicDisplayName(" player@mira.de ")).toBe("player");
    expect(getPublicDisplayName("  ", "Fallback")).toBe("Fallback");
    expect(getProfileInitials("lane partner")).toBe("L");
    expect(getProfileInitials("")).toBe("U");
  });
});

describe("profile avatar helpers", () => {
  test("uses the first safe avatar field", () => {
    expect(
      getAvatarUrl({
        avatarUrl: "javascript:alert(1)",
        imageUrl: "https://cdn.mira.test/image.png",
      }),
    ).toBe("https://cdn.mira.test/image.png");
  });

  test("rejects invalid and unsafe avatar URLs", () => {
    expect(getAvatarUrl({ avatarUrl: "notaurl" })).toBeUndefined();
    expect(getAvatarUrl({ avatarUrl: "file:///tmp/avatar.png" })).toBeUndefined();
  });

  test("falls back to the picture claim in an access token", () => {
    const token = createUnsignedJwt({
      picture: "https://cdn.mira.test/token-picture.png",
    });

    expect(getProfileAvatarUrl({}, token)).toBe(
      "https://cdn.mira.test/token-picture.png",
    );
  });

  test("ignores malformed token payloads", () => {
    expect(getProfileAvatarUrl({}, "invalid.token")).toBeUndefined();
  });
});
