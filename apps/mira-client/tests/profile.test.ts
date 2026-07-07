import { describe, expect, test } from "vitest";
import {
  formatTagId,
  getAvatarUrl,
  getProfileAvatarUrl,
  getProfileInitials,
  getProfileLevel,
  getProfileName,
  getProfileTagId,
  getPublicAvatarUrl,
  getPublicDisplayName,
  hasAvatarRightsConsentDate,
  hasPublicAvatarConsent,
  normalizeTagId,
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

describe("profile level and tag helpers", () => {
  test("normalizes profile levels from supported fields", () => {
    expect(getProfileLevel({ accountLevel: 42 })).toBe(42);
    expect(getProfileLevel({ account_level: "18" })).toBe(18);
    expect(getProfileLevel({ level: 7.9 })).toBe(7);
    expect(getProfileLevel({ summonerLevel: "21abc" })).toBe(21);
  });

  test("falls back when profile levels are missing or invalid", () => {
    expect(getProfileLevel({})).toBe(1);
    expect(getProfileLevel({ level: -1 })).toBe(1);
    expect(getProfileLevel({ level: "abc" })).toBe(1);
    expect(getProfileLevel({ level: null })).toBe(1);
  });

  test("normalizes and formats tag IDs", () => {
    expect(getProfileTagId({ tagId: " MIRA " })).toBe("MIRA");
    expect(normalizeTagId(" EUW ")).toBe("EUW");
    expect(normalizeTagId("   ")).toBeUndefined();
    expect(normalizeTagId(123)).toBeUndefined();
    expect(formatTagId("MIRA")).toBe("#MIRA");
    expect(formatTagId(undefined)).toBeUndefined();
  });
});

describe("profile avatar helpers", () => {
  test("requires avatar rights consent for public avatar URLs", () => {
    expect(
      getPublicAvatarUrl({
        avatarRightsConsented: true,
        avatarUrl: "https://cdn.mira.test/public-avatar.png",
      }),
    ).toBe("https://cdn.mira.test/public-avatar.png");
    expect(
      getPublicAvatarUrl({
        avatarRightsConsentedAt: "2026-01-02T03:04:05.000Z",
        avatarUrl: "https://cdn.mira.test/dated-avatar.png",
      }),
    ).toBe("https://cdn.mira.test/dated-avatar.png");
    expect(
      getPublicAvatarUrl({
        avatarUrl: "https://cdn.mira.test/private-avatar.png",
      }),
    ).toBeUndefined();
  });

  test("detects avatar rights consent fields", () => {
    expect(hasPublicAvatarConsent({ avatarRightsConsented: true })).toBe(true);
    expect(hasPublicAvatarConsent({ consentedAt: "2026-01-02" })).toBe(true);
    expect(hasPublicAvatarConsent({ avatarRightsConsented: false })).toBe(false);
    expect(hasPublicAvatarConsent()).toBe(false);
    expect(
      hasAvatarRightsConsentDate({
        avatarRightsConsentedAt: "not-a-date",
        consentedAt: "2026-01-02",
      }),
    ).toBe(false);
  });

  test("uses the first safe avatar field", () => {
    expect(
      getAvatarUrl({
        avatarUrl: "javascript:alert(1)",
        imageUrl: "https://cdn.mira.test/image.png",
      }),
    ).toBe("https://cdn.mira.test/image.png");
  });

  test("checks all avatar field aliases in order", () => {
    expect(getAvatarUrl({ picture: "http://cdn.mira.test/picture.png" })).toBe(
      "http://cdn.mira.test/picture.png",
    );
    expect(getAvatarUrl({ pictureUrl: "https://cdn.mira.test/picture-url.png" })).toBe(
      "https://cdn.mira.test/picture-url.png",
    );
    expect(
      getAvatarUrl({
        profileImageUrl: "https://cdn.mira.test/profile-image.png",
      }),
    ).toBe("https://cdn.mira.test/profile-image.png");
    expect(getAvatarUrl()).toBeUndefined();
  });

  test("builds Discord avatar URLs from user and avatar identifiers", () => {
    expect(
      getAvatarUrl({
        discordAvatar: "avatar_hash",
        discordUserId: "1234567890",
      }),
    ).toBe("https://cdn.discordapp.com/avatars/1234567890/avatar_hash.png");
    expect(
      getAvatarUrl({
        discord_avatar: "a_animated_hash",
        discord_user_id: "1234567890",
      }),
    ).toBe("https://cdn.discordapp.com/avatars/1234567890/a_animated_hash.gif");
    expect(
      getAvatarUrl({
        discordAvatar: "../avatar",
        discordUserId: "1234567890",
      }),
    ).toBeUndefined();
  });

  test("rejects invalid and unsafe avatar URLs", () => {
    expect(getAvatarUrl({ avatarUrl: "notaurl" })).toBeUndefined();
    expect(getAvatarUrl({ avatarUrl: "file:///tmp/avatar.png" })).toBeUndefined();
    expect(
      getAvatarUrl({
        avatarRightsConsented: false,
        avatarUrl: "https://cdn.mira.test/avatar.png",
      }),
    ).toBeUndefined();
  });

  test("falls back to the picture claim in an access token", () => {
    const token = createUnsignedJwt({
      picture: "https://cdn.mira.test/token-picture.png",
    });

    expect(getProfileAvatarUrl({}, token)).toBe(
      "https://cdn.mira.test/token-picture.png",
    );
    expect(
      getProfileAvatarUrl(
        { avatarUrl: "https://cdn.mira.test/profile-avatar.png" },
        token,
      ),
    ).toBe("https://cdn.mira.test/profile-avatar.png");
  });

  test("falls back to Discord avatar claims in an access token", () => {
    const token = createUnsignedJwt({
      discordAvatar: "discord_hash",
      discordUserId: "99887766",
    });

    expect(getProfileAvatarUrl({}, token)).toBe(
      "https://cdn.discordapp.com/avatars/99887766/discord_hash.png",
    );
  });

  test("ignores malformed token payloads", () => {
    expect(getProfileAvatarUrl({}, "invalid.token")).toBeUndefined();
    expect(getProfileAvatarUrl({}, "header..signature")).toBeUndefined();
    expect(getProfileAvatarUrl({}, "header.bm90LWpzb24.signature")).toBeUndefined();
    expect(getProfileAvatarUrl({}, createUnsignedJwt({ picture: 42 }))).toBeUndefined();
  });
});
