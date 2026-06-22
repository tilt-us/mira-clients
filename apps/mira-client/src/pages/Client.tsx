import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { ArrowLeft, Check, Copy, Crown, Info, Plus, Search, X } from "lucide-react";
import {
  abortRankedSearch,
  abortSearch,
  accept,
  bootstrap as liveBootstrap,
  cancelChampionPhase,
  clearChampionHover,
  clearChampionHoverDuplicate,
  client,
  createRankedLobby,
  decide,
  decline,
  get as getMatch,
  hoverChampion,
  hoverChampionDuplicate,
  invite,
  invitations as listLobbyInvitations,
  joinLobby,
  kickMember,
  leaveLobby,
  liveSendRequest,
  markChampionsReady,
  markChampionsReadyDuplicate,
  searchRanked,
  search as searchUsers,
  selectChampion,
  selectChampionDuplicate,
  startSearch,
  temporaryMatches,
  transferHost,
  updateMe,
  userStatusMe,
  type _8083ApiMatchResponse,
  type FriendUserResponse,
  type LobbyInvitation,
  type LobbyMember,
  type LobbySnapshot,
  type MatchLobbyResponse,
  type MatchPlayerResponse,
  type MatchResponse,
  type UpdateUserStatusRequest,
  type UserStatusSnapshot,
} from "../api/client";
import {
  LIVE_API_BASE_URL,
  MATCHMAKING_API_BASE_URL,
} from "../api/config";
import { getValidAccessToken } from "../auth/keycloak";
import { readTokens } from "../auth/storage";
import ChampionSelection from "./ChampionSelection";
import CloseDialog from "../components/CloseDialog";
import SettingsModal from "../components/SettingsModal";
import Sidebar from "../components/Sidebar";
import type { AppLocale } from "../i18n";
import { useNotifications } from "../notifications";
import type { AppResolution, ClientAnimation } from "../settings";
import type { FriendProfile, PresenceStatus, Translate } from "../types/ui";
import {
  getAvatarUrl,
  getProfileInitials,
  getPublicDisplayName,
} from "../utils/profile";

type ClientProps = {
  accentColor: string;
  allowFriendRequests: boolean;
  clientAnimation: ClientAnimation;
  closeDialogOpen: boolean;
  locale: AppLocale;
  onAccentColorChange: (accentColor: string) => void;
  onAllowFriendRequestsChange: (allowFriendRequests: boolean) => void;
  onClientAnimationChange: (clientAnimation: ClientAnimation) => void;
  onCloseDialogClose: () => void;
  onLocaleChange: (locale: AppLocale) => void;
  onLogout: () => void;
  onQuit: () => void;
  onResolutionChange: (resolution: AppResolution) => void;
  onSettingsClose: () => void;
  profileAvatarUrl?: string;
  profileName: string;
  profilePublicId?: number;
  resolution: AppResolution;
  settingsOpen: boolean;
  supportsFourKResolution: boolean;
  supportsTwoKResolution: boolean;
  t: Translate;
};

type GameModeIconProps = {
  question?: boolean;
};

type GameMode = "normal" | "ranked";
type ApiPresenceStatus = UpdateUserStatusRequest["status"];
type GameTeam = "dark" | "light";

type LaunchGameRequest = {
  accessToken: string;
  accentColor: string;
  champion: string;
  forceRestart?: boolean;
  matchId: string;
  matchmakingApiBaseUrl: string;
  playerPublicId: number;
  serverHost: string;
  serverControlBaseUrl: string;
  port: number;
  team: GameTeam;
};

type GameLaunchParameters = Omit<LaunchGameRequest, "accessToken" | "accentColor">;

type GameClientStatus = {
  running: boolean;
  pid?: number;
};

type PartyInviteCandidate = {
  avatarUrl?: string;
  email?: string;
  name: string;
  publicId?: number;
  source: "friend" | "user";
};
type LobbyMemberContextMenuState = {
  left: number;
  member: LobbyMember;
  top: number;
};
type MatchDecision = "accept" | "decline";

const afkDelayMs = 5 * 60 * 1000;
const matchAcceptTimeoutMs = 20_000;

