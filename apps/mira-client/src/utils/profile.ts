import type { UserProfileResponse } from "../api/client";

type AvatarFields = {
  avatarRightsConsented?: boolean;
  avatarRightsConsentedAt?: null | string;
  avatarUrl?: string;
  consentedAt?: null | string;
  imageUrl?: string;
  picture?: string;
  pictureUrl?: string;
  profileImageUrl?: string;
};

type LevelFields = {
  accountLevel?: unknown;
  account_level?: unknown;
  level?: unknown;
  summonerLevel?: unknown;
};

export function getProfileName(profile: UserProfileResponse) {
  const displayName = profile.displayName ?? profile.preferredUsername;

  return getPublicDisplayName(displayName, "User");
}

export function getProfileLevel(profile: UserProfileResponse) {
  const levelFields = profile as UserProfileResponse & LevelFields;
  const level =
    normalizeProfileLevel(levelFields.accountLevel) ??
    normalizeProfileLevel(levelFields.account_level) ??
    normalizeProfileLevel(levelFields.level) ??
    normalizeProfileLevel(levelFields.summonerLevel);

  return level ?? 1;
}

export function getProfileTagId(profile: UserProfileResponse) {
  return normalizeTagId(profile.tagId);
}

export function getProfileAvatarUrl(
  profile: UserProfileResponse,
  accessToken?: string,
) {
  return (
    getAvatarUrl(profile as AvatarFields) ??
    getSafeImageUrl(getTokenPicture(accessToken))
  );
}

export function getAvatarUrl(profile?: AvatarFields) {
  if (profile?.avatarRightsConsented === false) {
    return undefined;
  }

  return getRawAvatarUrl(profile);
}

export function getPublicAvatarUrl(profile?: AvatarFields) {
  if (!hasPublicAvatarConsent(profile)) {
    return undefined;
  }

  return getRawAvatarUrl(profile);
}

export function hasPublicAvatarConsent(profile?: AvatarFields) {
  return profile?.avatarRightsConsented === true || hasAvatarRightsConsentDate(profile);
}

export function hasAvatarRightsConsentDate(profile?: AvatarFields) {
  return isValidDateString(profile?.avatarRightsConsentedAt ?? profile?.consentedAt);
}

function getRawAvatarUrl(profile?: AvatarFields) {
  return (
    getSafeImageUrl(profile?.avatarUrl) ??
    getSafeImageUrl(profile?.picture) ??
    getSafeImageUrl(profile?.imageUrl) ??
    getSafeImageUrl(profile?.pictureUrl) ??
    getSafeImageUrl(profile?.profileImageUrl)
  );
}

function isValidDateString(value?: null | string) {
  if (typeof value !== "string" || !value.trim()) {
    return false;
  }

  return Number.isFinite(Date.parse(value));
}

export function getProfileInitials(profileName: string) {
  const publicName = getPublicDisplayName(profileName, "User");

  if (!publicName) {
    return "U";
  }

  return publicName[0].toUpperCase();
}

export function getPublicDisplayName(value?: string, fallback = "User") {
  const publicName = getPublicNamePart(value);

  return publicName || fallback;
}

function getPublicNamePart(value?: string) {
  const trimmedValue = value?.trim();

  if (!trimmedValue) {
    return undefined;
  }

  if (trimmedValue.includes("@")) {
    return trimmedValue.replace(/@.*/, "");
  }

  return trimmedValue;
}

function getTokenPicture(accessToken?: string) {
  const payload = getTokenPayload(accessToken);
  const picture = payload?.picture;

  return typeof picture === "string" ? picture : undefined;
}

function getTokenPayload(accessToken?: string) {
  try {
    const [, payload] = accessToken?.split(".") ?? [];

    if (!payload) {
      return undefined;
    }

    return JSON.parse(base64UrlDecode(payload)) as Record<string, unknown>;
  } catch {
    return undefined;
  }
}

function normalizeProfileLevel(value: unknown) {
  const numericValue =
    typeof value === "number"
      ? value
      : typeof value === "string"
        ? Number.parseInt(value, 10)
      : Number.NaN;

  return Number.isFinite(numericValue) && numericValue >= 0
    ? Math.floor(numericValue)
    : undefined;
}

export function normalizeTagId(value: unknown) {
  if (typeof value !== "string") {
    return undefined;
  }

  const tagId = value.trim();

  return tagId ? tagId : undefined;
}

export function formatTagId(value: unknown) {
  const tagId = normalizeTagId(value);

  return tagId ? `#${tagId}` : undefined;
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

function getSafeImageUrl(value?: string) {
  if (!value) {
    return undefined;
  }

  try {
    const url = new URL(value);

    return url.protocol === "https:" || url.protocol === "http:"
      ? url.toString()
      : undefined;
  } catch {
    return undefined;
  }
}
