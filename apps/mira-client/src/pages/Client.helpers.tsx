import type {
  ApiMatchResponse,
  DesktopSessionConflictEvent,
  FriendUserResponse,
  LobbyInvitation,
  LobbyMember,
  MatchLobbyResponse,
  LobbyRolesSnapshot,
  LobbySnapshot,
  MatchPlayerResponse,
  MatchResponse,
  OnlineUserStatusSnapshot,
  UpdateUserStatusRequest,
  UserStatusSnapshot,
} from "../api/client";
import { LIVE_API_BASE_URL } from "../api/config";
import { readTokens } from "../auth/storage";
import type { ChatParticipant, ChatRoom } from "../components/ChatDock";
import { getMatchTeams } from "../gameSession";
import {
  getMemberLobbyRoles,
  toApiLobbyRole,
} from "../lobbyRoles";
import type { FriendProfile, PresenceStatus, Translate } from "../types/ui";
import {
  formatTagId,
  getPublicAvatarUrl,
  getPublicDisplayName,
  normalizeTagId,
} from "../utils/profile";

type GameModeIconProps = {
  question?: boolean;
};

export const PARTY_INVITE_ONLINE_LIMIT = 2;
export const userPageCategories = [
  { disabled: false, id: "overview", labelKey: "profile-tab-overview" },
  { disabled: false, id: "champions", labelKey: "profile-tab-champions" },
  { disabled: true, id: "stats", labelKey: "profile-tab-stats" },
  { disabled: true, id: "teams", labelKey: "profile-tab-teams" },
  { disabled: true, id: "game-history", labelKey: "profile-tab-match-history" },
] as const;

export type ApiPresenceStatus = UpdateUserStatusRequest["status"];
export type UserPageCategory = (typeof userPageCategories)[number]["id"];

export type PartyInviteCandidate = {
  avatarUrl?: string;
  email?: string;
  name: string;
  publicId?: number;
  source: "friend" | "user";
  tagId?: string;
};
export type UserPageProfile = {
  avatarUrl?: string;
  level: number;
  name: string;
  publicId?: number;
  tagId?: string;
};

type TaggedFriendUser = FriendUserResponse & {
  displayTag?: unknown;
  gameTag?: unknown;
  level?: unknown;
  tag?: unknown;
  tagID?: unknown;
  tagId?: unknown;
  tag_id?: unknown;
};

type PublicAvatarFields = {
  avatarRightsConsented?: boolean;
  avatarRightsConsentedAt?: null | string;
  avatarUrl?: string;
  consentedAt?: null | string;
  imageUrl?: string;
  picture?: string;
  pictureUrl?: string;
  profileImageUrl?: string;
};

export function getUserPageNameClassName(name: string) {
  const nameLength = Array.from(name.trim()).length;

  if (nameLength > 20) {
    return "user-page-name user-page-name-overflow";
  }

  if (nameLength > 12) {
    return "user-page-name user-page-name-condensed";
  }

  if (nameLength > 8) {
    return "user-page-name user-page-name-compact";
  }

  return "user-page-name";
}

export type OnlineInviteUser = OnlineUserStatusSnapshot & Partial<FriendUserResponse>;
export type CurrentMatchPlayerProfile = {
  avatarUrl?: string;
  displayName: string;
  publicId?: number;
};
export type LobbyMemberContextMenuState = {
  left: number;
  member: LobbyMember;
  top: number;
};
export type MatchDecision = "accept" | "decline";
export type ChampionSelectionLeaveStatus = "DISCONNECTED" | "LEAVE" | "QUIT";
export type ChampionSelectionPlayerLeftEvent = {
  lobbyId?: string;
  matchId?: string;
  playerPublicId?: number;
  status?: ChampionSelectionLeaveStatus;
};
type MatchWithServerEvent = ApiMatchResponse & {
  serverEventType?: string;
};
export type PresenceSnapshot = {
  mode?: string;
  status: ApiPresenceStatus;
};

export const afkDelayMs = 5 * 60 * 1000;
export const matchAcceptTimeoutMs = 10_000;
export const matchFoundHexSpinDurationMs = 12_000;
export const matchFoundRequiredAcceptCount = 10;