function mapUserStatusToPresence(
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

function sendPresenceKeepalive(status: ApiPresenceStatus, mode?: string) {
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

function getMatchChampionForPlayer(match: _8083ApiMatchResponse, playerPublicId: number) {
  return match.championSelections?.find((selection) => {
    return selection.playerPublicId === playerPublicId;
  })?.champion;
}

function getGameClientChampionId(champion: string) {
  return champion.trim().toLowerCase();
}

function getMatchPort(match: _8083ApiMatchResponse) {
  return match.gameServer?.port;
}

function getMatchHost(match: _8083ApiMatchResponse) {
  return match.gameServer?.host;
}

function getMatchControlBaseUrl(match: _8083ApiMatchResponse) {
  const explicitBaseUrl = match.gameServer?.controlBaseUrl;

  if (explicitBaseUrl) {
    return explicitBaseUrl;
  }

  const controlHost = match.gameServer?.controlHost;
  const controlPort = match.gameServer?.controlPort;

  if (!controlHost || typeof controlPort !== "number") {
    return undefined;
  }

  const protocol = match.gameServer?.controlProtocol ?? "http";
  return `${protocol}://${controlHost}:${controlPort}`;
}

function hashString(value: string) {
  let hash = 0;

  for (let index = 0; index < value.length; index += 1) {
    hash = Math.imul(31, hash) + value.charCodeAt(index);
    hash |= 0;
  }

  return Math.abs(hash);
}

function getMatchSeed(match: _8083ApiMatchResponse) {
  return (
    match.matchId ??
    match.lobbies
      ?.map((lobby) => {
        const players = lobby.players
          ?.map((player) => player.publicId ?? player.displayName ?? "")
          .join(",");

        return `${lobby.lobbyId ?? ""}:${players ?? ""}`;
      })
      .sort()
      .join("|") ??
    "match"
  );
}

function getLobbySeed(lobby: MatchLobbyResponse) {
  const players = lobby.players
    ?.map((player) => player.publicId ?? player.displayName ?? "")
    .join(",");

  return `${lobby.lobbyId ?? ""}:${players ?? ""}`;
}

function getMatchTeams(match: _8083ApiMatchResponse): MatchLobbyResponse[] {
  const backendTeams: MatchLobbyResponse[] = [{ players: [] }, { players: [] }];
  let hasBackendTeams = false;

  for (const lobby of match.lobbies ?? []) {
    for (const player of lobby.players ?? []) {
      const team = (player as MatchPlayerResponse & { team?: string }).team?.toLowerCase();
      if (team !== "dark" && team !== "light") {
        continue;
      }

      hasBackendTeams = true;
      const teamIndex = team === "dark" ? 0 : 1;
      backendTeams[teamIndex] = {
        lobbyId: backendTeams[teamIndex].lobbyId ?? lobby.lobbyId,
        players: [...(backendTeams[teamIndex].players ?? []), player],
      };
    }
  }

  if (hasBackendTeams) {
    return backendTeams;
  }

  const matchSeed = getMatchSeed(match);
  const lobbies = [...(match.lobbies ?? [])].sort((left, right) => {
    return (
      hashString(`${matchSeed}:${getLobbySeed(left)}`) -
      hashString(`${matchSeed}:${getLobbySeed(right)}`)
    );
  });
  const teams: MatchLobbyResponse[] = [{ players: [] }, { players: [] }];

  for (const lobby of lobbies) {
    const players = lobby.players ?? [];

    if (players.length === 0) {
      continue;
    }

    const teamIndex =
      [0, 1]
        .sort((left, right) => {
          return (teams[left].players?.length ?? 0) - (teams[right].players?.length ?? 0);
        })
        .find((index) => {
          return (teams[index].players?.length ?? 0) + players.length <= 5;
        }) ?? ((teams[0].players?.length ?? 0) <= (teams[1].players?.length ?? 0) ? 0 : 1);

    teams[teamIndex] = {
      lobbyId: teams[teamIndex].lobbyId ?? lobby.lobbyId,
      players: [...(teams[teamIndex].players ?? []), ...players],
    };
  }

  return hashString(matchSeed) % 2 === 0 ? teams : [teams[1], teams[0]];
}

function getMatchTeamForPlayer(
  match: _8083ApiMatchResponse,
  playerPublicId: number,
): GameTeam | undefined {
  const teams = getMatchTeams(match);

  if (
    teams[0]?.players?.some((player) => {
      return player.publicId === playerPublicId;
    })
  ) {
    return "dark";
  }

  if (
    teams[1]?.players?.some((player) => {
      return player.publicId === playerPublicId;
    })
  ) {
    return "light";
  }

  return undefined;
}

function getInvitationMainInviter(invitation: LobbyInvitation) {
  return (
    invitation.inviters?.[0] ??
    (invitation.lobby ? getLobbyHost(invitation.lobby) : undefined) ??
    invitation.lobby?.members?.[0]
  );
}

function getMemberName(member?: LobbyMember) {
  return getLobbyDisplayName(
    member?.displayName ?? `User ${member?.publicId ?? ""}`.trim(),
  );
}

function getFriendUserName(user: FriendUserResponse) {
  return getPublicDisplayName(
    user.displayName,
    `User ${user.publicId ?? ""}`.trim(),
  );
}

function mapFriendToInviteCandidate(friend: FriendProfile): PartyInviteCandidate {
  return {
    avatarUrl: friend.avatarUrl,
    email: friend.email,
    name: friend.name,
    publicId: friend.publicId,
    source: "friend",
  };
}

function mapUserToInviteCandidate(user: FriendUserResponse): PartyInviteCandidate {
  return {
    avatarUrl: getAvatarUrl(user),
    email: user.email,
    name: getFriendUserName(user),
    publicId: user.publicId,
    source: "user",
  };
}

function mapFriendUserToProfile(user: FriendUserResponse): FriendProfile {
  return {
    avatarUrl: getAvatarUrl(user),
    email: user.email,
    id: String(user.publicId ?? user.email ?? user.displayName ?? "unknown-user"),
    name: getFriendUserName(user),
    publicId: user.publicId,
    status: "offline",
    rank: {
      name: "wood",
      label: "Wood",
      tier: "I",
    },
  };
}

function getInviteCandidateKey(candidate: PartyInviteCandidate) {
  return candidate.publicId ?? candidate.email ?? candidate.name;
}

function getInviteCandidateSubtitle(candidate: PartyInviteCandidate) {
  return typeof candidate.publicId === "number"
    ? `#${candidate.publicId}`
    : candidate.email;
}

function getLobbyDisplayName(name: string) {
  return name.trim().split(/\s+/)[0] || name;
}

function getInvitationModeLabel(invitation: LobbyInvitation) {
  return invitation.mode === "RANKED" || invitation.lobby?.mode === "RANKED"
    ? "Ranked"
    : "Normal";
}

function formatLobbySearchTime(totalSeconds: number) {
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

function isMatchForLobby(match: _8083ApiMatchResponse, lobbyId?: string) {
  return Boolean(
    lobbyId && match.lobbies?.some((lobby) => lobby.lobbyId === lobbyId),
  );
}

function isMatchReady(match: _8083ApiMatchResponse) {
  const acceptances = match.acceptances ?? [];

  return (
    match.status === "CHAMPION_SELECTION" ||
    match.status === "READY" ||
    (acceptances.length > 0 &&
      acceptances.every((acceptance) => acceptance.status === "ACCEPTED"))
  );
}

function normalizeMatchResponse(match: MatchResponse): _8083ApiMatchResponse {
  return match;
}

function getMatchPlayerPublicIds(match: _8083ApiMatchResponse) {
  return (
    match.lobbies
      ?.flatMap((lobby) => lobby.players ?? [])
      .map((player) => player.publicId)
      .filter((publicId): publicId is number => typeof publicId === "number") ?? []
  );
}

function areAllChampionsSelected(match: _8083ApiMatchResponse) {
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

function mergeMatchChampionHovers(
  match: _8083ApiMatchResponse,
  hovers?: _8083ApiMatchResponse["championHovers"],
): _8083ApiMatchResponse {
  return {
    ...match,
    championHovers: hovers ?? [],
  };
}

function mapLobbyToMatchPlayers(lobby: LobbySnapshot) {
  return (
    lobby.members
      ?.filter((member) => typeof member.publicId === "number")
      .map((member) => ({
        publicId: member.publicId as number,
        displayName: member.displayName,
        avatarUrl: member.avatarUrl,
      })) ?? []
  );
}

function toPublicId(value: unknown) {
  if (typeof value === "number") {
    return value;
  }

  if (typeof value === "string") {
    const parsedValue = Number.parseInt(value, 10);

    return Number.isNaN(parsedValue) ? undefined : parsedValue;
  }

  return undefined;
}

function normalizeLobbyMember(member: LobbyMember): LobbyMember {
  return {
    ...member,
    publicId: toPublicId(member.publicId),
  };
}

function normalizeLobbySnapshot(lobby?: LobbySnapshot): LobbySnapshot | undefined {
  if (!lobby) {
    return undefined;
  }

  return {
    ...lobby,
    ownerPublicId: toPublicId(lobby.ownerPublicId),
    members: lobby.members?.map(normalizeLobbyMember),
  };
}

function normalizeLobbyInvitation(invitation: LobbyInvitation): LobbyInvitation {
  const lobby = normalizeLobbySnapshot(invitation.lobby);

  return {
    ...invitation,
    inviteePublicId: toPublicId(invitation.inviteePublicId),
    lobby,
    lobbyId: invitation.lobbyId ?? lobby?.id,
    inviters: invitation.inviters?.map(normalizeLobbyMember),
  };
}

function shouldShowLobbyInvitation(
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

function mergeLobbyInvitations(
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

function getLobbySlotMembers(lobby: LobbySnapshot) {
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

function getLobbyHost(lobby: LobbySnapshot) {
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

function findLobbyInvitation(value: unknown, depth = 0): LobbyInvitation | undefined {
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

  if (typeof record.lobbyId === "string") {
    return normalizeLobbyInvitation(record as LobbyInvitation);
  }

  if (
    typeof lobbyRecord?.id === "string" &&
    ("inviteePublicId" in record ||
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

function findLobbySnapshot(value: unknown, depth = 0): LobbySnapshot | undefined {
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

function findMatchResponse(value: unknown, depth = 0): _8083ApiMatchResponse | undefined {
  if (!value || depth > 5) {
    return undefined;
  }

  if (typeof value === "string") {
    try {
      return findMatchResponse(JSON.parse(value) as unknown, depth + 1);
    } catch {
      return undefined;
    }
  }

  if (Array.isArray(value)) {
    for (const item of value) {
      const match = findMatchResponse(item, depth + 1);

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

  if (
    typeof record.matchId === "string" &&
    typeof record.status === "string" &&
    Array.isArray(record.lobbies)
  ) {
    return normalizeMatchResponse(record as MatchResponse);
  }

  for (const nestedValue of Object.values(record)) {
    const match = findMatchResponse(nestedValue, depth + 1);

    if (match) {
      return match;
    }
  }

  return undefined;
}

function GameModeIcon({ question }: GameModeIconProps) {
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

function Client({
  accentColor,
  allowFriendRequests,
  clientAnimation,
  closeDialogOpen,
  locale,
  onAccentColorChange,
  onAllowFriendRequestsChange,
  onClientAnimationChange,
  onCloseDialogClose,
  onLocaleChange,
  onLogout,
  onQuit,
  onResolutionChange,
  onSettingsClose,
  profileAvatarUrl,
  profileName,
  profilePublicId,
  resolution,
  settingsOpen,
  supportsFourKResolution,
  supportsTwoKResolution,
  t,
}: ClientProps) {
  const [gameSelectorOpen, setGameSelectorOpen] = useState(false);
  const [selectedGameMode, setSelectedGameMode] = useState<GameMode>("ranked");
  const [gameInProgress, setGameInProgress] = useState(false);
  const [activeLobby, setActiveLobby] = useState<LobbySnapshot>();
  const [lobbyInvitations, setLobbyInvitations] = useState<LobbyInvitation[]>([]);
  const [, setLobbyError] = useState<string>();
  const [partyInviteOpen, setPartyInviteOpen] = useState(false);
  const [partyInviteFriends, setPartyInviteFriends] = useState<FriendProfile[]>([]);
  const [partyInviteSearch, setPartyInviteSearch] = useState("");
  const [partyInviteSearchResults, setPartyInviteSearchResults] = useState<
    FriendUserResponse[]
  >([]);
  const [partyInviteSearching, setPartyInviteSearching] = useState(false);
  const [partyInviteBusyId, setPartyInviteBusyId] = useState<number>();
  const [lobbyMemberContextMenu, setLobbyMemberContextMenu] =
    useState<LobbyMemberContextMenuState>();
  const [lobbyMemberActionBusyId, setLobbyMemberActionBusyId] = useState<number>();
  const [lobbyIdContextMenuOpen, setLobbyIdContextMenuOpen] = useState(false);
  const [lobbySearchStartedAt, setLobbySearchStartedAt] = useState<number>();
  const [lobbySearchAbortedLobbyId, setLobbySearchAbortedLobbyId] = useState<string>();
  const [lobbySearchNow, setLobbySearchNow] = useState(Date.now());
  const [pendingMatch, setPendingMatch] = useState<_8083ApiMatchResponse>();
  const [championSelectionMatch, setChampionSelectionMatch] =
    useState<_8083ApiMatchResponse>();
  const [gameLaunchParameters, setGameLaunchParameters] =
    useState<GameLaunchParameters>();
  const [gameClientRunning, setGameClientRunning] = useState(false);
  const [gameReconnectBusy, setGameReconnectBusy] = useState(false);
  const [matchDecisionBusy, setMatchDecisionBusy] = useState<MatchDecision>();
  const [matchFoundStartedAt, setMatchFoundStartedAt] = useState<number>();
  const [matchFoundNow, setMatchFoundNow] = useState(Date.now());
  const [matchAutoDeclinedId, setMatchAutoDeclinedId] = useState<string>();
  const [championsReadyMarkedMatchId, setChampionsReadyMarkedMatchId] = useState<string>();
  const [forceOnlinePublicIds, setForceOnlinePublicIds] = useState<number[]>([]);
  const activeLobbyRef = useRef<LobbySnapshot | undefined>(undefined);
  const championSelectionMatchRef = useRef<_8083ApiMatchResponse | undefined>(undefined);
  const [presenceStatus, setPresenceStatus] = useState<PresenceStatus>("online");
  const lastActivityRef = useRef(Date.now());
  const hiddenSinceRef = useRef<number | undefined>(undefined);
  const remotePresenceRef = useRef<string | undefined>(undefined);
  const requeueingLobbyIdsRef = useRef<Set<string>>(new Set());
  const declinedLobbyInvitationIdsRef = useRef<Set<string>>(new Set());
  const playButtonAnimated =
    clientAnimation === "all" || clientAnimation === "ui-elements";
  const { notify } = useNotifications();
  const playerSlots = Array.from({ length: 5 }, (_, index) => index);
  const lobbySlotMembers = activeLobby ? getLobbySlotMembers(activeLobby) : [];
  const activeLobbyHost = activeLobby ? getLobbyHost(activeLobby) : undefined;
  const isCurrentUserLobbyHost = activeLobbyHost?.publicId === profilePublicId;
  const lobbyIsSearching =
    Boolean(lobbySearchStartedAt) ||
    (activeLobby?.status === "SEARCHING" &&
      activeLobby.id !== lobbySearchAbortedLobbyId);
  const lobbySearchSeconds = lobbySearchStartedAt
    ? Math.max(0, Math.floor((lobbySearchNow - lobbySearchStartedAt) / 1000))
    : 0;
  const lobbySearchTime = formatLobbySearchTime(lobbySearchSeconds);
  const currentPlayerAcceptance = pendingMatch?.acceptances?.find((acceptance) => {
    return acceptance.playerPublicId === profilePublicId;
  });
  const currentPlayerAccepted = currentPlayerAcceptance?.status === "ACCEPTED";
  const matchFoundElapsedMs = matchFoundStartedAt
    ? Math.max(0, matchFoundNow - matchFoundStartedAt)
    : 0;
  const matchFoundProgress = matchFoundStartedAt
    ? Math.max(0, 1 - matchFoundElapsedMs / matchAcceptTimeoutMs)
    : 1;
  const matchFoundRemainingSeconds = matchFoundStartedAt
    ? Math.max(0, Math.ceil((matchAcceptTimeoutMs - matchFoundElapsedMs) / 1_000))
    : 20;
  const matchFoundAcceptedCount =
    pendingMatch?.acceptances?.filter((acceptance) => acceptance.status === "ACCEPTED")
      .length ?? 0;
  const matchFoundMaxAcceptCount =
    pendingMatch?.acceptances?.length ||
    pendingMatch?.lobbies?.reduce((count, lobby) => {
      return count + (lobby.players?.length ?? 0);
    }, 0) ||
    0;

  function notifyLobbyError(message: string) {
    setLobbyError(message);
    notify({
      type: "error",
      message,
    });
  }
  const activeLobbyMemberPublicIds = useMemo(() => {
    return new Set(
      activeLobby?.members
        ?.map((member) => member.publicId)
        .filter((publicId): publicId is number => typeof publicId === "number") ??
        [],
    );
  }, [activeLobby?.members]);
  const friendPublicIds = useMemo(() => {
    return new Set(
      partyInviteFriends
        .map((friend) => friend.publicId)
        .filter((publicId): publicId is number => typeof publicId === "number"),
    );
  }, [partyInviteFriends]);
  const inviteCandidates = useMemo(() => {
    const query = partyInviteSearch.trim().toLowerCase();
    const candidatesById = new Map<number | string, PartyInviteCandidate>();
    const matchesQuery = (candidate: PartyInviteCandidate) => {
      if (!query) {
        return true;
      }

      return (
        candidate.name.toLowerCase().includes(query) ||
        candidate.email?.toLowerCase().includes(query) ||
        String(candidate.publicId ?? "").includes(query)
      );
    };

    for (const friend of partyInviteFriends) {
      const candidate = mapFriendToInviteCandidate(friend);

      if (matchesQuery(candidate)) {
        candidatesById.set(getInviteCandidateKey(candidate), candidate);
      }
    }

    for (const user of partyInviteSearchResults) {
      const candidate = mapUserToInviteCandidate(user);
      const key = getInviteCandidateKey(candidate);

      if (!candidatesById.has(key)) {
        candidatesById.set(key, candidate);
      }
    }

    return [...candidatesById.values()].filter((candidate) => {
      return candidate.publicId !== profilePublicId;
    });
  }, [partyInviteFriends, partyInviteSearch, partyInviteSearchResults, profilePublicId]);
  useEffect(() => {
    activeLobbyRef.current = activeLobby;
  }, [activeLobby]);

  useEffect(() => {
    championSelectionMatchRef.current = championSelectionMatch;
  }, [championSelectionMatch]);

  useEffect(() => {
    if (!gameInProgress || !isTauri()) {
      return;
    }

    let active = true;

    async function refreshGameClientStatus() {
      try {
        const status = await invoke<GameClientStatus>("game_client_status");

        if (active) {
          setGameClientRunning(status.running);
        }
      } catch (caughtError) {
        console.error(caughtError);

        if (active) {
          setGameClientRunning(false);
        }
      }
    }

    void refreshGameClientStatus();

    const intervalId = window.setInterval(() => {
      void refreshGameClientStatus();
    }, 1_500);

    return () => {
      active = false;
      window.clearInterval(intervalId);
    };
  }, [gameInProgress]);

  useEffect(() => {
    const matchId = gameLaunchParameters?.matchId;

    if (!gameInProgress || !matchId) {
      return;
    }

    const activeMatchId = matchId;
    let active = true;

    async function refreshGameMatchStatus() {
      const result = await getMatch({
        baseUrl: MATCHMAKING_API_BASE_URL,
        path: { matchId: activeMatchId },
      });

      if (!active || result.error || !result.data) {
        return;
      }

      if (result.data.status === "ENDED") {
        applyMatch(normalizeMatchResponse(result.data));
      }
    }

    void refreshGameMatchStatus();

    const intervalId = window.setInterval(() => {
      void refreshGameMatchStatus();
    }, 3_000);

    return () => {
      active = false;
      window.clearInterval(intervalId);
    };
  }, [gameInProgress, gameLaunchParameters?.matchId]);

  useEffect(() => {
    if (!activeLobby) {
      setLobbySearchStartedAt(undefined);
      setLobbySearchAbortedLobbyId(undefined);
      setPendingMatch(undefined);
      setMatchFoundStartedAt(undefined);
      setMatchAutoDeclinedId(undefined);
      return;
    }

    if (activeLobby.status !== "SEARCHING") {
      setLobbySearchStartedAt(undefined);
      setPendingMatch(undefined);
      setMatchFoundStartedAt(undefined);
      setMatchAutoDeclinedId(undefined);
      return;
    }

    if (
      activeLobby.status === "SEARCHING" &&
      !lobbySearchStartedAt &&
      activeLobby.id !== lobbySearchAbortedLobbyId
    ) {
      const updatedAt = activeLobby.updatedAt ? Date.parse(activeLobby.updatedAt) : Date.now();

      setLobbySearchStartedAt(Number.isFinite(updatedAt) ? updatedAt : Date.now());
      setLobbySearchNow(Date.now());
    }
  }, [activeLobby, lobbySearchAbortedLobbyId, lobbySearchStartedAt]);

  useEffect(() => {
    if (!lobbySearchStartedAt) {
      return;
    }

    setLobbySearchNow(Date.now());

    const intervalId = window.setInterval(() => {
      setLobbySearchNow(Date.now());
    }, 1_000);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [lobbySearchStartedAt]);

  useEffect(() => {
    if (!pendingMatch || !matchFoundStartedAt) {
      return;
    }

    setMatchFoundNow(Date.now());

    const intervalId = window.setInterval(() => {
      setMatchFoundNow(Date.now());
    }, 50);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [matchFoundStartedAt, pendingMatch]);

  useEffect(() => {
    if (championSelectionMatch || (!lobbyIsSearching && !pendingMatch?.matchId)) {
      return;
    }

    let active = true;

    async function refreshMatch() {
      if (pendingMatch?.matchId) {
        const result = await getMatch({
          baseUrl: MATCHMAKING_API_BASE_URL,
          path: { matchId: pendingMatch.matchId },
        });

        if (active && !result.error) {
          applyMatch(result.data);
        }

        return;
      }

      const result = await temporaryMatches({
        baseUrl: MATCHMAKING_API_BASE_URL,
      });

      if (!active || result.error) {
        return;
      }

      const match = result.data?.find((temporaryMatch) => {
        return (
          temporaryMatch.status !== "CANCELLED" &&
          isMatchForLobby(temporaryMatch, activeLobby?.id)
        );
      });

      applyMatch(match);
    }

    void refreshMatch();

    const intervalId = window.setInterval(refreshMatch, 1_500);

    return () => {
      active = false;
      window.clearInterval(intervalId);
    };
  }, [
    activeLobby?.id,
    championSelectionMatch,
    lobbyIsSearching,
    pendingMatch?.matchId,
  ]);

  useEffect(() => {
    if (!championSelectionMatch?.matchId) {
      return;
    }

    let active = true;

    async function refreshChampionSelectionMatch() {
      if (!championSelectionMatch?.matchId) {
        return;
      }

      const result = await getMatch({
        baseUrl: MATCHMAKING_API_BASE_URL,
        path: { matchId: championSelectionMatch.matchId },
      });

      if (active && !result.error && result.data) {
        applyMatch(result.data);
      }
    }

    const intervalId = window.setInterval(refreshChampionSelectionMatch, 1_000);

    return () => {
      active = false;
      window.clearInterval(intervalId);
    };
  }, [championSelectionMatch?.matchId]);

  useEffect(() => {
    if (
      !championSelectionMatch?.matchId ||
      championsReadyMarkedMatchId === championSelectionMatch.matchId ||
      !areAllChampionsSelected(championSelectionMatch)
    ) {
      return;
    }

    const matchId = championSelectionMatch.matchId;

    setChampionsReadyMarkedMatchId(matchId);

    void markChampionsReady({
      baseUrl: MATCHMAKING_API_BASE_URL,
      path: { matchId },
    }).then(async (result) => {
      if (!result.error && result.data) {
        setChampionSelectionMatch(normalizeMatchResponse(result.data));
        return;
      }

      const fallbackResult = await markChampionsReadyDuplicate({
        baseUrl: MATCHMAKING_API_BASE_URL,
        path: { matchId },
      });

      if (!fallbackResult.error && fallbackResult.data) {
        setChampionSelectionMatch(fallbackResult.data);
      }
    });
  }, [championSelectionMatch, championsReadyMarkedMatchId]);

  function applyLobbyInvitations(nextInvitations: LobbyInvitation[]) {
    const visibleInvitations = nextInvitations.filter((invitation) => {
      const normalizedInvitation = normalizeLobbyInvitation(invitation);

      return (
        !normalizedInvitation.lobbyId ||
        !declinedLobbyInvitationIdsRef.current.has(normalizedInvitation.lobbyId)
      );
    });

    setLobbyInvitations((currentInvitations) =>
      mergeLobbyInvitations(
        currentInvitations,
        visibleInvitations,
        activeLobbyRef.current?.id,
        profilePublicId,
      ),
    );
  }

  function replaceLobbyInvitations(nextInvitations: LobbyInvitation[]) {
    const visibleInvitations = nextInvitations.filter((invitation) => {
      const normalizedInvitation = normalizeLobbyInvitation(invitation);

      return (
        !normalizedInvitation.lobbyId ||
        !declinedLobbyInvitationIdsRef.current.has(normalizedInvitation.lobbyId)
      );
    });

    setLobbyInvitations(
      mergeLobbyInvitations(
        [],
        visibleInvitations,
        activeLobbyRef.current?.id,
        profilePublicId,
      ),
    );
  }

  async function restartMatchSearchForLobby(lobby: LobbySnapshot) {
    if (!lobby.id || requeueingLobbyIdsRef.current.has(lobby.id)) {
      return;
    }

    requeueingLobbyIdsRef.current.add(lobby.id);

    try {
      await searchRanked({
        baseUrl: LIVE_API_BASE_URL,
        body: { lobbyId: lobby.id },
      });

      const result = await startSearch({
        baseUrl: MATCHMAKING_API_BASE_URL,
        body: {
          lobbyId: lobby.id,
          mode: "RANKED",
          players: mapLobbyToMatchPlayers(lobby),
        },
      });

      if (!result.error) {
        applyMatch(result.data?.match, { keepSearchingOnCancel: true });
      }
    } finally {
      requeueingLobbyIdsRef.current.delete(lobby.id);
    }
  }

  function applyMatch(
    match: _8083ApiMatchResponse | undefined,
    options: { keepSearchingOnCancel?: boolean } = {},
  ) {
    if (!match) {
      return;
    }

    if (gameInProgress) {
      if (
        match.status === "ENDED" &&
        (!gameLaunchParameters?.matchId || match.matchId === gameLaunchParameters.matchId)
      ) {
        finishGameSession(match);
      }

      return;
    }

    if (
      championSelectionMatch?.matchId &&
      match.matchId !== championSelectionMatch.matchId
    ) {
      return;
    }

    if (match.status === "CANCELLED") {
      const lobby = activeLobbyRef.current;
      const keepSearching =
        options.keepSearchingOnCancel ?? lobby?.status === "SEARCHING";

      setPendingMatch(undefined);
      setMatchFoundStartedAt(undefined);
      setMatchAutoDeclinedId(undefined);
      setChampionSelectionMatch(undefined);
      setChampionsReadyMarkedMatchId(undefined);

      if (keepSearching && lobby?.id) {
        const startedAt = lobby.updatedAt ? Date.parse(lobby.updatedAt) : Date.now();

        setLobbySearchStartedAt((currentStartedAt) =>
          currentStartedAt ??
          (Number.isFinite(startedAt) ? startedAt : Date.now()),
        );
        setLobbySearchNow(Date.now());
        setLobbySearchAbortedLobbyId(undefined);
        setActiveLobby((currentLobby) =>
          currentLobby
            ? {
                ...currentLobby,
                status: "SEARCHING",
              }
            : currentLobby,
        );

        if (getLobbyHost(lobby)?.publicId === profilePublicId) {
          void restartMatchSearchForLobby(lobby);
        }
      } else {
        setLobbySearchStartedAt(undefined);
      }

      return;
    }

    if (isMatchReady(match)) {
      setPendingMatch(undefined);
      setMatchFoundStartedAt(undefined);
      setMatchAutoDeclinedId(undefined);
      setChampionSelectionMatch(match);
      return;
    }

    if (match.status === "PENDING_ACCEPTANCE") {
      setMatchFoundStartedAt((currentStartedAt) => {
        if (currentStartedAt && pendingMatch?.matchId === match.matchId) {
          return currentStartedAt;
        }

        return Date.now();
      });
      setMatchFoundNow(Date.now());
      if (pendingMatch?.matchId !== match.matchId) {
        setMatchAutoDeclinedId(undefined);
      }
      setPendingMatch(match);
    }
  }

  function markPublicIdOnlineTemporarily(publicId: number) {
    setForceOnlinePublicIds((currentPublicIds) => {
      if (currentPublicIds.includes(publicId)) {
        return currentPublicIds;
      }

      return [...currentPublicIds, publicId];
    });

    window.setTimeout(() => {
      setForceOnlinePublicIds((currentPublicIds) =>
        currentPublicIds.filter((currentPublicId) => currentPublicId !== publicId),
      );
    }, 45_000);
  }

  async function refreshLobbyFriendProfiles() {
    const result = await liveBootstrap({
      baseUrl: LIVE_API_BASE_URL,
    });

    if (result.error) {
      notifyLobbyError(t("friend-api-error"));
      return;
    }

    setPartyInviteFriends(
      (result.data?.friends?.friends ?? []).map(mapFriendUserToProfile),
    );

    replaceLobbyInvitations(result.data?.lobbyInvitations ?? []);
    setLobbyError(undefined);
  }

  useEffect(() => {
    let active = true;

    async function refreshInvitations() {
      const result = await listLobbyInvitations({
        baseUrl: LIVE_API_BASE_URL,
      });

      if (!active || result.error) {
        return;
      }

      replaceLobbyInvitations(result.data ?? []);
    }

    void refreshInvitations();

    const intervalId = window.setInterval(refreshInvitations, 3_000);

    return () => {
      active = false;
      window.clearInterval(intervalId);
    };
  }, [activeLobby?.id, profilePublicId]);

  useEffect(() => {
    if (!activeLobby) {
      return;
    }

    void refreshLobbyFriendProfiles();
  }, [activeLobby?.id]);

  useEffect(() => {
    if (!partyInviteOpen) {
      return;
    }

    let active = true;

    void refreshLobbyFriendProfiles().finally(() => {
      if (!active) {
        return;
      }
    });

    return () => {
      active = false;
    };
  }, [partyInviteOpen, t]);

  useEffect(() => {
    if (!partyInviteOpen) {
      return;
    }

    const query = partyInviteSearch.trim();

    if (query.length < 2) {
      setPartyInviteSearchResults([]);
      setPartyInviteSearching(false);
      return;
    }

    let active = true;
    setPartyInviteSearching(true);

    const timeoutId = window.setTimeout(async () => {
      const result = await searchUsers({
        query: { q: query },
      });

      if (!active) {
        return;
      }

      if (result.error) {
        notifyLobbyError(t("friend-api-error"));
        setPartyInviteSearchResults([]);
      } else {
        setPartyInviteSearchResults(result.data?.users ?? []);
      }

      setPartyInviteSearching(false);
    }, 240);

    return () => {
      active = false;
      window.clearTimeout(timeoutId);
    };
  }, [partyInviteOpen, partyInviteSearch, t]);

  useEffect(() => {
    if (!lobbyMemberContextMenu) {
      return;
    }

    function closeLobbyMemberContextMenu() {
      setLobbyMemberContextMenu(undefined);
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        closeLobbyMemberContextMenu();
      }
    }

    window.addEventListener("click", closeLobbyMemberContextMenu);
    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("click", closeLobbyMemberContextMenu);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [lobbyMemberContextMenu]);

  useEffect(() => {
    if (!lobbyIdContextMenuOpen) {
      return;
    }

    function closeLobbyIdContextMenu() {
      setLobbyIdContextMenuOpen(false);
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        closeLobbyIdContextMenu();
      }
    }

    window.addEventListener("click", closeLobbyIdContextMenu);
    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("click", closeLobbyIdContextMenu);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [lobbyIdContextMenuOpen]);

  async function publishPresence(status: ApiPresenceStatus, mode?: string) {
    const presenceKey = `${status}:${mode ?? ""}`;

    if (remotePresenceRef.current === presenceKey) {
      return;
    }

    remotePresenceRef.current = presenceKey;

    const result = await updateMe({
      baseUrl: LIVE_API_BASE_URL,
      body: { status, mode },
    });

    if (result.error) {
      remotePresenceRef.current = undefined;
    }
  }

  function getIdlePresenceStatus(): ApiPresenceStatus {
    const now = Date.now();
    const hiddenForMs = hiddenSinceRef.current ? now - hiddenSinceRef.current : 0;
    const inactiveForMs = now - lastActivityRef.current;

    return inactiveForMs >= afkDelayMs || hiddenForMs >= afkDelayMs
      ? "AFK"
      : "ONLINE";
  }

  function syncPresenceWithActivity() {
    if (championSelectionMatchRef.current) {
      setPresenceStatus("championselection");
      void publishPresence("CHAMPION_SELECTION");
      return;
    }

    if (activeLobbyRef.current?.status === "SEARCHING") {
      setPresenceStatus("inqueue");
      void publishPresence("IN_QUEUE");
      return;
    }

    if (activeLobbyRef.current) {
      setPresenceStatus("inlobby");
      void publishPresence("IN_LOBBY", selectedGameMode === "ranked" ? "Ranked" : "Normal");
      return;
    }

    const nextStatus = getIdlePresenceStatus();
    setPresenceStatus(mapUserStatusToPresence(nextStatus));
    void publishPresence(nextStatus);
  }

  function suppressMatchLobbyInvitations(match?: _8083ApiMatchResponse) {
    for (const lobby of match?.lobbies ?? []) {
      if (lobby.lobbyId) {
        declinedLobbyInvitationIdsRef.current.add(lobby.lobbyId);
      }
    }
  }

  function finishGameSession(match?: _8083ApiMatchResponse) {
    suppressMatchLobbyInvitations(match);
    activeLobbyRef.current = undefined;
    championSelectionMatchRef.current = undefined;
    setLobbyInvitations([]);
    setPendingMatch(undefined);
    setMatchFoundStartedAt(undefined);
    setMatchAutoDeclinedId(undefined);
    setChampionSelectionMatch(undefined);
    setChampionsReadyMarkedMatchId(undefined);
    setGameInProgress(false);
    setGameClientRunning(false);
    setGameLaunchParameters(undefined);
    setGameReconnectBusy(false);
    setLobbySearchStartedAt(undefined);
    setLobbySearchAbortedLobbyId(undefined);
    setActiveLobby(undefined);
    setGameSelectorOpen(false);
    setPresenceStatus("online");
    void publishPresence("ONLINE");
  }

  function handleRemovedFromActiveLobby() {
    activeLobbyRef.current = undefined;
    setActiveLobby(undefined);
    setPresenceStatus("online");
    void publishPresence("ONLINE");
  }

  useEffect(() => {
    let active = true;

    async function initializePresence() {
      const currentStatus = await userStatusMe({
        baseUrl: LIVE_API_BASE_URL,
      });

      if (!active) {
        return;
      }

      if (!currentStatus.error && currentStatus.data?.status) {
        setPresenceStatus(
          mapUserStatusToPresence(currentStatus.data.status, currentStatus.data.mode),
        );
      }

      lastActivityRef.current = Date.now();
      setPresenceStatus("online");
      void publishPresence("ONLINE");
    }

    void initializePresence();

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    syncPresenceWithActivity();
  }, [activeLobby?.id, activeLobby?.status, championSelectionMatch?.matchId, selectedGameMode]);

  useEffect(() => {
    function markActivity() {
      lastActivityRef.current = Date.now();

      if (!document.hidden) {
        hiddenSinceRef.current = undefined;
      }

      if (!activeLobbyRef.current && !championSelectionMatchRef.current) {
        setPresenceStatus("online");
        void publishPresence("ONLINE");
      }
    }

    function handleVisibilityChange() {
      if (document.hidden) {
        hiddenSinceRef.current = Date.now();
        return;
      }

      markActivity();
    }

    const activityEvents = [
      "keydown",
      "mousedown",
      "mousemove",
      "pointerdown",
      "pointermove",
      "touchstart",
      "wheel",
    ] as const;

    for (const eventName of activityEvents) {
      window.addEventListener(eventName, markActivity, { passive: true });
    }

    document.addEventListener("visibilitychange", handleVisibilityChange);

    const intervalId = window.setInterval(syncPresenceWithActivity, 15_000);

    return () => {
      for (const eventName of activityEvents) {
        window.removeEventListener(eventName, markActivity);
      }

      document.removeEventListener("visibilitychange", handleVisibilityChange);
      window.clearInterval(intervalId);
    };
  }, []);

  useEffect(() => {
    function leaveActiveLobby() {
      const lobby = activeLobbyRef.current;

      if (!lobby?.id) {
        return;
      }

      void leaveLobby({
        baseUrl: LIVE_API_BASE_URL,
        path: { lobbyId: lobby.id },
      });
    }

    function publishOffline() {
      sendPresenceKeepalive("OFFLINE");
      void publishPresence("OFFLINE");
    }

    window.addEventListener("pagehide", leaveActiveLobby);
    window.addEventListener("pagehide", publishOffline);
    window.addEventListener("beforeunload", leaveActiveLobby);
    window.addEventListener("beforeunload", publishOffline);

    return () => {
      window.removeEventListener("pagehide", leaveActiveLobby);
      window.removeEventListener("pagehide", publishOffline);
      window.removeEventListener("beforeunload", leaveActiveLobby);
      window.removeEventListener("beforeunload", publishOffline);
    };
  }, []);

  async function leaveCurrentLobby() {
    const lobby = activeLobbyRef.current;

    if (!lobby?.id) {
      return;
    }

    await leaveLobby({
      baseUrl: LIVE_API_BASE_URL,
      path: { lobbyId: lobby.id },
    });
    activeLobbyRef.current = undefined;
    setActiveLobby(undefined);
    syncPresenceWithActivity();
  }

  useEffect(() => {
    let active = true;
    const abortController = new AbortController();

    function addInvitation(invitation: LobbyInvitation) {
      applyLobbyInvitations([invitation]);
    }

    async function refreshActiveLobby() {
      const activeLobbyId = activeLobbyRef.current?.id;

      if (!activeLobbyId) {
        return;
      }

      const result = await liveBootstrap({
        baseUrl: LIVE_API_BASE_URL,
      });

      if (!active || result.error) {
        return;
      }

      const lobby = result.data?.openFriendLobbies?.find((openLobby) => {
        return openLobby.id === activeLobbyId;
      });

      if (!lobby) {
        return;
      }

      const stillInLobby = lobby.members?.some((member) => {
        return member.publicId === profilePublicId;
      });

      if (stillInLobby) {
        setActiveLobby(lobby);
      } else {
        handleRemovedFromActiveLobby();
      }
    }

    async function listenForLobbyEvents() {
      try {
        const result = await client.sse.get<unknown>({
          baseUrl: LIVE_API_BASE_URL,
          signal: abortController.signal,
          url: "/api/live/events",
        });

        for await (const _event of result.stream) {
          if (!active) {
            break;
          }

          const invitation = findLobbyInvitation(_event);
          const lobbySnapshot = findLobbySnapshot(_event);
          const match = findMatchResponse(_event);

          if (match) {
            applyMatch(match, { keepSearchingOnCancel: false });
          }

          if (invitation) {
            addInvitation(invitation);

            if (invitation.lobby?.id === activeLobbyRef.current?.id) {
              setActiveLobby(invitation.lobby);
            }
          }

          if (lobbySnapshot && lobbySnapshot.id === activeLobbyRef.current?.id) {
            const stillInLobby = lobbySnapshot.members?.some((member) => {
              return member.publicId === profilePublicId;
            });

            if (stillInLobby) {
              setActiveLobby(lobbySnapshot);
            } else {
              handleRemovedFromActiveLobby();
            }
          } else {
            await refreshActiveLobby();
          }
        }
      } catch {
        // The Sidebar owns the visible live error state; invite cards can retry on
        // the next successful bootstrap/event without interrupting the client view.
      }
    }

    void listenForLobbyEvents();

    return () => {
      active = false;
      abortController.abort();
    };
  }, [activeLobby?.id, profilePublicId]);

  async function handleTopButtonClick() {
    if (activeLobby) {
      await leaveCurrentLobby();
      setGameSelectorOpen(true);
      return;
    }

    setGameSelectorOpen((open) => !open);
  }

  async function handleClientLogout() {
    await leaveCurrentLobby();
    await publishPresence("OFFLINE");
    onLogout();
  }

  async function handleClientQuit() {
    await leaveCurrentLobby();
    await publishPresence("OFFLINE");
    onQuit();
  }

  async function handleCreateLobby() {
    if (selectedGameMode !== "ranked") {
      return;
    }

    setLobbyError(undefined);
    setGameInProgress(false);
    setGameLaunchParameters(undefined);

    const result = await createRankedLobby({
      baseUrl: LIVE_API_BASE_URL,
    });

    if (result.error || !result.data?.id) {
      notifyLobbyError(t("lobby-create-error"));
      return;
    }

    setLobbySearchAbortedLobbyId(undefined);
    setLobbySearchStartedAt(undefined);
    setActiveLobby(result.data);
    setGameSelectorOpen(false);
  }

  async function handleLobbySearch() {
    if (!activeLobby?.id) {
      return;
    }

    if (!isCurrentUserLobbyHost) {
      return;
    }

    if (lobbyIsSearching) {
      const [rankedResult, matchResult] = await Promise.all([
        abortRankedSearch({
          baseUrl: LIVE_API_BASE_URL,
          body: { lobbyId: activeLobby.id },
        }),
        abortSearch({
          baseUrl: MATCHMAKING_API_BASE_URL,
          path: { lobbyId: activeLobby.id },
        }),
      ]);

      applyMatch(matchResult.data?.cancelledMatch, { keepSearchingOnCancel: false });
      setPendingMatch(undefined);
      setMatchAutoDeclinedId(undefined);
      setMatchFoundStartedAt(undefined);
      setLobbySearchStartedAt(undefined);
      setLobbySearchAbortedLobbyId(activeLobby.id);
      setActiveLobby(rankedResult.error || !rankedResult.data
        ? {
            ...activeLobby,
            status: "OPEN",
          }
        : rankedResult.data,
      );
      return;
    }

    setLobbyError(undefined);
    const wasLocallyAborted = activeLobby.id === lobbySearchAbortedLobbyId;
    setLobbySearchAbortedLobbyId(undefined);

    if (!wasLocallyAborted && activeLobby.status === "SEARCHING") {
      const startedAt = activeLobby.updatedAt ? Date.parse(activeLobby.updatedAt) : Date.now();

      setLobbySearchStartedAt(Number.isFinite(startedAt) ? startedAt : Date.now());
      setLobbySearchNow(Date.now());

      const result = await startSearch({
        baseUrl: MATCHMAKING_API_BASE_URL,
        body: {
          lobbyId: activeLobby.id,
          mode: "RANKED",
          players: mapLobbyToMatchPlayers(activeLobby),
        },
      });

      if (!result.error) {
        applyMatch(result.data?.match);
      }

      return;
    }

    const [result, matchSearchResult] = await Promise.all([
      searchRanked({
        baseUrl: LIVE_API_BASE_URL,
        body: { lobbyId: activeLobby.id },
      }),
      startSearch({
        baseUrl: MATCHMAKING_API_BASE_URL,
        body: {
          lobbyId: activeLobby.id,
          mode: "RANKED",
          players: mapLobbyToMatchPlayers(activeLobby),
        },
      }),
    ]);

    if (result.error && matchSearchResult.error) {
      notifyLobbyError(t("lobby-search-error"));
      return;
    }

    const startedAt = result.data?.startedAt
      ? Date.parse(result.data.startedAt)
      : Date.now();

    setLobbySearchStartedAt(Number.isFinite(startedAt) ? startedAt : Date.now());
    setLobbySearchNow(Date.now());
    setActiveLobby({
      ...activeLobby,
      status: "SEARCHING",
      updatedAt: result.data?.startedAt ?? activeLobby.updatedAt,
    });

    if (!matchSearchResult.error) {
      applyMatch(matchSearchResult.data?.match);
    }
  }

  async function handleCopyLobbyId() {
    if (!activeLobby?.id) {
      return;
    }

    await navigator.clipboard.writeText(activeLobby.id);
    setLobbyIdContextMenuOpen(false);
  }

  async function handleMatchDecision(decision: MatchDecision) {
    if (!pendingMatch?.matchId) {
      return;
    }

    const matchId = pendingMatch.matchId;
    setMatchDecisionBusy(decision);

    const result = await (decision === "accept" ? accept : decline)({
      baseUrl: MATCHMAKING_API_BASE_URL,
      path: { matchId },
    });

    let nextMatch = result.error || !result.data
      ? undefined
      : normalizeMatchResponse(result.data);

    if (!nextMatch && typeof profilePublicId === "number") {
      const fallbackResult = await decide({
        baseUrl: MATCHMAKING_API_BASE_URL,
        body: {
          playerPublicId: profilePublicId,
          decision: decision === "accept" ? "ACCEPTED" : "DECLINED",
        },
        path: { matchId },
      });

      if (!fallbackResult.error && fallbackResult.data) {
        nextMatch = fallbackResult.data;
      }
    }

    setMatchDecisionBusy(undefined);

    if (!nextMatch && decision === "accept") {
      notifyLobbyError(t("match-decision-error"));
      return;
    }

    if (nextMatch) {
      applyMatch(nextMatch, { keepSearchingOnCancel: decision !== "decline" });
    }

    if (decision === "decline") {
      if (!nextMatch) {
        notifyLobbyError(t("match-decision-error"));
      }

      setPendingMatch(undefined);
      setMatchFoundStartedAt(undefined);
      setMatchAutoDeclinedId(undefined);
      setLobbySearchStartedAt(undefined);
      setActiveLobby((currentLobby) =>
        currentLobby
          ? {
              ...currentLobby,
              status: "OPEN",
            }
          : currentLobby,
      );
    }
  }

  useEffect(() => {
    if (
      !pendingMatch ||
      !matchFoundStartedAt ||
      currentPlayerAccepted ||
      matchDecisionBusy ||
      matchAutoDeclinedId === pendingMatch.matchId ||
      matchFoundElapsedMs < matchAcceptTimeoutMs
    ) {
      return;
    }

    setMatchAutoDeclinedId(pendingMatch.matchId);
    void handleMatchDecision("decline");
  }, [
    currentPlayerAccepted,
    matchAutoDeclinedId,
    matchDecisionBusy,
    matchFoundElapsedMs,
    matchFoundStartedAt,
    pendingMatch,
  ]);

  async function handleLobbyFriendDrop(friend: FriendProfile) {
    if (!activeLobby?.id || typeof friend.publicId !== "number") {
      return;
    }

    const result = await invite({
      baseUrl: LIVE_API_BASE_URL,
      body: { targetPublicId: friend.publicId },
      path: { lobbyId: activeLobby.id },
    });

    if (result.error) {
      notifyLobbyError(t("lobby-invite-error"));
      return;
    }

    setLobbyError(undefined);
  }

  function openPartyInviteDialog() {
    if (!activeLobby?.id) {
      return;
    }

    setPartyInviteOpen(true);
    setPartyInviteSearch("");
    setPartyInviteSearchResults([]);
    setLobbyError(undefined);
  }

  async function handleInviteCandidate(candidate: PartyInviteCandidate) {
    if (!activeLobby?.id || typeof candidate.publicId !== "number") {
      return;
    }

    setPartyInviteBusyId(candidate.publicId);

    const result = await invite({
      baseUrl: LIVE_API_BASE_URL,
      body: { targetPublicId: candidate.publicId },
      path: { lobbyId: activeLobby.id },
    });

    setPartyInviteBusyId(undefined);

    if (result.error) {
      notifyLobbyError(t("lobby-invite-error"));
      return;
    }

    setLobbyError(undefined);
  }

  function openLobbyMemberContextMenu(
    member: LobbyMember,
    element: HTMLElement,
  ) {
    const rect = element.getBoundingClientRect();

    setLobbyMemberContextMenu({
      left: rect.left + rect.width / 2,
      member,
      top: rect.bottom + 8,
    });
  }

  function handleViewLobbyMemberProfile() {
    setLobbyMemberContextMenu(undefined);
  }

  async function handleAddLobbyMemberFriend(member: LobbyMember) {
    if (typeof member.publicId !== "number") {
      return;
    }

    setLobbyMemberActionBusyId(member.publicId);

    const result = await liveSendRequest({
      baseUrl: LIVE_API_BASE_URL,
      body: { targetPublicId: member.publicId },
    });

    setLobbyMemberActionBusyId(undefined);

    if (result.error) {
      notifyLobbyError(t("friend-api-error"));
      return;
    }

    setLobbyMemberContextMenu(undefined);
    await refreshLobbyFriendProfiles();
  }

  async function handleMakeLobbyHost(member: LobbyMember) {
    if (!activeLobby?.id || typeof member.publicId !== "number") {
      return;
    }

    setLobbyMemberActionBusyId(member.publicId);

    const result = await transferHost({
      baseUrl: LIVE_API_BASE_URL,
      body: { targetPublicId: member.publicId },
      path: { lobbyId: activeLobby.id },
    });

    setLobbyMemberActionBusyId(undefined);

    if (result.error || !result.data) {
      notifyLobbyError(t("lobby-host-transfer-error"));
      return;
    }

    setActiveLobby(result.data);
    setLobbyError(undefined);
    setLobbyMemberContextMenu(undefined);
  }

  async function handleKickMember(member: LobbyMember) {
    const lobbyHost = activeLobby ? getLobbyHost(activeLobby) : undefined;

    if (
      !activeLobby?.id ||
      lobbyHost?.publicId !== profilePublicId ||
      typeof member.publicId !== "number" ||
      member.publicId === profilePublicId
    ) {
      return;
    }

    setLobbyMemberActionBusyId(member.publicId);

    const result = await kickMember({
      baseUrl: LIVE_API_BASE_URL,
      path: {
        lobbyId: activeLobby.id,
        memberPublicId: member.publicId,
      },
    });

    setLobbyMemberActionBusyId(undefined);

    if (result.error || !result.data) {
      notifyLobbyError(t("lobby-kick-error"));
      return;
    }

    setLobbyError(undefined);
    markPublicIdOnlineTemporarily(member.publicId);
    setActiveLobby(result.data);
    setLobbyMemberContextMenu(undefined);
  }

  async function handleJoinFriendParty(lobby: LobbySnapshot) {
    if (!lobby.id) {
      return;
    }

    const result = await joinLobby({
      baseUrl: LIVE_API_BASE_URL,
      path: { lobbyId: lobby.id },
    });

    if (result.error || !result.data) {
      notifyLobbyError(t("lobby-join-error"));
      return;
    }

    setActiveLobby(result.data);
    setGameSelectorOpen(false);
  }

  async function handleAcceptInvite(invitation: LobbyInvitation) {
    if (!invitation.lobbyId) {
      return;
    }

    declinedLobbyInvitationIdsRef.current.delete(invitation.lobbyId);

    const result = await joinLobby({
      baseUrl: LIVE_API_BASE_URL,
      path: { lobbyId: invitation.lobbyId },
    });

    if (result.error || !result.data) {
      notifyLobbyError(t("lobby-join-error"));
      return;
    }

    setActiveLobby(result.data);
    setLobbyInvitations((currentInvitations) =>
      currentInvitations.filter(
        (currentInvitation) => currentInvitation.lobbyId !== invitation.lobbyId,
      ),
    );
    setGameSelectorOpen(false);
  }

  function handleDeclineInvite(invitation: LobbyInvitation) {
    if (invitation.lobbyId) {
      declinedLobbyInvitationIdsRef.current.add(invitation.lobbyId);
    }

    setLobbyInvitations((currentInvitations) =>
      currentInvitations.filter(
        (currentInvitation) => currentInvitation.lobbyId !== invitation.lobbyId,
      ),
    );
  }

  async function handleChampionSelect(champion: string) {
    if (!championSelectionMatch?.matchId) {
      return false;
    }

    const result = await selectChampion({
      baseUrl: MATCHMAKING_API_BASE_URL,
      body: { champion },
      path: { matchId: championSelectionMatch.matchId },
    });

    let nextMatch = result.error || !result.data
      ? undefined
      : normalizeMatchResponse(result.data);

    if (!nextMatch && typeof profilePublicId === "number") {
      const fallbackResult = await selectChampionDuplicate({
        baseUrl: MATCHMAKING_API_BASE_URL,
        body: {
          champion,
          playerPublicId: profilePublicId,
        },
        path: { matchId: championSelectionMatch.matchId },
      });

      if (!fallbackResult.error && fallbackResult.data) {
        nextMatch = fallbackResult.data;
      }
    }

    if (!nextMatch) {
      notifyLobbyError(t("match-decision-error"));
      return false;
    }

    setChampionSelectionMatch(nextMatch);
    return true;
  }

  async function handleChampionHover(champion?: string, publish = true) {
    if (!championSelectionMatch?.matchId || !publish) {
      return;
    }

    const matchId = championSelectionMatch.matchId;

    if (!champion) {
      const result = await clearChampionHover({
        baseUrl: MATCHMAKING_API_BASE_URL,
        path: { matchId },
      });

      if (!result.error && result.data) {
        setChampionSelectionMatch((currentMatch) =>
          currentMatch ? mergeMatchChampionHovers(currentMatch, result.data.hovers) : currentMatch,
        );
        return;
      }

      if (typeof profilePublicId !== "number") {
        return;
      }

      const fallbackResult = await clearChampionHoverDuplicate({
        baseUrl: MATCHMAKING_API_BASE_URL,
        path: { matchId, playerPublicId: profilePublicId },
      });

      if (!fallbackResult.error && fallbackResult.data) {
        setChampionSelectionMatch((currentMatch) =>
          currentMatch
            ? mergeMatchChampionHovers(currentMatch, fallbackResult.data.hovers)
            : currentMatch,
        );
      }

      return;
    }

    const result = await hoverChampion({
      baseUrl: MATCHMAKING_API_BASE_URL,
      body: { champion },
      path: { matchId },
    });

    if (!result.error && result.data) {
      setChampionSelectionMatch((currentMatch) =>
        currentMatch ? mergeMatchChampionHovers(currentMatch, result.data.hovers) : currentMatch,
      );
      return;
    }

    if (typeof profilePublicId !== "number") {
      return;
    }

    const fallbackResult = await hoverChampionDuplicate({
      baseUrl: MATCHMAKING_API_BASE_URL,
      body: { champion, playerPublicId: profilePublicId },
      path: { matchId },
    });

    if (!fallbackResult.error && fallbackResult.data) {
      setChampionSelectionMatch((currentMatch) =>
        currentMatch
          ? mergeMatchChampionHovers(currentMatch, fallbackResult.data.hovers)
          : currentMatch,
      );
    }
  }

  async function handleChampionSelectionTimeout() {
    let abortedLobby: LobbySnapshot | undefined;

    if (championSelectionMatch?.matchId) {
      await cancelChampionPhase({
        baseUrl: MATCHMAKING_API_BASE_URL,
        path: { matchId: championSelectionMatch.matchId },
      });
    }

    if (activeLobby?.id) {
      const [rankedResult, matchResult] = await Promise.all([
        abortRankedSearch({
          baseUrl: LIVE_API_BASE_URL,
          body: { lobbyId: activeLobby.id },
        }),
        abortSearch({
          baseUrl: MATCHMAKING_API_BASE_URL,
          path: { lobbyId: activeLobby.id },
        }),
      ]);

      if (!rankedResult.error && rankedResult.data) {
        abortedLobby = rankedResult.data;
      }

      applyMatch(matchResult.data?.cancelledMatch, { keepSearchingOnCancel: false });
    }

    setChampionSelectionMatch(undefined);
    setPendingMatch(undefined);
    setMatchFoundStartedAt(undefined);
    setMatchAutoDeclinedId(undefined);
    setChampionsReadyMarkedMatchId(undefined);
    setLobbySearchStartedAt(undefined);
    setLobbySearchAbortedLobbyId(undefined);
    setActiveLobby((currentLobby) =>
      abortedLobby ??
      (currentLobby
        ? {
            ...currentLobby,
            status: "OPEN",
          }
        : currentLobby),
    );
  }

  function createGameLaunchParameters(match: _8083ApiMatchResponse): GameLaunchParameters {
    if (!match.matchId) {
      throw new Error("Match-ID fehlt.");
    }

    if (typeof profilePublicId !== "number") {
      throw new Error("Spieler-ID fehlt.");
    }

    const selectedChampion = getMatchChampionForPlayer(match, profilePublicId);

    if (!selectedChampion) {
      throw new Error("Champion fehlt.");
    }

    const champion = getGameClientChampionId(selectedChampion);

    const port = getMatchPort(match);

    if (typeof port !== "number") {
      throw new Error("Game-Server-Port fehlt.");
    }

    const serverHost = getMatchHost(match);

    if (!serverHost) {
      throw new Error("Game-Server-Adresse fehlt.");
    }

    const serverControlBaseUrl = getMatchControlBaseUrl(match);

    if (!serverControlBaseUrl) {
      throw new Error("Game-Server-Control-Adresse fehlt.");
    }

    const team = getMatchTeamForPlayer(match, profilePublicId);

    if (!team) {
      throw new Error("Team fehlt.");
    }

    return {
      champion,
      matchId: match.matchId,
      matchmakingApiBaseUrl: MATCHMAKING_API_BASE_URL,
      playerPublicId: profilePublicId,
      serverHost,
      serverControlBaseUrl,
      port,
      team,
    };
  }

  async function launchGameClient(parameters: GameLaunchParameters, forceRestart = false) {
    if (!isTauri()) {
      throw new Error("Game Client kann nur in der Desktop-App gestartet werden.");
    }

    const accessToken = await getValidAccessToken();

    if (!accessToken) {
      throw new Error("Access Token fehlt.");
    }

    const request: LaunchGameRequest = {
      accessToken,
      accentColor,
      ...parameters,
      forceRestart,
    };

    await invoke("launch_game", { request });
    setGameLaunchParameters(parameters);
    setGameClientRunning(true);
  }

  function finishGameStart() {
    setChampionSelectionMatch(undefined);
    setPendingMatch(undefined);
    setMatchFoundStartedAt(undefined);
    setMatchAutoDeclinedId(undefined);
    setChampionsReadyMarkedMatchId(undefined);
    setLobbySearchStartedAt(undefined);
    setLobbySearchAbortedLobbyId(undefined);
    setActiveLobby(undefined);
    setGameSelectorOpen(false);
    setGameInProgress(true);
  }

  async function handleReadyPhaseComplete() {
    const match = championSelectionMatch;

    if (!match) {
      return;
    }

    try {
      const launchParameters = createGameLaunchParameters(match);

      await launchGameClient(launchParameters, true);
      finishGameStart();
    } catch (caughtError) {
      console.error(caughtError);
      notifyLobbyError(t("client-game-start-error"));
    }
  }

  async function handleReconnectGameClient() {
    if (!gameLaunchParameters || gameReconnectBusy) {
      return;
    }

    setGameReconnectBusy(true);
    setLobbyError(undefined);

    try {
      const latestMatch = await getMatch({
        baseUrl: MATCHMAKING_API_BASE_URL,
        path: { matchId: gameLaunchParameters.matchId },
      });
      const launchParameters =
        latestMatch.error || !latestMatch.data
          ? gameLaunchParameters
          : createGameLaunchParameters(latestMatch.data);

      await launchGameClient(launchParameters, true);
    } catch (caughtError) {
      console.error(caughtError);
      notifyLobbyError(t("client-game-start-error"));
    } finally {
      setGameReconnectBusy(false);
    }
  }

  if (championSelectionMatch) {
    return (
      <>
        <ChampionSelection
          currentPlayerPublicId={profilePublicId}
          match={championSelectionMatch}
          t={t}
          onChampionHover={handleChampionHover}
          onChampionSelect={handleChampionSelect}
          onPickTimeout={() => void handleChampionSelectionTimeout()}
          onReadyPhaseComplete={handleReadyPhaseComplete}
        />
        {closeDialogOpen ? (
          <CloseDialog
            t={t}
            warning={t("champion-select-close-warning")}
            onClose={onCloseDialogClose}
            onLogout={() => void handleClientLogout()}
            onQuit={() => void handleClientQuit()}
          />
        ) : null}
      </>
    );
  }

  return (
    <>
      <Sidebar
        activeLobbyId={activeLobby?.id}
        activeLobbyMemberPublicIds={[...activeLobbyMemberPublicIds]}
        forceOnlinePublicIds={forceOnlinePublicIds}
        onFriendPartyInvite={handleLobbyFriendDrop}
        onFriendPartyJoin={handleJoinFriendParty}
        onLobbyFriendDrop={handleLobbyFriendDrop}
        partyInviteEnabled={Boolean(activeLobby)}
        presenceStatus={presenceStatus}
        profileAvatarUrl={profileAvatarUrl}
        profileName={profileName}
        profilePublicId={profilePublicId}
        t={t}
      />

      {!gameSelectorOpen && !activeLobby && !gameInProgress ? (
        <button
          aria-pressed={gameSelectorOpen}
          className="client-play-button"
          data-animated={playButtonAnimated}
          type="button"
          onClick={handleTopButtonClick}
        >
          <span>{t("client-play")}</span>
        </button>
      ) : null}

      <section className="dashboard-panel" aria-label="Dashboard">
        {gameInProgress ? (
          <div className="client-game-running-message" role="status">
            <strong>
              {t(gameClientRunning ? "client-game-running" : "client-game-closed")}
            </strong>
            {!gameClientRunning && gameLaunchParameters ? (
              <button
                className="client-game-connect-button"
                disabled={gameReconnectBusy}
                type="button"
                onClick={() => void handleReconnectGameClient()}
              >
                {t(
                  gameReconnectBusy
                    ? "client-game-connect-loading"
                    : "client-game-connect",
                )}
              </button>
            ) : null}
          </div>
        ) : null}
        {gameSelectorOpen || activeLobby ? (
          <button
            className="client-page-back-button"
            type="button"
            onClick={() => void handleTopButtonClick()}
          >
            <span className="client-page-back-arrow" aria-hidden="true">
              <ArrowLeft size={20} />
            </span>
            <span>{t("client-back")}</span>
          </button>
        ) : null}
        {lobbyInvitations.length > 0 ? (
          <div className="lobby-invite-stack">
            {lobbyInvitations.map((invitation) => {
              const mainInviter = getInvitationMainInviter(invitation);
              const inviters = invitation.inviters ?? invitation.lobby?.members ?? [];

              return (
                <article className="lobby-invite-card" key={invitation.lobbyId}>
                  <div className="lobby-invite-copy">
                    <div className="lobby-invite-avatar-stack" aria-hidden="true">
                      {inviters.slice(0, 3).map((inviter, index) => (
                        <span
                          className="lobby-member-avatar lobby-invite-avatar"
                          key={inviter.publicId ?? index}
                          style={{ zIndex: 3 - index }}
                        >
                          {inviter.avatarUrl ? (
                            <img alt="" src={inviter.avatarUrl} />
                          ) : (
                            getMemberName(inviter).charAt(0).toUpperCase()
                          )}
                        </span>
                      ))}
                    </div>
                    <span>{getMemberName(mainInviter)}</span>
                    <small>{getInvitationModeLabel(invitation)}</small>
                  </div>
                  <div className="lobby-invite-actions">
                    <button
                      aria-label={t("lobby-invite-deny")}
                      type="button"
                      onClick={() => handleDeclineInvite(invitation)}
                    >
                      <X size={16} />
                    </button>
                    <button
                      aria-label={t("lobby-invite-accept")}
                      type="button"
                      onClick={() => void handleAcceptInvite(invitation)}
                    >
                      <Check size={16} />
                    </button>
                  </div>
                </article>
              );
            })}
          </div>
        ) : null}
        <div
          aria-hidden={!gameSelectorOpen}
          className={
            gameSelectorOpen
              ? "game-selector-page game-selector-page-open"
              : "game-selector-page"
          }
        >
          <div className="game-mode-grid">
            <div className="game-mode-primary">
              <article className="game-mode-card game-mode-card-primary">
                <GameModeIcon />
                <h2>Main Mode</h2>
              </article>

              <div className="game-mode-controls">
                <span
                  className="game-mode-toggle-tooltip"
                  title={t("game-mode-disabled-tooltip")}
                >
                  <button
                    aria-pressed={selectedGameMode === "normal"}
                    className="game-mode-toggle-button"
                    disabled
                    tabIndex={gameSelectorOpen ? 0 : -1}
                    type="button"
                  >
                    {t("game-mode-normal")}
                  </button>
                </span>
                <button
                  aria-pressed={selectedGameMode === "ranked"}
                  className="game-mode-toggle-button"
                  tabIndex={gameSelectorOpen ? 0 : -1}
                  type="button"
                  onClick={() => setSelectedGameMode("ranked")}
                >
                  {t("game-mode-ranked")}
                </button>
                <button
                  className="game-mode-create-button"
                  tabIndex={gameSelectorOpen ? 0 : -1}
                  type="button"
                  onClick={() => void handleCreateLobby()}
                >
                  {t("game-mode-create")}
                </button>
              </div>
            </div>

            <article className="game-mode-card game-mode-card-disabled">
              <GameModeIcon question />
              <h2>{t("game-mode-coming-soon")}</h2>
            </article>

            <article className="game-mode-card game-mode-card-disabled">
              <GameModeIcon question />
              <h2>{t("game-mode-coming-soon")}</h2>
            </article>
          </div>
        </div>

        {activeLobby ? (
          <section className="lobby-page" aria-label={t("lobby-title")}>
            <div className="lobby-id-info">
              <button
                aria-label={t("lobby-id")}
                className="lobby-id-info-button"
                type="button"
                onClick={(event) => {
                  event.stopPropagation();
                  setLobbyIdContextMenuOpen((open) => !open);
                }}
              >
                <Info size={18} />
              </button>
              {lobbyIdContextMenuOpen ? (
                <div
                  className="lobby-id-context-menu"
                  role="menu"
                  onClick={(event) => event.stopPropagation()}
                  onMouseDown={(event) => event.stopPropagation()}
                >
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => void handleCopyLobbyId()}
                  >
                    <Copy size={15} />
                    <span>{t("lobby-id-copy")}</span>
                  </button>
                </div>
              ) : null}
            </div>

            <div className="lobby-slots" data-lobby-invite-drop="true">
              {playerSlots.map((slot) => {
                const member = lobbySlotMembers[slot];
                const lobbyHost = getLobbyHost(activeLobby);
                const isCurrentUser = member?.publicId === profilePublicId;
                const isHost = member?.publicId === lobbyHost?.publicId;
                const canInviteSlot = !member;
                const canOpenMemberMenu = Boolean(member);
                const memberName = member
                  ? getMemberName(member)
                  : isCurrentUser
                    ? getLobbyDisplayName(profileName)
                    : undefined;

                return (
                  <div
                    className={
                      [
                        "lobby-player-slot",
                        isCurrentUser ? "lobby-player-slot-owner" : "",
                        isHost ? "lobby-player-slot-host" : "",
                        member ? "lobby-player-slot-filled" : "lobby-player-slot-empty",
                      ]
                        .filter(Boolean)
                        .join(" ")
                    }
                    key={slot}
                  >
                    <div
                      className={
                        canOpenMemberMenu
                          ? "lobby-player-circle lobby-player-circle-member"
                          : canInviteSlot
                            ? "lobby-player-circle lobby-player-circle-inviteable"
                          : "lobby-player-circle"
                      }
                      role={canOpenMemberMenu || canInviteSlot ? "button" : undefined}
                      tabIndex={canOpenMemberMenu || canInviteSlot ? 0 : undefined}
                      onClick={
                        member
                          ? (event) => {
                              event.stopPropagation();
                              openLobbyMemberContextMenu(member, event.currentTarget);
                            }
                          : canInviteSlot
                            ? openPartyInviteDialog
                          : undefined
                      }
                      onKeyDown={
                        member
                          ? (event) => {
                              if (event.key === "Enter" || event.key === " ") {
                                event.preventDefault();
                                openLobbyMemberContextMenu(member, event.currentTarget);
                              }
                            }
                          : canInviteSlot
                            ? (event) => {
                                if (event.key === "Enter" || event.key === " ") {
                                  event.preventDefault();
                                  openPartyInviteDialog();
                                }
                              }
                          : undefined
                      }
                    >
                      <span className="lobby-player-avatar-content">
                        {member ? (
                          member.avatarUrl ? (
                            <img alt="" src={member.avatarUrl} />
                          ) : member.publicId === profilePublicId && profileAvatarUrl ? (
                            <img alt="" src={profileAvatarUrl} />
                          ) : (
                            getMemberName(member).charAt(0).toUpperCase()
                          )
                        ) : (
                          <Plus size={28} />
                        )}
                      </span>
                    </div>
                    {isHost ? (
                      <span className="lobby-host-crown" title="Host">
                        <Crown size={18} />
                      </span>
                    ) : null}
                    <div className="lobby-player-info">
                      <span>{memberName ?? t("lobby-slot-open")}</span>
                      <small>
                        {member
                          ? isHost
                            ? "Host"
                            : t("lobby-slot-ready")
                          : t("lobby-slot-invite")}
                      </small>
                    </div>
                    {isCurrentUser ? (
                        <div className="lobby-owner-actions">
                          <button type="button" onClick={openPartyInviteDialog}>
                            <Plus size={18} />
                          </button>
                          <button type="button" onClick={openPartyInviteDialog}>
                            <Plus size={18} />
                          </button>
                        </div>
                    ) : null}
                  </div>
                );
              })}
            </div>

            <div
              className={
                lobbyIsSearching
                  ? "lobby-search-control lobby-search-control-active"
                  : "lobby-search-control"
              }
            >
              <div className="lobby-search-timer" aria-live="polite">
                <span>{lobbySearchTime}</span>
              </div>
              <button
                className="lobby-search-button"
                disabled={!isCurrentUserLobbyHost}
                type="button"
                onClick={() => void handleLobbySearch()}
              >
                {lobbyIsSearching ? t("lobby-search-abort") : t("lobby-search")}
              </button>
            </div>

          </section>
        ) : null}
      </section>

      {lobbyMemberContextMenu && activeLobby ? (() => {
        const member = lobbyMemberContextMenu.member;
        const memberPublicId = member.publicId;
        const isSelf = memberPublicId === profilePublicId;
        const lobbyHost = getLobbyHost(activeLobby);
        const isCurrentUserHost = lobbyHost?.publicId === profilePublicId;
        const isFriend =
          typeof memberPublicId === "number" && friendPublicIds.has(memberPublicId);
        const actionBusy = lobbyMemberActionBusyId === memberPublicId;

        return (
          <div
            className="lobby-member-context-menu"
            role="menu"
            style={{
              left: lobbyMemberContextMenu.left,
              top: lobbyMemberContextMenu.top,
            }}
            onClick={(event) => event.stopPropagation()}
            onMouseDown={(event) => event.stopPropagation()}
          >
            <button
              type="button"
              role="menuitem"
              onClick={handleViewLobbyMemberProfile}
            >
              {t("lobby-member-view-profile")}
            </button>
            {!isSelf && !isFriend ? (
              <button
                disabled={actionBusy}
                type="button"
                role="menuitem"
                onClick={() => void handleAddLobbyMemberFriend(member)}
              >
                {t("lobby-member-add-friend")}
              </button>
            ) : null}
            {isCurrentUserHost && !isSelf ? (
              <button
                disabled={actionBusy}
                type="button"
                role="menuitem"
                onClick={() => void handleMakeLobbyHost(member)}
              >
                {t("lobby-member-make-host")}
              </button>
            ) : null}
            {isCurrentUserHost && !isSelf ? (
              <button
                className="danger"
                disabled={actionBusy}
                type="button"
                role="menuitem"
                onClick={() => void handleKickMember(member)}
              >
                {t("lobby-member-kick-player")}
              </button>
            ) : null}
          </div>
        );
      })() : null}

      {partyInviteOpen && activeLobby ? (
        <div
          className="dialog-backdrop friend-add-dialog-backdrop lobby-party-invite-dialog-backdrop"
          role="presentation"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) {
              setPartyInviteOpen(false);
            }
          }}
        >
          <div
            aria-modal="true"
            className="friend-add-dialog lobby-party-invite-dialog"
            role="dialog"
            onMouseDown={(event) => event.stopPropagation()}
          >
            <div className="friend-add-dialog-header">
              <h2>{t("lobby-invite-dialog-title")}</h2>
              <button
                aria-label={t("client-close")}
                className="friend-add-close-button"
                type="button"
                onClick={() => setPartyInviteOpen(false)}
              >
                <X size={18} />
              </button>
            </div>

            <label className="friend-add-search">
              <Search size={16} />
              <input
                aria-label={t("lobby-invite-search")}
                autoFocus
                placeholder={t("lobby-invite-search")}
                value={partyInviteSearch}
                onChange={(event) => setPartyInviteSearch(event.target.value)}
              />
              {partyInviteSearching ? (
                <span>{t("friend-add-searching")}</span>
              ) : null}
            </label>

            <div className="friend-add-list">
              {inviteCandidates.length > 0 ? (
                inviteCandidates.map((candidate) => {
                  const candidateKey = getInviteCandidateKey(candidate);
                  const candidateSubtitle = getInviteCandidateSubtitle(candidate);
                  const candidateInLobby =
                    typeof candidate.publicId === "number" &&
                    activeLobbyMemberPublicIds.has(candidate.publicId);
                  const canInvite =
                    typeof candidate.publicId === "number" && !candidateInLobby;

                  return (
                    <div className="friend-add-row" key={candidateKey}>
                      <span className="friend-add-avatar" aria-hidden="true">
                        {getProfileInitials(candidate.name)}
                        {candidate.avatarUrl ? (
                          <img
                            alt=""
                            className="friend-avatar-image"
                            referrerPolicy="no-referrer"
                            src={candidate.avatarUrl}
                            onError={(event) => {
                              event.currentTarget.hidden = true;
                            }}
                          />
                        ) : null}
                      </span>
                      <span className="friend-add-row-copy">
                        <span>{candidate.name}</span>
                        <span>
                          {candidateSubtitle}
                          {candidate.source === "friend" ? " · FL" : ""}
                        </span>
                      </span>
                      <button
                        className="friend-add-action-button"
                        disabled={!canInvite || partyInviteBusyId === candidate.publicId}
                        type="button"
                        onClick={() => void handleInviteCandidate(candidate)}
                      >
                        {candidateInLobby
                          ? t("lobby-invite-already-in-lobby")
                          : t("lobby-invite-player")}
                      </button>
                    </div>
                  );
                })
              ) : (
                <p className="friend-add-empty">
                  {partyInviteSearch.trim().length >= 2
                    ? t("friend-add-no-results")
                    : t("lobby-invite-empty")}
                </p>
              )}
            </div>
          </div>
        </div>
      ) : null}

      {pendingMatch ? (
        <div className="match-found-backdrop" role="presentation">
          <section
            aria-labelledby="match-found-title"
            aria-modal="true"
            className="match-found-modal"
            role="dialog"
          >
            <div
              className="match-found-countdown"
              style={
                {
                  "--match-found-progress-angle": `${matchFoundProgress * 360}deg`,
                } as CSSProperties
              }
            >
              <div className="match-found-countdown-core">
                <h2 id="match-found-title">{t("match-found-title")}</h2>
                <span>{matchFoundRemainingSeconds}</span>
                <small>
                  {matchFoundAcceptedCount}/{matchFoundMaxAcceptCount}
                </small>
              </div>
            </div>
            <div className="match-found-actions">
              <button
                className="match-found-accept"
                disabled={Boolean(matchDecisionBusy) || currentPlayerAccepted}
                type="button"
                onClick={() => void handleMatchDecision("accept")}
              >
                {currentPlayerAccepted
                  ? t("match-found-waiting")
                  : t("match-found-accept")}
              </button>
              <button
                className="match-found-decline"
                disabled={Boolean(matchDecisionBusy) || currentPlayerAccepted}
                type="button"
                onClick={() => void handleMatchDecision("decline")}
              >
                {t("match-found-decline")}
              </button>
            </div>
          </section>
        </div>
      ) : null}

      {closeDialogOpen ? (
        <CloseDialog
          t={t}
          onClose={onCloseDialogClose}
          onLogout={() => void handleClientLogout()}
          onQuit={() => void handleClientQuit()}
        />
      ) : null}

      {settingsOpen ? (
        <SettingsModal
          accentColor={accentColor}
          allowFriendRequests={allowFriendRequests}
          clientAnimation={clientAnimation}
          locale={locale}
          resolution={resolution}
          supportsFourKResolution={supportsFourKResolution}
          supportsTwoKResolution={supportsTwoKResolution}
          t={t}
          vision="Vision.ALL"
          onAccentColorChange={onAccentColorChange}
          onAllowFriendRequestsChange={onAllowFriendRequestsChange}
          onClientAnimationChange={onClientAnimationChange}
          onClose={onSettingsClose}
          onLocaleChange={onLocaleChange}
          onResolutionChange={onResolutionChange}
        />
      ) : null}
    </>
  );
}

export default Client;
