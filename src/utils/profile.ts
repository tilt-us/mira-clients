import type { UserProfileResponse } from "../api/client";

export function getProfileName(profile: UserProfileResponse) {
  const displayName = profile.displayName ?? profile.preferredUsername;

  if (!displayName) {
    return "User";
  }

  return displayName.trim().split(/\s+/)[0] || "User";
}

export function getProfileInitials(profileName: string) {
  const words = profileName
    .replace(/@.*/, "")
    .split(/[\s._-]+/)
    .filter(Boolean);

  if (words.length === 0) {
    return "U";
  }

  return words
    .slice(0, 2)
    .map((word) => word[0])
    .join("")
    .toUpperCase();
}