export function getMatchFoundOverlayStroke(accentColor: string) {
  const hexMatch = accentColor.trim().match(/^#([0-9a-f]{3}|[0-9a-f]{6})$/i);

  if (!hexMatch) {
    return `color-mix(in srgb, ${accentColor} 72%, #ffffff)`;
  }

  const hexValue =
    hexMatch[1].length === 3
      ? hexMatch[1]
          .split("")
          .map((part) => part + part)
          .join("")
      : hexMatch[1];
  const red = Number.parseInt(hexValue.slice(0, 2), 16);
  const green = Number.parseInt(hexValue.slice(2, 4), 16);
  const blue = Number.parseInt(hexValue.slice(4, 6), 16);
  const brightness = (red * 299 + green * 587 + blue * 114) / 1000;
  const mixTarget = brightness > 220 ? 24 : 255;
  const mixAmount = brightness > 220 ? 0.32 : 0.26;
  const mixChannel = (channel: number) =>
    Math.round(channel + (mixTarget - channel) * mixAmount);

  return `rgb(${mixChannel(red)} ${mixChannel(green)} ${mixChannel(blue)})`;
}

export function getShortestRotationDegrees(rotationDegrees: number) {
  const normalizedRotation = ((rotationDegrees % 360) + 360) % 360;

  return normalizedRotation > 180
    ? normalizedRotation - 360
    : normalizedRotation;
}

export function parseApiTimestamp(value?: string) {
  if (!value) {
    return undefined;
  }

  const parsedValue = Date.parse(value);

  return Number.isFinite(parsedValue) ? parsedValue : undefined;
}

export function getDesktopSessionConflictKey(event: DesktopSessionConflictEvent) {
  return (
    event.sessionId ??
    `${event.publicId ?? ""}:${event.userId ?? ""}:${event.occurredAt ?? ""}:${
      event.deviceType ?? ""
    }:${event.reason ?? ""}`
  );
}

export function mapUserStatusToPresence(
  status?: UserStatusSnapshot["status"],
  mode?: string,
): PresenceStatus {
  const normalizedMode = mode?.toLowerCase() ?? "";

  switch (status) {
    case "ONLINE":
      return "online";
    case "AFK":
      return "afk";
    case "IN_LOBBY":
      return "inlobby";
    case "IN_QUEUE":
      return "inqueue";
    case "CHAMPION_SELECTION":
      return "championselection";
    case "IN_GAME":
      if (normalizedMode.includes("champion")) {
        return "championselection";
      }

      return "ingame";
    case "SPECTATE":
      return "ingame";
    case "OFFLINE":
    default:
      return "offline";
  }
}

export function sendPresenceKeepalive(status: ApiPresenceStatus, mode?: string) {
  const accessToken = readTokens()?.accessToken;

  void fetch(`${LIVE_API_BASE_URL}/api/user-status/me`, {
    body: JSON.stringify({ status, mode }),
    headers: {
      "Content-Type": "application/json",
      ...(accessToken ? { Authorization: `Bearer ${accessToken}` } : {}),
    },
    keepalive: true,
    method: "PUT",
  }).catch(() => {
    // The regular API path also attempts to send the status; unload keepalive is best effort.
  });
}

export function isActivePresenceStatus(status: ApiPresenceStatus | undefined) {
  return (
    status === "IN_LOBBY" ||
    status === "IN_QUEUE" ||
    status === "CHAMPION_SELECTION" ||
    status === "IN_GAME" ||
    status === "SPECTATE"
  );
}

export function isFinishedMatchStatus(status?: ApiMatchResponse["status"]) {
  return status === "ENDED" || status === "CANCELLED";
}

export function getErrorMessage(error: unknown, fallback: string) {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "string" && error.trim()) {
    return error;
  }

  if (error && typeof error === "object") {
    const errorObject = error as {
      error?: unknown;
      message?: unknown;
      status?: unknown;
    };

    for (const value of [errorObject.message, errorObject.error, errorObject.status]) {
      if (typeof value === "string" && value.trim()) {
        return value;
      }

      if (typeof value === "number") {
        return value.toString();
      }
    }

    try {
      const serializedError = JSON.stringify(errorObject);

      if (serializedError && serializedError !== "{}") {
        return serializedError;
      }
    } catch {
      // Fall through to the generic object string below.
    }

    const objectText = String(error);
    if (objectText && objectText !== "[object Object]") {
      return objectText;
    }
  }

  return fallback;
}

export function getInvitationMainInviter(invitation: LobbyInvitation) {
  return (
    invitation.inviters?.[0] ??
    (invitation.lobby ? getLobbyHost(invitation.lobby) : undefined) ??
    invitation.lobby?.members?.[0]
  );
}

export function getMemberName(member?: LobbyMember) {
  return getLobbyDisplayName(
    member?.displayName ?? `User ${member?.publicId ?? ""}`.trim(),
  );
}

export function getLobbyMemberPublicAvatarUrl(
  member?: LobbyMember | MatchPlayerResponse,
  publicUsersByPublicId?: ReadonlyMap<number, FriendUserResponse>,
) {
  const publicUser =
    typeof member?.publicId === "number"
      ? publicUsersByPublicId?.get(member.publicId)
      : undefined;

  return getPublicAvatarUrl(publicUser ?? (member as PublicAvatarFields | undefined));
}

export function getLobbyMemberTagId(member: LobbyMember) {
  const runtimeMember = member as LobbyMember & { tagId?: unknown };
  return normalizeTagId(runtimeMember.tagId);
}

export function getLobbyMemberNameTag(member: LobbyMember) {
  const tagId = getLobbyMemberTagId(member);
  const identity = getLobbyMemberDisplayIdentity(member);

  return tagId ? `${identity.name}#${tagId}` : identity.name;
}

