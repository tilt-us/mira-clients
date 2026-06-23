import type { UserProfileResponse } from "../api/client";

type AvatarFields = {
  avatarUrl?: string;
  imageUrl?: string;
  picture?: string;
  pictureUrl?: string;
  profileImageUrl?: string;
};

export function getProfileName(profile: UserProfileResponse) {
  const displayName = profile.displayName ?? profile.preferredUsername;

  return getPublicDisplayName(displayName, "User");
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
  return (
    getSafeImageUrl(profile?.avatarUrl) ??
    getSafeImageUrl(profile?.picture) ??
    getSafeImageUrl(profile?.imageUrl) ??
    getSafeImageUrl(profile?.pictureUrl) ??
    getSafeImageUrl(profile?.profileImageUrl)
  );
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