export function getLobbyMemberDisplayIdentity(member: LobbyMember) {
  const name = getMemberName(member);
  const tagId = getLobbyMemberTagId(member);

  if (tagId) {
    const tagSuffix = `#${tagId}`;

    if (name.toLocaleLowerCase().endsWith(tagSuffix.toLocaleLowerCase())) {
      return {
        name: name.slice(0, -tagSuffix.length).trim() || name,
        tagId: formatTagId(tagId),
      };
    }

    return {
      name,
      tagId: formatTagId(tagId),
    };
  }

  const parsedName = name.match(/^(.*?)#([A-Za-z0-9]{3,5})$/);
  if (parsedName?.[1] && parsedName[2]) {
    return {
      name: parsedName[1].trim() || name,
      tagId: `#${parsedName[2]}`,
    };
  }

  return { name, tagId: undefined };
}

export function normalizeLobbyIdentityName(name: string | undefined) {
  return getLobbyDisplayName(name ?? "").trim().toLocaleLowerCase();
}

export function isSameLobbyMember(
  left: LobbyMember | undefined,
  right: LobbyMember | undefined,
) {
  if (!left || !right) {
    return false;
  }

  if (typeof left.publicId === "number" && typeof right.publicId === "number") {
    return left.publicId === right.publicId;
  }

  const leftName = normalizeLobbyIdentityName(getMemberName(left));
  const rightName = normalizeLobbyIdentityName(getMemberName(right));

  return Boolean(leftName && rightName && leftName === rightName);
}

export function getCurrentLobbyMember(
  lobby: LobbySnapshot | undefined,
  profilePublicId: number | undefined,
  profileName: string,
) {
  const members = lobby?.members ?? [];

  if (typeof profilePublicId === "number") {
    const currentMemberByPublicId = members.find((member) => {
      return member.publicId === profilePublicId;
    });

    if (currentMemberByPublicId) {
      return currentMemberByPublicId;
    }
  }

  const normalizedProfileName = normalizeLobbyIdentityName(profileName);

  if (normalizedProfileName) {
    const currentMemberByName = members.find((member) => {
      return normalizeLobbyIdentityName(getMemberName(member)) === normalizedProfileName;
    });

    if (currentMemberByName) {
      return currentMemberByName;
    }
  }

  return members.length === 1 ? members[0] : undefined;
}

export function getFriendUserName(user: FriendUserResponse) {
  return stripNameTagSuffix(getPublicDisplayName(
    user.displayName,
    `User ${user.publicId ?? ""}`.trim(),
  ));
}

export function getFriendUserTagId(user: Partial<TaggedFriendUser>) {
  return normalizeTagId(
    user.tagId ??
      user.tag_id ??
      user.tagID ??
      user.tag ??
      user.displayTag ??
      user.gameTag,
  ) ?? parseNameTagSuffix(user.displayName);
}

function parseNameTagSuffix(value: unknown) {
  if (typeof value !== "string") {
    return undefined;
  }

  const match = value.trim().match(/#([A-Za-z0-9_-]+)$/);

  return match ? normalizeTagId(match[1]) : undefined;
}

function stripNameTagSuffix(value: string) {
  return value.replace(/#[A-Za-z0-9_-]+$/, "").trim() || value;
}

export function getFriendUserLevel(user: Partial<TaggedFriendUser>) {
  const level =
    typeof user.level === "number"
      ? user.level
      : typeof user.level === "string"
        ? Number.parseInt(user.level, 10)
        : Number.NaN;

  return Number.isFinite(level) && level >= 0 ? Math.floor(level) : undefined;
}

export function mapFriendToInviteCandidate(friend: FriendProfile): PartyInviteCandidate {
  return {
    avatarUrl: friend.avatarUrl,
    email: friend.email,
    name: friend.name,
    publicId: friend.publicId,
    source: "friend",
    tagId: friend.tagId,
  };
}

export function isInviteablePresence(status: PresenceStatus) {
  return status !== "offline";
}

export function mapUserToInviteCandidate(user: FriendUserResponse): PartyInviteCandidate {
  const publicId = toPublicId(user.publicId);

  return {
    avatarUrl: getPublicAvatarUrl(user as PublicAvatarFields),
    email: user.email,
    name: getFriendUserName(user),
    publicId,
    source: "user",
    tagId: getFriendUserTagId(user),
  };
}

export function mapOnlineUserToInviteCandidate(user: OnlineInviteUser): PartyInviteCandidate {
  const publicId = toPublicId(user.publicId);

  return {
    avatarUrl: getPublicAvatarUrl(user as PublicAvatarFields),
    email: user.email,
    name: getPublicDisplayName(
      user.displayName,
      `User ${publicId ?? ""}`.trim(),
    ),
    publicId,
    source: "user",
    tagId: getFriendUserTagId(user),
  };
}

export function mapFriendUserToProfile(
  user: FriendUserResponse,
  userStatus?: UserStatusSnapshot,
  onlinePublicIds: ReadonlySet<number> = new Set(),
): FriendProfile {
  const publicId = toPublicId(user.publicId);
  const status =
    userStatus?.status !== undefined
      ? mapUserStatusToPresence(userStatus.status, userStatus.mode)
      : typeof publicId === "number" && onlinePublicIds.has(publicId)
        ? "online"
        : "offline";

  return {
    avatarUrl: getPublicAvatarUrl(user as PublicAvatarFields),
    email: user.email,
    id: String(publicId ?? user.email ?? user.displayName ?? "unknown-user"),
    level: getFriendUserLevel(user),
    name: getFriendUserName(user),
    publicId,
    status,
    tagId: getFriendUserTagId(user),
    rank: {
      name: "wood",
      label: "Wood",
      tier: "I",
    },
  };
}

export function getInviteCandidateKey(candidate: PartyInviteCandidate) {
  return candidate.publicId ?? candidate.email ?? candidate.name;
}

export function getInviteCandidateSubtitle(
  candidate: PartyInviteCandidate,
  options: { showEmail: boolean } = { showEmail: true },
) {
  const parts = [
    formatTagId(candidate.tagId),
    options.showEmail ? candidate.email : undefined,
  ].filter((part): part is string => Boolean(part));

  return parts.length > 0 ? parts.join(" · ") : undefined;
}

export function getLobbyDisplayName(name: string) {
  return name.trim() || name;
}

export function getInvitationModeLabel(invitation: LobbyInvitation) {
  return invitation.mode === "RANKED" || invitation.lobby?.mode === "RANKED"
    ? "Ranked"
    : "Normal";
}

export function getLobbyModeLabel(lobby: LobbySnapshot | undefined, t: Translate) {
  return lobby?.mode === "RANKED" ? t("game-mode-ranked") : t("game-mode-normal");
}

export function getLobbyLeaveColor(accentColor: string) {
  const red = Number.parseInt(accentColor.slice(1, 3), 16) / 255;
  const green = Number.parseInt(accentColor.slice(3, 5), 16) / 255;
  const blue = Number.parseInt(accentColor.slice(5, 7), 16) / 255;
  const max = Math.max(red, green, blue);
  const min = Math.min(red, green, blue);
  const delta = max - min;
  const hue =
    delta === 0
      ? 0
      : max === red
        ? ((green - blue) / delta + (green < blue ? 6 : 0)) * 60
        : max === green
          ? ((blue - red) / delta + 2) * 60
          : ((red - green) / delta + 4) * 60;

  return hue <= 24 || hue >= 342 ? "#ff8a2a" : "#ff3f46";
}

export function formatLobbySearchTime(totalSeconds: number) {
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const paddedMinutes = String(minutes).padStart(2, "0");
  const paddedSeconds = String(seconds).padStart(2, "0");

  if (hours > 0) {
    return `${String(hours).padStart(2, "0")}:${paddedMinutes}:${paddedSeconds}`;
  }

  return `${paddedMinutes}:${paddedSeconds}`;
}

export function isMatchForLobby(match: ApiMatchResponse, lobbyId?: string) {
  return Boolean(
    lobbyId && match.lobbies?.some((lobby) => lobby.lobbyId === lobbyId),
  );
}

export function isMatchReady(match: ApiMatchResponse) {
  const acceptances = match.acceptances ?? [];
  const playerPublicIds = getMatchPlayerPublicIds(match);
  const acceptedPublicIds = new Set(
    acceptances
      .filter((acceptance) => acceptance.status === "ACCEPTED")
      .map((acceptance) => acceptance.playerPublicId)
      .filter((publicId): publicId is number => typeof publicId === "number"),
  );
  const hasServerPhaseTiming = Boolean(match.serverNow && match.phaseEndsAt);
  const allKnownPlayersAccepted =
    playerPublicIds.length > 0 &&
    playerPublicIds.every((publicId) => acceptedPublicIds.has(publicId));

  return (
    match.status === "CHAMPION_SELECTION" ||
    match.status === "READY" ||
    (match.status === "PENDING_ACCEPTANCE" &&
      hasServerPhaseTiming &&
      allKnownPlayersAccepted)
  );
}

export function getMatchServerEventType(match: ApiMatchResponse | undefined) {
  return (match as MatchWithServerEvent | undefined)?.serverEventType;
}

export function isWarmupMatch(
  match: ApiMatchResponse | undefined,
): match is ApiMatchResponse {
  const phase = match?.phase?.trim().toUpperCase();

  if (phase) {
    return phase === "PENDING_ACCEPTANCE" || phase === "WARMUP";
  }

  return (
    match?.status === "PENDING_ACCEPTANCE" ||
    (match?.status === "CHAMPION_SELECTION" &&
      getMatchServerEventType(match) === "MATCH_CHAMPION_SELECTION_STARTED")
  );
}

export function isWarmupActive(match: ApiMatchResponse | undefined) {
  if (!isWarmupMatch(match)) {
    return false;
  }

  const phaseEndsAt = parseApiTimestamp(match.phaseEndsAt);

  return phaseEndsAt !== undefined && phaseEndsAt > Date.now();
}

export function shouldTreatChampionSelectionAsWarmup(match: ApiMatchResponse) {
  const phase = match.phase?.trim().toUpperCase();

  if (phase) {
    return phase === "WARMUP" || phase === "PENDING_ACCEPTANCE";
  }

  return (
    match.status === "CHAMPION_SELECTION" &&
    Boolean(match.phaseEndsAt) &&
    (match.championSelections?.length ?? 0) === 0 &&
    (getMatchServerEventType(match) === "MATCH_CHAMPION_SELECTION_STARTED" ||
      getMatchServerEventType(match) === undefined)
  );
}

export function isMatchGameStarted(match: ApiMatchResponse | undefined) {
  return match?.status === "READY";
}

export function normalizeRoleAssignmentSource(value: unknown): MatchPlayerResponse["roleAssignmentSource"] {
  return value === "PRIMARY" || value === "SECONDARY" || value === "AUTOFILL"
    ? value
    : undefined;
}

export function normalizeMatchResponse(match: MatchResponse): ApiMatchResponse {
  return {
    ...match,
    lobbies: match.lobbies?.map((lobby) => ({
      ...lobby,
      players: lobby.players?.map((player) => ({
        ...player,
        roleAssignmentSource: normalizeRoleAssignmentSource(
          player.roleAssignmentSource,
        ),
      })),
    })),
    roleAssignments: match.roleAssignments?.map((assignment) => ({
      ...assignment,
      source: normalizeRoleAssignmentSource(assignment.source),
    })),
  };
}

export function getMatchPlayerPublicIds(match: ApiMatchResponse) {
  return (
    match.lobbies
      ?.flatMap((lobby) => lobby.players ?? [])
      .map((player) => player.publicId)
      .filter((publicId): publicId is number => typeof publicId === "number") ?? []
  );
}

export function getLobbyChatRoom(
  lobby: LobbySnapshot | undefined,
  t: Translate,
  currentPlayerProfile?: CurrentMatchPlayerProfile,
  publicUsersByPublicId?: ReadonlyMap<number, FriendUserResponse>,
): ChatRoom | undefined {
  if (!lobby?.id || (lobby.members?.length ?? 0) === 0) {
    return undefined;
  }

  return {
    contextId: lobby.id,
    id: `lobby:${lobby.id}`,
    locked: true,
    name: t("chat-lobby-title"),
    participants: getLobbyChatParticipants(
      lobby.members ?? [],
      currentPlayerProfile,
      publicUsersByPublicId,
    ),
    subtitle: t("chat-lobby-subtitle"),
    type: "lobby",
  };
}

function getLobbyChatParticipants(
  members: LobbyMember[],
  currentPlayerProfile?: CurrentMatchPlayerProfile,
  publicUsersByPublicId?: ReadonlyMap<number, FriendUserResponse>,
): ChatParticipant[] {
  return members
    .filter((member) => typeof member.publicId === "number")
    .map((member) => {
      const isCurrentPlayer = member.publicId === currentPlayerProfile?.publicId;
      const name = getPublicDisplayName(
        isCurrentPlayer ? currentPlayerProfile?.displayName : member.displayName,
        "User",
      );

      return {
        avatarUrl: isCurrentPlayer
          ? currentPlayerProfile?.avatarUrl
          : getLobbyMemberPublicAvatarUrl(member, publicUsersByPublicId),
        name,
        publicId: member.publicId as number,
      };
    });
}

export function getChampionSelectionTeamChatRoom(
  match: ApiMatchResponse | undefined,
  currentPlayerProfile: CurrentMatchPlayerProfile | undefined,
  t: Translate,
  publicUsersByPublicId?: ReadonlyMap<number, FriendUserResponse>,
): ChatRoom | undefined {
  if (!match?.matchId || typeof currentPlayerProfile?.publicId !== "number") {
    return undefined;
  }

  const teams = getMatchTeams(match);
  const teamIndex = teams.findIndex((team) => {
    return team.players?.some((player) => player.publicId === currentPlayerProfile.publicId);
  });

  if (teamIndex < 0) {
    return undefined;
  }

  const team = teamIndex === 0 ? "Dark" : "Light";

  return {
    contextId: match.matchId,
    id: `team:${match.matchId}:${teamIndex}`,
    locked: true,
    name: t("chat-team-title"),
    participants: getTeamChatParticipants(
      teams[teamIndex]?.players ?? [],
      currentPlayerProfile,
      publicUsersByPublicId,
    ),
    subtitle: t("chat-team-subtitle"),
    team,
    type: "team",
  };
}

export function getChampionSelectionLobbyChatRoom(
  match: ApiMatchResponse | undefined,
  currentPlayerProfile: CurrentMatchPlayerProfile | undefined,
  t: Translate,
  publicUsersByPublicId?: ReadonlyMap<number, FriendUserResponse>,
): ChatRoom | undefined {
  if (!match?.matchId || typeof currentPlayerProfile?.publicId !== "number") {
    return undefined;
  }

  const lobby = match.lobbies?.find((currentLobby) => {
    return currentLobby.players?.some(
      (player) => player.publicId === currentPlayerProfile.publicId,
    );
  });

  if (!lobby?.lobbyId) {
    return undefined;
  }

  return {
    contextId: lobby.lobbyId,
    id: `lobby:${lobby.lobbyId}`,
    locked: true,
    name: t("chat-lobby-title"),
    participants: getMatchLobbyChatParticipants(
      lobby,
      currentPlayerProfile,
      publicUsersByPublicId,
    ),
    subtitle: t("chat-lobby-subtitle"),
    type: "lobby",
  };
}

function getMatchLobbyChatParticipants(
  lobby: MatchLobbyResponse,
  currentPlayerProfile: CurrentMatchPlayerProfile,
  publicUsersByPublicId?: ReadonlyMap<number, FriendUserResponse>,
): ChatParticipant[] {
  return getTeamChatParticipants(
    lobby.players ?? [],
    currentPlayerProfile,
    publicUsersByPublicId,
  );
}

function getTeamChatParticipants(
  players: MatchPlayerResponse[],
  currentPlayerProfile: CurrentMatchPlayerProfile,
  publicUsersByPublicId?: ReadonlyMap<number, FriendUserResponse>,
): ChatParticipant[] {
  return players
    .filter((player) => typeof player.publicId === "number")
    .map((player) => {
      const isCurrentPlayer = player.publicId === currentPlayerProfile.publicId;
      const name = getPublicDisplayName(
        isCurrentPlayer ? currentPlayerProfile.displayName : player.displayName,
        "User",
      );

      return {
        avatarUrl: isCurrentPlayer
          ? currentPlayerProfile.avatarUrl
          : getLobbyMemberPublicAvatarUrl(player, publicUsersByPublicId),
        name,
        publicId: player.publicId as number,
      };
    });
}

export function areAllChampionsSelected(match: ApiMatchResponse) {
  const playerPublicIds = getMatchPlayerPublicIds(match);
  const selectedPublicIds = new Set(
    match.championSelections
      ?.map((selection) => selection.playerPublicId)
      .filter((publicId): publicId is number => typeof publicId === "number") ?? [],
  );

  return (
    playerPublicIds.length > 0 &&
    playerPublicIds.every((publicId) => selectedPublicIds.has(publicId))
  );
}

export function mergeMatchChampionHovers(
  match: ApiMatchResponse,
  hovers?: ApiMatchResponse["championHovers"],
): ApiMatchResponse {
  return {
    ...match,
    championHovers: hovers ?? [],
  };
}

export function isGenericPlayerName(value?: string) {
  return /^(player|user)(?:\s+\d+)?$/i.test(value?.trim() ?? "");
}

export function mapLobbyToMatchPlayers(
  lobby: LobbySnapshot,
  currentPlayerProfile?: CurrentMatchPlayerProfile,
  publicUsersByPublicId?: ReadonlyMap<number, FriendUserResponse>,
) {
  return (
    lobby.members
      ?.filter((member) => typeof member.publicId === "number")
      .map((member) => {
        const isCurrentPlayer = member.publicId === currentPlayerProfile?.publicId;
        const displayName = isCurrentPlayer
          ? currentPlayerProfile?.displayName
          : member.displayName;
        const avatarUrl = isCurrentPlayer
          ? currentPlayerProfile?.avatarUrl
          : getLobbyMemberPublicAvatarUrl(member, publicUsersByPublicId);
        const roles = getMemberLobbyRoles(member);

        return {
          publicId: member.publicId as number,
          displayName: getPublicDisplayName(displayName, "User"),
          avatarUrl,
          ...(roles[0]
            ? { primaryRole: toApiLobbyRole(roles[0]) }
            : {}),
          ...(roles[1]
            ? { secondaryRole: toApiLobbyRole(roles[1]) }
            : {}),
        };
      }) ?? []
  );
}

export function mergeKnownMatchPlayer(
  player: MatchPlayerResponse,
  knownPlayer?: MatchPlayerResponse,
): MatchPlayerResponse {
  const playerDisplayName = !isGenericPlayerName(player.displayName)
    ? getPublicDisplayName(player.displayName, "")
    : undefined;
  const knownDisplayName = !isGenericPlayerName(knownPlayer?.displayName)
    ? getPublicDisplayName(knownPlayer?.displayName, "")
    : undefined;

  return {
    ...player,
    displayName:
      playerDisplayName ??
      knownDisplayName ??
      "User",
    avatarUrl: player.avatarUrl ?? knownPlayer?.avatarUrl,
  };
}

export function enrichMatchPlayers(
  match: ApiMatchResponse,
  knownPlayers: Map<number, MatchPlayerResponse>,
) {
  return {
    ...match,
    lobbies: match.lobbies?.map((lobby) => ({
      ...lobby,
      players: lobby.players?.map((player) => {
        const knownPlayer =
          typeof player.publicId === "number"
            ? knownPlayers.get(player.publicId)
            : undefined;

        return mergeKnownMatchPlayer(player, knownPlayer);
      }),
    })),
  };
}

export function toPublicId(value: unknown) {
  if (typeof value === "number") {
    return value;
  }

  if (typeof value === "string") {
    const parsedValue = Number.parseInt(value, 10);

    return Number.isNaN(parsedValue) ? undefined : parsedValue;
  }

  return undefined;
}

export function normalizeLobbyMember(member: LobbyMember): LobbyMember {
  return {
    ...member,
    publicId: toPublicId(member.publicId),
  };
}

export function normalizeLobbySnapshot(lobby?: LobbySnapshot): LobbySnapshot | undefined {
  if (!lobby) {
    return undefined;
  }

  return {
    ...lobby,
    ownerPublicId: toPublicId(lobby.ownerPublicId),
    members: lobby.members?.map(normalizeLobbyMember),
  };
}

export function normalizeLobbyInvitation(invitation: LobbyInvitation): LobbyInvitation {
  const lobby = normalizeLobbySnapshot(invitation.lobby);
  const invitationWithAliases = invitation as LobbyInvitation & {
    id?: unknown;
    lobby_id?: unknown;
    invitee_public_id?: unknown;
  };
  const lobbyId =
    invitation.lobbyId ??
    (typeof invitationWithAliases.lobby_id === "string"
      ? invitationWithAliases.lobby_id
      : undefined) ??
    lobby?.id ??
    (typeof invitationWithAliases.id === "string"
      ? invitationWithAliases.id
      : undefined);
  const inviteePublicId =
    toPublicId(invitation.inviteePublicId) ??
    toPublicId(invitationWithAliases.invitee_public_id);

  return {
    ...invitation,
    inviteePublicId,
    lobby,
    lobbyId,
    inviters: invitation.inviters?.map(normalizeLobbyMember),
  };
}

export function shouldShowLobbyInvitation(
  invitation: LobbyInvitation,
  activeLobbyId: string | undefined,
  profilePublicId: number | undefined,
) {
  const mainInviter = getInvitationMainInviter(invitation);

  return (
    Boolean(invitation.lobbyId) &&
    invitation.lobbyId !== activeLobbyId &&
    (invitation.inviteePublicId === undefined ||
      invitation.inviteePublicId === profilePublicId) &&
    mainInviter?.publicId !== profilePublicId &&
    invitation.lobby?.ownerPublicId !== profilePublicId
  );
}

export function mergeLobbyInvitations(
  currentInvitations: LobbyInvitation[],
  nextInvitations: LobbyInvitation[],
  activeLobbyId: string | undefined,
  profilePublicId: number | undefined,
) {
  const invitationsByLobbyId = new Map<string, LobbyInvitation>();

  for (const invitation of currentInvitations) {
    const normalizedInvitation = normalizeLobbyInvitation(invitation);

    if (
      normalizedInvitation.lobbyId &&
      shouldShowLobbyInvitation(normalizedInvitation, activeLobbyId, profilePublicId)
    ) {
      invitationsByLobbyId.set(normalizedInvitation.lobbyId, normalizedInvitation);
    }
  }

  for (const invitation of nextInvitations) {
    const normalizedInvitation = normalizeLobbyInvitation(invitation);

    if (
      normalizedInvitation.lobbyId &&
      shouldShowLobbyInvitation(normalizedInvitation, activeLobbyId, profilePublicId)
    ) {
      invitationsByLobbyId.delete(normalizedInvitation.lobbyId);
      invitationsByLobbyId.set(normalizedInvitation.lobbyId, normalizedInvitation);
    }
  }

  return [...invitationsByLobbyId.values()].sort((left, right) => {
    const leftUpdatedAt = left.updatedAt ? Date.parse(left.updatedAt) : 0;
    const rightUpdatedAt = right.updatedAt ? Date.parse(right.updatedAt) : 0;

    return rightUpdatedAt - leftUpdatedAt;
  });
}

export function getLobbySlotMembers(lobby: LobbySnapshot) {
  const members = lobby.members ?? [];
  const host = getLobbyHost(lobby);
  const otherMembers = members.filter((member) => {
    return member.publicId !== host?.publicId;
  });
  const slotMembers: Array<LobbyMember | undefined> = [];
  const sideSlots = [0, 1, 3, 4];

  if (host) {
    slotMembers[2] = host;
  }

  for (const [index, member] of otherMembers.entries()) {
    const slot = sideSlots[index];

    if (slot === undefined) {
      break;
    }

    slotMembers[slot] = member;
  }

  return slotMembers;
}

export function getLobbyHost(lobby: LobbySnapshot) {
  const members = lobby.members ?? [];
  const owner = members.find((member) => member.publicId === lobby.ownerPublicId);

  if (owner) {
    return owner;
  }

  const joinedMembers = members
    .map((member) => {
      const joinedAt = member.joinedAt ? Date.parse(member.joinedAt) : Number.NaN;

      return {
        joinedAt,
        member,
      };
    })
    .filter(({ joinedAt }) => Number.isFinite(joinedAt))
    .sort((left, right) => left.joinedAt - right.joinedAt);

  if (joinedMembers[0]) {
    return joinedMembers[0].member;
  }

  return members[0];
}

export function findLobbyInvitation(value: unknown, depth = 0): LobbyInvitation | undefined {
  if (!value || depth > 5) {
    return undefined;
  }

  if (typeof value === "string") {
    try {
      return findLobbyInvitation(JSON.parse(value) as unknown, depth + 1);
    } catch {
      return undefined;
    }
  }

  if (Array.isArray(value)) {
    for (const item of value) {
      const invitation = findLobbyInvitation(item, depth + 1);

      if (invitation) {
        return invitation;
      }
    }

    return undefined;
  }

  if (typeof value !== "object") {
    return undefined;
  }

  const record = value as Record<string, unknown>;
  const lobby = record.lobby;
  const lobbyRecord =
    lobby && typeof lobby === "object" ? (lobby as Record<string, unknown>) : undefined;

  const recordLobbyId =
    typeof record.lobbyId === "string"
      ? record.lobbyId
      : typeof record.lobby_id === "string"
        ? record.lobby_id
        : undefined;

  if (
    recordLobbyId &&
    ("inviteePublicId" in record ||
      "invitee_public_id" in record ||
      "inviters" in record ||
      "lobby" in record ||
      "updatedAt" in record) &&
    !("players" in record)
  ) {
    return normalizeLobbyInvitation({
      ...(record as LobbyInvitation),
      lobbyId: recordLobbyId,
    });
  }

  if (
    typeof lobbyRecord?.id === "string" &&
    ("inviteePublicId" in record ||
      "invitee_public_id" in record ||
      "inviters" in record ||
      "updatedAt" in record ||
      "mode" in record)
  ) {
    return normalizeLobbyInvitation({
      ...(record as LobbyInvitation),
      lobbyId: lobbyRecord.id,
    });
  }

  for (const nestedValue of Object.values(record)) {
    const invitation = findLobbyInvitation(nestedValue, depth + 1);

    if (invitation) {
      return invitation;
    }
  }

  return undefined;
}

export function findLobbySnapshot(value: unknown, depth = 0): LobbySnapshot | undefined {
  if (!value || depth > 5) {
    return undefined;
  }

  if (typeof value === "string") {
    try {
      return findLobbySnapshot(JSON.parse(value) as unknown, depth + 1);
    } catch {
      return undefined;
    }
  }

  if (Array.isArray(value)) {
    for (const item of value) {
      const snapshot = findLobbySnapshot(item, depth + 1);

      if (snapshot) {
        return snapshot;
      }
    }

    return undefined;
  }

  if (typeof value !== "object") {
    return undefined;
  }

  const record = value as Record<string, unknown>;

  if (typeof record.id === "string" && Array.isArray(record.members)) {
    return normalizeLobbySnapshot(record as LobbySnapshot);
  }

  for (const nestedValue of Object.values(record)) {
    const snapshot = findLobbySnapshot(nestedValue, depth + 1);

    if (snapshot) {
      return snapshot;
    }
  }

  return undefined;
}

export function findLobbyRolesSnapshot(value: unknown, depth = 0): LobbyRolesSnapshot | undefined {
  if (!value || depth > 5) {
    return undefined;
  }

  if (typeof value === "string") {
    try {
      return findLobbyRolesSnapshot(JSON.parse(value) as unknown, depth + 1);
    } catch {
      return undefined;
    }
  }

  if (Array.isArray(value)) {
    for (const item of value) {
      const snapshot = findLobbyRolesSnapshot(item, depth + 1);

      if (snapshot) {
        return snapshot;
      }
    }

    return undefined;
  }

  if (typeof value !== "object") {
    return undefined;
  }

  const record = value as Record<string, unknown>;

  if (
    typeof record.lobbyId === "string" &&
    Array.isArray(record.members) &&
    record.members.some((member) => {
      return (
        member &&
        typeof member === "object" &&
        ("primaryRole" in member || "secondaryRole" in member)
      );
    })
  ) {
    return record as LobbyRolesSnapshot;
  }

  for (const nestedValue of Object.values(record)) {
    const snapshot = findLobbyRolesSnapshot(nestedValue, depth + 1);

    if (snapshot) {
      return snapshot;
    }
  }

  return undefined;
}

export function findUserStatusSnapshot(value: unknown, depth = 0): UserStatusSnapshot | undefined {
  if (!value || depth > 5) {
    return undefined;
  }

  if (typeof value === "string") {
    try {
      return findUserStatusSnapshot(JSON.parse(value) as unknown, depth + 1);
    } catch {
      return undefined;
    }
  }

  if (Array.isArray(value)) {
    for (const item of value) {
      const snapshot = findUserStatusSnapshot(item, depth + 1);

      if (snapshot) {
        return snapshot;
      }
    }

    return undefined;
  }

  if (typeof value !== "object") {
    return undefined;
  }

  const record = value as Record<string, unknown>;

  if (
    typeof record.publicId === "number" &&
    typeof record.status === "string" &&
    ("mode" in record || "updatedAt" in record)
  ) {
    return record as UserStatusSnapshot;
  }

  for (const nestedValue of Object.values(record)) {
    const snapshot = findUserStatusSnapshot(nestedValue, depth + 1);

    if (snapshot) {
      return snapshot;
    }
  }

  return undefined;
}

export function findDesktopSessionConflictEvent(
  value: unknown,
  depth = 0,
): DesktopSessionConflictEvent | undefined {
  if (!value || depth > 5) {
    return undefined;
  }

  if (typeof value === "string") {
    try {
      return findDesktopSessionConflictEvent(JSON.parse(value) as unknown, depth + 1);
    } catch {
      return undefined;
    }
  }

  if (Array.isArray(value)) {
    for (const item of value) {
      const event = findDesktopSessionConflictEvent(item, depth + 1);

      if (event) {
        return event;
      }
    }

    return undefined;
  }

  if (typeof value !== "object") {
    return undefined;
  }

  const record = value as Record<string, unknown>;
  const eventType = record.type ?? record.event ?? record.name;

  if (eventType === "AUTH_SESSION_CONFLICT") {
    const payload = record.payload ?? record.data;

    if (payload && typeof payload === "object") {
      return payload as DesktopSessionConflictEvent;
    }

    return record as DesktopSessionConflictEvent;
  }

  if (
    (typeof record.publicId === "number" ||
      typeof record.userId === "string" ||
      typeof record.sessionId === "string") &&
    (typeof record.reason === "string" ||
      typeof record.occurredAt === "string" ||
      typeof record.deviceType === "string") &&
    ("sourceIp" in record || "userAgent" in record || "reason" in record)
  ) {
    return record as DesktopSessionConflictEvent;
  }

  for (const nestedValue of Object.values(record)) {
    const event = findDesktopSessionConflictEvent(nestedValue, depth + 1);

    if (event) {
      return event;
    }
  }

  return undefined;
}

export function getWireEventType(record: Record<string, unknown>) {
  return [record.type, record.event, record.eventType, record.eventName, record.name].find(
    (currentValue): currentValue is string => typeof currentValue === "string",
  );
}

export function withServerEventType(
  match: ApiMatchResponse,
  serverEventType?: string,
): ApiMatchResponse {
  return serverEventType
    ? ({
        ...match,
        serverEventType,
      } as MatchWithServerEvent)
    : match;
}

export function findMatchResponse(
  value: unknown,
  depth = 0,
  serverEventType?: string,
): ApiMatchResponse | undefined {
  if (!value || depth > 5) {
    return undefined;
  }

  if (typeof value === "string") {
    try {
      return findMatchResponse(JSON.parse(value) as unknown, depth + 1, serverEventType);
    } catch {
      return undefined;
    }
  }

  if (Array.isArray(value)) {
    for (const item of value) {
      const match = findMatchResponse(item, depth + 1, serverEventType);

      if (match) {
        return match;
      }
    }

    return undefined;
  }

  if (typeof value !== "object") {
    return undefined;
  }

  const record = value as Record<string, unknown>;
  const currentEventType = getWireEventType(record) ?? serverEventType;
  const payload = record.payload ?? record.data;

  if (payload && typeof payload === "object") {
    const payloadMatch = findMatchResponse(payload, depth + 1, currentEventType);

    if (payloadMatch) {
      return payloadMatch;
    }
  }

  if (
    typeof record.matchId === "string" &&
    typeof record.status === "string" &&
    Array.isArray(record.lobbies)
  ) {
    return withServerEventType(
      normalizeMatchResponse(record as MatchResponse),
      currentEventType,
    );
  }

  if (
    typeof record.matchId === "string" &&
    (record.status === "CANCELLED" || record.status === "ENDED")
  ) {
    return withServerEventType(
      normalizeMatchResponse(record as MatchResponse),
      currentEventType,
    );
  }

  for (const nestedValue of Object.values(record)) {
    const match = findMatchResponse(nestedValue, depth + 1, currentEventType);

    if (match) {
      return match;
    }
  }

  return undefined;
}

export function isChampionSelectionLeaveStatus(
  value: unknown,
): value is ChampionSelectionLeaveStatus {
  if (typeof value !== "string") {
    return false;
  }

  const normalizedValue = value.toUpperCase();

  return (
    normalizedValue === "DISCONNECTED" ||
    normalizedValue === "LEAVE" ||
    normalizedValue === "QUIT"
  );
}

export function normalizeChampionSelectionLeaveStatus(
  value: unknown,
): ChampionSelectionLeaveStatus | undefined {
  if (!isChampionSelectionLeaveStatus(value)) {
    return undefined;
  }

  return value.toUpperCase() as ChampionSelectionLeaveStatus;
}

export function toChampionSelectionPlayerLeftEvent(
  value: unknown,
): ChampionSelectionPlayerLeftEvent | undefined {
  if (!value || typeof value !== "object") {
    return undefined;
  }

  const record = value as Record<string, unknown>;
  const playerPublicId = toPublicId(record.playerPublicId);
  const status = normalizeChampionSelectionLeaveStatus(record.status);

  if (typeof record.matchId === "string" && status) {
    return {
      lobbyId: typeof record.lobbyId === "string" ? record.lobbyId : undefined,
      matchId: record.matchId,
      playerPublicId,
      status,
    };
  }

  return undefined;
}

export function findChampionSelectionPlayerLeftEvent(
  value: unknown,
  depth = 0,
  insidePlayerLeftEvent = false,
): ChampionSelectionPlayerLeftEvent | undefined {
  if (!value || depth > 5) {
    return undefined;
  }

  if (typeof value === "string") {
    try {
      return findChampionSelectionPlayerLeftEvent(
        JSON.parse(value) as unknown,
        depth + 1,
        insidePlayerLeftEvent,
      );
    } catch {
      return undefined;
    }
  }

  if (Array.isArray(value)) {
    for (const item of value) {
      const event = findChampionSelectionPlayerLeftEvent(
        item,
        depth + 1,
        insidePlayerLeftEvent,
      );

      if (event) {
        return event;
      }
    }

    return undefined;
  }

  if (typeof value !== "object") {
    return undefined;
  }

  const record = value as Record<string, unknown>;
  const eventType = [
    record.type,
    record.event,
    record.eventType,
    record.eventName,
    record.name,
  ].find((currentValue): currentValue is string => typeof currentValue === "string");
  const isPlayerLeftEvent =
    insidePlayerLeftEvent || eventType === "MATCH_CHAMPION_SELECTION_PLAYER_LEFT";
  const directEvent = toChampionSelectionPlayerLeftEvent(record);

  if (directEvent) {
    return directEvent;
  }

  for (const nestedValue of Object.values(record)) {
    const event = findChampionSelectionPlayerLeftEvent(
      nestedValue,
      depth + 1,
      isPlayerLeftEvent,
    );

    if (event) {
      return event;
    }
  }

  return undefined;
}

export function GameModeIcon({ question }: GameModeIconProps) {
  if (question) {
    return <span className="game-mode-question-icon">?</span>;
  }

  return (
    <span className="game-mode-main-icon" aria-hidden="true">
      <svg
        className="game-mode-blossom"
        focusable="false"
        viewBox="0 0 128 128"
      >
        <g className="game-mode-kunai-ring">
          <path d="M64 6 L72 24 L64 34 L56 24 Z" />
          <path d="M122 64 L104 72 L94 64 L104 56 Z" />
          <path d="M64 122 L56 104 L64 94 L72 104 Z" />
          <path d="M6 64 L24 56 L34 64 L24 72 Z" />
          <path d="M23 23 L40 32 L43 45 L30 42 Z" />
          <path d="M105 23 L98 42 L85 45 L88 32 Z" />
          <path d="M23 105 L30 86 L43 83 L40 96 Z" />
          <path d="M105 105 L88 96 L85 83 L98 86 Z" />
        </g>
        <g className="game-mode-open-blossom">
          <path d="M64 58 C51 36 56 18 64 12 C72 18 77 36 64 58 Z" />
          <path d="M70 62 C82 40 101 37 110 42 C108 52 94 64 70 62 Z" />
          <path d="M68 70 C93 72 103 88 102 98 C92 101 75 93 68 70 Z" />
          <path d="M60 70 C53 93 36 101 26 98 C25 88 35 72 60 70 Z" />
          <path d="M58 62 C34 64 20 52 18 42 C27 37 46 40 58 62 Z" />
          <circle cx="64" cy="66" r="11" />
        </g>
      </svg>
    </span>
  );
}
