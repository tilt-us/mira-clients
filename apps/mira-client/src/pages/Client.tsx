import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type MouseEvent,
  type PointerEvent,
} from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  ArrowLeft,
  Check,
  ChevronLeft,
  ChevronRight,
  Copy,
  Crown,
  Info,
  Pencil,
  Plus,
  Search,
  SlidersHorizontal,
  X,
} from "lucide-react";
import {
  abortRankedSearch,
  abortSearch,
  accept,
  bootstrap as liveBootstrap,
  cancelChampionPhase,
  cancelChampionPhaseDuplicate,
  clearChampionHover,
  clearChampionHoverDuplicate,
  client,
  createRankedLobby,
  deleteChatRoom,
  decide,
  decline,
  get as getMatch,
  getUserSettingsSummary,
  hoverChampion,
  hoverChampionDuplicate,
  invite,
  invitations as listLobbyInvitations,
  listRooms as listChatRooms,
  joinLobby,
  kickMember,
  leaveLobby,
  liveSendRequest,
  markChampionsReady,
  markChampionsReadyDuplicate,
  notifyChampionSelectionLeft,
  online as listOnlineUsers,
  searchRanked,
  search as searchUsers,
  selectChampion,
  selectChampionDuplicate,
  startSearch,
  temporaryMatches,
  transferHost,
  usersByPublicIds,
  getLobbyRoles,
  updateMe,
  updateLobbyMemberRoles,
  userStatusMe,
  type ApiMatchResponse,
  type ChatRoomResponse,
  type FriendUserResponse,
  type LobbyInvitation,
  type LobbyMember,
  type LobbySnapshot,
  type MatchPlayerResponse,
  type UserStatusSnapshot,
} from "../api/client";
import {
  API_BASE_URL,
  CHAT_API_BASE_URL,
  LIVE_API_BASE_URL,
  MATCHMAKING_API_BASE_URL,
} from "../api/config";
import { getValidAccessToken } from "../auth/keycloak";
import ChampionSelection from "./ChampionSelection";
import ChatDock from "../components/ChatDock";
import CloseDialog from "../components/CloseDialog";
import ProfileChampionsTab from "../components/ProfileChampionsTab";
import MatchFoundDialog from "../components/MatchFoundDialog";
import SettingsModal from "../components/SettingsModal";
import Sidebar from "../components/Sidebar";
import liraWallpaper from "../../../../assets/wallpapers/lira-wallpaper.png";
import type { AppLocale } from "../i18n";
import { useNotifications } from "../notifications";
import type {
  AppResolution,
  BackgroundChampion,
  ChatPosition,
  ClientAnimation,
  ClientSettingsFolder,
  FriendRequestPolicy,
  GameScreenMode,
  UiScale,
} from "../settings";
import type { FriendProfile, PresenceStatus, Translate } from "../types/ui";
import {
  formatTagId,
  getProfileInitials,
  getPublicAvatarUrl,
  getPublicDisplayName,
  normalizeTagId,
} from "../utils/profile";
import {
  clearStoredGameSession,
  createGameMatchManifest,
  getGameClientChampionId,
  getMatchChampionForPlayer,
  getMatchControlBaseUrl,
  getMatchHost,
  getMatchPort,
  getMatchTeamForPlayer,
  readStoredGameSession,
  sendChampionSelectionLeaveKeepalive,
  sendCancelChampionPhaseKeepalive,
  writeStoredGameSession,
  type GameClientStatus,
  type GameLaunchParameters,
  type LaunchGameRequest,
} from "../gameSession";
import {
  PARTY_INVITE_ONLINE_LIMIT,
  afkDelayMs,
  areAllChampionsSelected,
  findChampionSelectionPlayerLeftEvent,
  findDesktopSessionConflictEvent,
  findLobbyInvitation,
  findLobbyRolesSnapshot,
  findLobbySnapshot,
  findMatchResponse,
  findUserStatusSnapshot,
  formatLobbySearchTime,
  GameModeIcon,
  getChampionSelectionTeamChatRoom,
  getChampionSelectionLobbyChatRoom,
  getCurrentLobbyMember,
  getDesktopSessionConflictKey,
  getErrorMessage,
  getInvitationMainInviter,
  getInvitationModeLabel,
  getInviteCandidateKey,
  getInviteCandidateSubtitle,
  getLobbyChatRoom,
  getLobbyDisplayName,
  getLobbyHost,
  getLobbyLeaveColor,
  getLobbyMemberPublicAvatarUrl,
  getMemberName,
  getLobbyMemberDisplayIdentity,
  getLobbyMemberNameTag,
  getLobbyModeLabel,
  getLobbySlotMembers,
  getMatchFoundOverlayStroke,
  getFriendUserLevel,
  getFriendUserName,
  getFriendUserTagId,
  getShortestRotationDegrees,
  getUserPageNameClassName,
  enrichMatchPlayers,
  isActivePresenceStatus,
  isFinishedMatchStatus,
  isInviteablePresence,
  isMatchForLobby,
  isMatchGameStarted,
  isMatchReady,
  isWarmupMatch,
  isSameLobbyMember,
  isWarmupActive,
  mapFriendToInviteCandidate,
  mapFriendUserToProfile,
  mapLobbyToMatchPlayers,
  mapOnlineUserToInviteCandidate,
  mapUserStatusToPresence,
  mapUserToInviteCandidate,
  matchAcceptTimeoutMs,
  matchFoundHexSpinDurationMs,
  matchFoundRequiredAcceptCount,
  mergeLobbyInvitations,
  mergeKnownMatchPlayer,
  mergeMatchChampionHovers,
  normalizeLobbyIdentityName,
  normalizeLobbyInvitation,
  normalizeMatchResponse,
  parseApiTimestamp,
  sendPresenceKeepalive,
  shouldTreatChampionSelectionAsWarmup,
  toPublicId,
  userPageCategories,
  withServerEventType,
  type ApiPresenceStatus,
  type ChampionSelectionLeaveStatus,
  type ChampionSelectionPlayerLeftEvent,
  type CurrentMatchPlayerProfile,
  type LobbyMemberContextMenuState,
  type MatchDecision,
  type OnlineInviteUser,
  type PartyInviteCandidate,
  type PresenceSnapshot,
  type UserPageCategory,
  type UserPageProfile,
} from "./Client.helpers";
import {
  getLobbyPresenceMode,
  getLobbyRoleLimitError,
  getLobbyRolesFromPresenceMode,
  getMemberLobbyRoles,
  hasLobbyRoles,
  LobbyRoleIcon,
  lobbyRoles,
  normalizeLobbyRoleSelection,
  toApiLobbyRole,
  type GameMode,
  type LobbyMemberWithRoles,
  type LobbyRoleId,
  type LobbyRoleSelection,
} from "../lobbyRoles";

type ClientProps = {
  accentColor: string;
  backgroundChampion: BackgroundChampion;
  chatPosition: ChatPosition;
  clientSettingsFolders: ClientSettingsFolder[];
  clientAnimation: ClientAnimation;
  friendRequestPolicy: FriendRequestPolicy;
  closeDialogOpen: boolean;
  gameScreenMode: GameScreenMode;
  locale: AppLocale;
  onAccentColorChange: (accentColor: string) => void;
  onBackgroundChampionChange: (backgroundChampion: BackgroundChampion) => void;
  onChatPositionChange: (chatPosition: ChatPosition) => void;
  onClientSettingsFoldersChange: (folders: ClientSettingsFolder[]) => void;
  onClientAnimationChange: (clientAnimation: ClientAnimation) => void;
  onCloseDialogClose: () => void;
  onFriendRequestPolicyChange: (friendRequestPolicy: FriendRequestPolicy) => void;
  onGameScreenModeChange: (gameScreenMode: GameScreenMode) => void;
  onLocaleChange: (locale: AppLocale) => void;
  onLogout: () => void | Promise<void>;
  onQuit: () => void;
  onResolutionChange: (resolution: AppResolution) => void;
  onSettingsClose: () => void;
  onShowEmailPublicChange: (showEmailPublic: boolean) => void;
  onUiScaleChange: (uiScale: UiScale) => void;
  profileAvatarUrl?: string;
  profileLevel: number;
  profileName: string;
  profilePublicId?: number;
  profileTagId?: string;
  resolution: AppResolution;
  showEmailPublic: boolean;
  settingsOpen: boolean;
  supportsFourKResolution: boolean;
  supportsTwoKResolution: boolean;
  t: Translate;
  uiScale: UiScale;
};

type ClientBackTarget = "main" | "gameSelector" | "lobby";

function Client({
  accentColor,
  backgroundChampion,
  chatPosition,
  clientSettingsFolders,
  clientAnimation,
  friendRequestPolicy,
  closeDialogOpen,
  gameScreenMode,
  locale,
  onAccentColorChange,
  onBackgroundChampionChange,
  onChatPositionChange,
  onClientSettingsFoldersChange,
  onClientAnimationChange,
  onCloseDialogClose,
  onFriendRequestPolicyChange,
  onGameScreenModeChange,
  onLocaleChange,
  onLogout,
  onQuit,
  onResolutionChange,
  onSettingsClose,
  onShowEmailPublicChange,
  onUiScaleChange,
  profileAvatarUrl,
  profileLevel,
  profileName,
  profilePublicId,
  profileTagId,
  resolution,
  showEmailPublic,
  settingsOpen,
  supportsFourKResolution,
  supportsTwoKResolution,
  t,
  uiScale,
}: ClientProps) {
  const [gameSelectorOpen, setGameSelectorOpen] = useState(false);
  const [lobbyPageOpen, setLobbyPageOpen] = useState(false);
  const [gameSelectorBackTarget, setGameSelectorBackTarget] =
    useState<ClientBackTarget>("main");
  const [lobbyPageBackTarget, setLobbyPageBackTarget] =
    useState<ClientBackTarget>("main");
  const [lobbyRosterOpen, setLobbyRosterOpen] = useState(false);
  const [userPageOpen, setUserPageOpen] = useState(false);
  const [viewedUserPageProfile, setViewedUserPageProfile] =
    useState<UserPageProfile>();
  const [activeUserPageCategory, setActiveUserPageCategory] =
    useState<UserPageCategory>("overview");
  const [championTabFocused, setChampionTabFocused] = useState(false);
  const [championTabBackSignal, setChampionTabBackSignal] = useState(0);
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
  const [partyInviteOnlineUsers, setPartyInviteOnlineUsers] = useState<
    OnlineInviteUser[]
  >([]);
  const [userEmailVisibilityByPublicId, setUserEmailVisibilityByPublicId] =
    useState<Map<number, boolean>>(new Map());
  const [publicUsersByPublicId, setPublicUsersByPublicId] = useState<
    Map<number, FriendUserResponse>
  >(new Map());
  const [partyInviteOnlinePage, setPartyInviteOnlinePage] = useState(0);
  const [partyInviteSearching, setPartyInviteSearching] = useState(false);
  const [partyInviteBusyId, setPartyInviteBusyId] = useState<number>();
  const [selectedLobbyRoles, setSelectedLobbyRoles] =
    useState<LobbyRoleSelection>([undefined, undefined]);
  const [lobbyMemberRoles, setLobbyMemberRoles] = useState<
    Record<number, LobbyRoleSelection>
  >({});
  const [openLobbyRolePicker, setOpenLobbyRolePicker] = useState<0 | 1>();
  const [lobbyMemberContextMenu, setLobbyMemberContextMenu] =
    useState<LobbyMemberContextMenuState>();
  const [lobbyMemberActionBusyId, setLobbyMemberActionBusyId] = useState<number>();
  const [lobbyIdContextMenuOpen, setLobbyIdContextMenuOpen] = useState(false);
  const [lobbySearchStartedAt, setLobbySearchStartedAt] = useState<number>();
  const [lobbySearchAbortedLobbyId, setLobbySearchAbortedLobbyId] = useState<string>();
  const [lobbySearchNow, setLobbySearchNow] = useState(Date.now());
  const [pendingMatch, setPendingMatch] = useState<ApiMatchResponse>();
  const [championSelectionMatch, setChampionSelectionMatch] =
    useState<ApiMatchResponse>();
  const [gameLaunchParameters, setGameLaunchParameters] =
    useState<GameLaunchParameters>();
  const [gameClientRunning, setGameClientRunning] = useState(false);
  const [gameClientClosedByClient, setGameClientClosedByClient] = useState(false);
  const [gameReconnectBusy, setGameReconnectBusy] = useState(false);
  const [matchDecisionBusy, setMatchDecisionBusy] = useState<MatchDecision>();
  const [matchFoundStartedAt, setMatchFoundStartedAt] = useState<number>();
  const [matchFoundNow, setMatchFoundNow] = useState(Date.now());
  const [matchFoundServerClockOffsetMs, setMatchFoundServerClockOffsetMs] =
    useState(0);
  const [matchFoundHexAligned, setMatchFoundHexAligned] = useState(false);
  const [matchAutoDeclinedId, setMatchAutoDeclinedId] = useState<string>();
  const [championsReadyMarkedMatchId, setChampionsReadyMarkedMatchId] = useState<string>();
  const [forceOnlinePublicIds, setForceOnlinePublicIds] = useState<number[]>([]);
  const activeLobbyRef = useRef<LobbySnapshot | undefined>(undefined);
  const championSelectionMatchRef = useRef<ApiMatchResponse | undefined>(undefined);
  const gameInProgressRef = useRef(false);
  const gameLaunchParametersRef = useRef<GameLaunchParameters | undefined>(undefined);
  const [presenceStatus, setPresenceStatus] = useState<PresenceStatus>("online");
  const lastActivityRef = useRef(Date.now());
  const hiddenSinceRef = useRef<number | undefined>(undefined);
  const remotePresenceRef = useRef<string | undefined>(undefined);
  const currentPresenceRef = useRef<PresenceSnapshot>({ status: "ONLINE" });
  const topActionDragTimerRef = useRef<number | undefined>(undefined);
  const topActionDraggingRef = useRef(false);
  const presenceInitializedRef = useRef(false);
  const shuttingDownRef = useRef(false);
  const championSelectionTimeoutInFlightRef = useRef(false);
  const requeueingLobbyIdsRef = useRef<Set<string>>(new Set());
  const declinedLobbyInvitationTimestampsRef = useRef<Map<string, number>>(new Map());
  const seenDesktopSessionConflictIdsRef = useRef<Set<string>>(new Set());
  const playButtonAnimated =
    clientAnimation === "all" || clientAnimation === "ui-elements";
  const lobbyLeaveColor = getLobbyLeaveColor(accentColor);
  const { notify } = useNotifications();

  function getClientChatRoomId(room: ChatRoomResponse) {
    const runtimeRoom = room as ChatRoomResponse & { id?: unknown };

    return (
      (typeof room.roomId === "string" && room.roomId) ||
      (typeof runtimeRoom.id === "string" && runtimeRoom.id) ||
      undefined
    );
  }

  function getClientChatRoomType(room: ChatRoomResponse) {
    return room.type?.toUpperCase();
  }

  function getClientChatRoomParticipantPublicIds(room: ChatRoomResponse) {
    return (
      room.participantPublicIds
        ?.map((publicId) => toPublicId(publicId))
        .filter((publicId): publicId is number => typeof publicId === "number") ?? []
    );
  }

  function getClientChatRooms(data: unknown): ChatRoomResponse[] {
    if (Array.isArray(data)) {
      return data as ChatRoomResponse[];
    }

    if (data && typeof data === "object") {
      const rooms = (data as { rooms?: unknown }).rooms;

      if (Array.isArray(rooms)) {
        return rooms as ChatRoomResponse[];
      }
    }

    return [];
  }

  function chatRoomContextMatches(roomContextId: string | undefined, contextId: string) {
    const normalizedRoomContextId = roomContextId?.toUpperCase();
    const normalizedContextId = contextId.toUpperCase();

    if (!normalizedRoomContextId) {
      return false;
    }

    return (
      normalizedRoomContextId === normalizedContextId ||
      normalizedRoomContextId.startsWith(`${normalizedContextId}:`) ||
      normalizedRoomContextId.endsWith(`:${normalizedContextId}`) ||
      normalizedRoomContextId.includes(`:${normalizedContextId}:`)
    );
  }

  async function getCurrentUserGroupChatRoomIds(
    roomTypes: readonly ("LOBBY" | "TEAM")[],
    contextIds: readonly string[],
  ) {
    if (typeof profilePublicId !== "number" || contextIds.length === 0) {
      return [];
    }

    const roomsResult = await listChatRooms({
      baseUrl: CHAT_API_BASE_URL,
    }).catch(() => undefined);

    if (!roomsResult || roomsResult.error) {
      return [];
    }

    const roomTypeSet = new Set(roomTypes);

    return getClientChatRooms(roomsResult.data)
      .filter((room) => {
        const roomId = getClientChatRoomId(room);
        const roomType = getClientChatRoomType(room);
        const participants = getClientChatRoomParticipantPublicIds(room);

        return (
          Boolean(roomId) &&
          roomTypeSet.has(roomType as "LOBBY" | "TEAM") &&
          participants.includes(profilePublicId) &&
          contextIds.some((contextId) =>
            chatRoomContextMatches(room.contextId, contextId),
          )
        );
      })
      .map(getClientChatRoomId)
      .filter((roomId): roomId is string => Boolean(roomId));
  }

  async function getCurrentUserGroupChatRooms(
    roomTypes: readonly ("LOBBY" | "TEAM")[],
    contextIds: readonly string[],
  ) {
    if (typeof profilePublicId !== "number" || contextIds.length === 0) {
      return [];
    }

    const roomsResult = await listChatRooms({
      baseUrl: CHAT_API_BASE_URL,
    }).catch(() => undefined);

    if (!roomsResult || roomsResult.error) {
      return [];
    }

    const roomTypeSet = new Set(roomTypes);

    return getClientChatRooms(roomsResult.data).filter((room) => {
      const roomId = getClientChatRoomId(room);
      const roomType = getClientChatRoomType(room);
      const participants = getClientChatRoomParticipantPublicIds(room);

      return (
        Boolean(roomId) &&
        roomTypeSet.has(roomType as "LOBBY" | "TEAM") &&
        participants.includes(profilePublicId) &&
        contextIds.some((contextId) =>
          chatRoomContextMatches(room.contextId, contextId),
        )
      );
    });
  }

  function currentUserIsOnlyChatRoomParticipant(room: ChatRoomResponse) {
    const participants = getClientChatRoomParticipantPublicIds(room);

    return (
      typeof profilePublicId === "number" &&
      participants.length > 0 &&
      participants.every((participantPublicId) => participantPublicId === profilePublicId)
    );
  }

  async function deleteChatRoomsById(roomIds: readonly string[]) {
    await Promise.all(
      [...new Set(roomIds)].map((roomId) =>
        deleteChatRoom({
          baseUrl: CHAT_API_BASE_URL,
          path: { roomId },
        }).catch(() => undefined),
      ),
    );
  }

  async function deleteCurrentUserGroupChatRooms(
    roomTypes: readonly ("LOBBY" | "TEAM")[],
    contextIds: readonly string[],
  ) {
    await deleteChatRoomsById(
      await getCurrentUserGroupChatRoomIds(roomTypes, contextIds),
    );
  }

  function currentUserIsLastLobbyMember(lobby: LobbySnapshot) {
    return (
      typeof profilePublicId === "number" &&
      (lobby.members?.length ?? 0) > 0 &&
      (lobby.members ?? []).every((member) => member.publicId === profilePublicId)
    );
  }

  function getCurrentMatchLobbyId(match: ApiMatchResponse | undefined) {
    if (typeof profilePublicId !== "number") {
      return undefined;
    }

    return match?.lobbies?.find((lobby) => {
      return lobby.players?.some((player) => player.publicId === profilePublicId);
    })?.lobbyId;
  }

  async function deleteChampionSelectionChatRooms(
    match: ApiMatchResponse | undefined,
  ) {
    const contextIds = [
      match?.matchId,
      getCurrentMatchLobbyId(match),
    ].filter((contextId): contextId is string => Boolean(contextId));

    await deleteCurrentUserGroupChatRooms(["TEAM", "LOBBY"], contextIds);
  }

  useEffect(() => {
    return () => {
      clearTopActionDragTimer();
    };
  }, []);

  useEffect(() => {
    function handleEscapeBack(event: globalThis.KeyboardEvent) {
      if (event.key !== "Escape" || closeDialogOpen || settingsOpen) {
        return;
      }

      if (championTabFocused || userPageOpen || lobbyPageOpen || gameSelectorOpen) {
        event.preventDefault();
        handleTopButtonClick();
      }
    }

    window.addEventListener("keydown", handleEscapeBack);

    return () => {
      window.removeEventListener("keydown", handleEscapeBack);
    };
  }, [
    closeDialogOpen,
    championTabFocused,
    gameSelectorBackTarget,
    gameSelectorOpen,
    lobbyPageBackTarget,
    lobbyPageOpen,
    settingsOpen,
    userPageOpen,
  ]);

  const selfUserPageProfile = useMemo<UserPageProfile>(
    () => ({
      avatarUrl: profileAvatarUrl,
      level: profileLevel,
      name: profileName,
      publicId: profilePublicId,
      tagId: profileTagId,
    }),
    [profileAvatarUrl, profileLevel, profileName, profilePublicId, profileTagId],
  );
  const activeUserPageProfile = viewedUserPageProfile ?? selfUserPageProfile;
  const userPageShowsSelf = !viewedUserPageProfile;
  const currentMatchPlayerProfile = useMemo<CurrentMatchPlayerProfile>(
    () => ({
      avatarUrl: profileAvatarUrl,
      displayName: getPublicDisplayName(profileName, "User"),
      publicId: profilePublicId,
    }),
    [profileAvatarUrl, profileName, profilePublicId],
  );
  const lobbyPublicUserIds = useMemo(() => {
    const publicIds = new Set<number>();
    const addPublicId = (publicId?: number) => {
      if (
        typeof publicId === "number" &&
        publicId !== profilePublicId
      ) {
        publicIds.add(publicId);
      }
    };

    activeLobby?.members?.forEach((member) => addPublicId(member.publicId));
    lobbyInvitations.forEach((invitation) => {
      invitation.inviters?.forEach((member) => addPublicId(member.publicId));
      invitation.lobby?.members?.forEach((member) => addPublicId(member.publicId));
    });
    championSelectionMatch?.lobbies?.forEach((lobby) => {
      lobby.players?.forEach((player) => addPublicId(player.publicId));
    });

    return [...publicIds].sort((left, right) => left - right);
  }, [activeLobby, championSelectionMatch, lobbyInvitations, profilePublicId]);
  const lobbyPublicUserIdKey = lobbyPublicUserIds.join(",");

  useEffect(() => {
    let active = true;

    if (lobbyPublicUserIds.length === 0) {
      setPublicUsersByPublicId((currentUsers) =>
        currentUsers.size === 0 ? currentUsers : new Map(),
      );
      return () => {
        active = false;
      };
    }

    void usersByPublicIds({
      baseUrl: API_BASE_URL,
      query: { publicIds: lobbyPublicUserIds },
    }).then((result) => {
      if (!active || result.error) {
        return;
      }

      setPublicUsersByPublicId(
        new Map(
          (result.data?.users ?? [])
            .filter(
              (user): user is FriendUserResponse & { publicId: number } =>
                typeof user.publicId === "number",
            )
            .map((user) => [user.publicId, user]),
        ),
      );
    });

    return () => {
      active = false;
    };
  }, [lobbyPublicUserIdKey]);
  const playerSlots = Array.from({ length: 5 }, (_, index) => index);
  const lobbySlotMembers = activeLobby ? getLobbySlotMembers(activeLobby) : [];
  const lobbyRosterMembers = lobbySlotMembers.filter(
    (member): member is LobbyMember => Boolean(member),
  );
  const lobbyIsFull =
    lobbySlotMembers.filter((member): member is LobbyMember => Boolean(member)).length >=
    playerSlots.length;
  const visibleLobbyRoleSlots: readonly (0 | 1)[] = lobbyIsFull ? [0] : [0, 1];
  const activeLobbyCurrentMember = getCurrentLobbyMember(
    activeLobby,
    profilePublicId,
    profileName,
  );
  const lobbyChatRooms = useMemo(() => {
    const lobbyChatRoom = getLobbyChatRoom(
      activeLobby,
      t,
      currentMatchPlayerProfile,
      publicUsersByPublicId,
    );

    return lobbyChatRoom ? [lobbyChatRoom] : [];
  }, [activeLobby, currentMatchPlayerProfile, publicUsersByPublicId, t]);
  const championSelectionChatRooms = useMemo(() => {
    const teamChatRoom = getChampionSelectionTeamChatRoom(
      championSelectionMatch,
      currentMatchPlayerProfile,
      t,
      publicUsersByPublicId,
    );
    const championSelectionLobbyChatRoom =
      lobbyChatRooms[0] ??
      getChampionSelectionLobbyChatRoom(
        championSelectionMatch,
        currentMatchPlayerProfile,
        t,
        publicUsersByPublicId,
      );
    const championSelectionLobbyChatRooms = championSelectionLobbyChatRoom
      ? [championSelectionLobbyChatRoom]
      : [];

    return teamChatRoom
      ? [teamChatRoom, ...championSelectionLobbyChatRooms]
      : championSelectionLobbyChatRooms;
  }, [
    championSelectionMatch,
    currentMatchPlayerProfile,
    lobbyChatRooms,
    publicUsersByPublicId,
    t,
  ]);
  const activeLobbyHost = activeLobby ? getLobbyHost(activeLobby) : undefined;
  const isCurrentUserLobbyHost = isSameLobbyMember(
    activeLobbyHost,
    activeLobbyCurrentMember,
  );
  const lobbyIsSearching =
    Boolean(lobbySearchStartedAt) ||
    (activeLobby?.status === "SEARCHING" &&
      activeLobby.id !== lobbySearchAbortedLobbyId);
  const partyInvitesLocked = lobbyIsSearching || activeLobby?.status === "SEARCHING";
  const lobbySearchSeconds = lobbySearchStartedAt
    ? Math.max(0, Math.floor((lobbySearchNow - lobbySearchStartedAt) / 1000))
    : 0;
  const lobbySearchTime = formatLobbySearchTime(lobbySearchSeconds);
  const allLobbyMembersHaveRoles =
    !activeLobby?.members?.length ||
    activeLobby.members.every((member) => {
      const roles = getEffectiveLobbyMemberRoles(member);

      return Boolean(roles[0] && (lobbyIsFull || roles[1]));
    });
  const lobbySearchDisabledByRoles =
    Boolean(activeLobby?.id) &&
    isCurrentUserLobbyHost &&
    !lobbyIsSearching &&
    !allLobbyMembersHaveRoles;
  const currentPlayerAcceptance = pendingMatch?.acceptances?.find((acceptance) => {
    return acceptance.playerPublicId === profilePublicId;
  });
  const currentPlayerAccepted = currentPlayerAcceptance?.status === "ACCEPTED";
  const matchFoundServerNowMs = matchFoundNow + matchFoundServerClockOffsetMs;
  const matchFoundPhaseEndsAtMs = parseApiTimestamp(pendingMatch?.phaseEndsAt);
  const matchFoundFallbackElapsedMs = matchFoundStartedAt
    ? Math.max(0, matchFoundNow - matchFoundStartedAt)
    : 0;
  const matchFoundRemainingMs =
    matchFoundPhaseEndsAtMs !== undefined
      ? Math.max(0, matchFoundPhaseEndsAtMs - matchFoundServerNowMs)
      : matchFoundStartedAt
        ? Math.max(0, matchAcceptTimeoutMs - matchFoundFallbackElapsedMs)
        : matchAcceptTimeoutMs;
  const matchFoundRemainingSeconds = Math.max(
    0,
    Math.ceil(matchFoundRemainingMs / 1_000),
  );
  const matchFoundRemainingRatio = Math.min(
    1,
    Math.max(0, matchFoundRemainingMs / matchAcceptTimeoutMs),
  );
  const matchFoundAnimationElapsedMs = matchFoundStartedAt
    ? Math.max(0, matchFoundNow - matchFoundStartedAt)
    : Math.max(0, matchAcceptTimeoutMs - matchFoundRemainingMs);
  const matchFoundSpinTurn =
    (matchFoundAnimationElapsedMs % matchFoundHexSpinDurationMs) /
    matchFoundHexSpinDurationMs;
  const matchFoundCountdownClassName = [
    "match-found-countdown",
    currentPlayerAccepted ? "accepted" : "",
    matchFoundHexAligned ? "aligned" : "",
  ]
    .filter(Boolean)
    .join(" ");
  const matchFoundCountdownStyle = {
    "--match-found-progress": matchFoundRemainingRatio.toString(),
    "--match-found-base-rotation": `${getShortestRotationDegrees(
      -360 * matchFoundSpinTurn,
    )}deg`,
    "--match-found-overlay-rotation": `${getShortestRotationDegrees(
      360 * matchFoundSpinTurn,
    )}deg`,
  } as CSSProperties;
  const matchFoundAcceptedCount =
    pendingMatch?.acceptances?.filter((acceptance) => acceptance.status === "ACCEPTED")
      .length ?? 0;
  const matchFoundOverlayStroke = useMemo(
    () => getMatchFoundOverlayStroke(accentColor),
    [accentColor],
  );

  function notifyLobbyError(message: string) {
    setLobbyError(message);
    notify({
      type: "error",
      message,
    });
  }

  function notifyGameStartError(error: unknown) {
    const fallback = t("client-game-start-error");
    const detail = getErrorMessage(error, fallback);

    notifyLobbyError(detail === fallback ? fallback : `${fallback} ${detail}`);
  }

  function rememberLobbyMemberRoles(members: Array<LobbyMember | LobbyMemberWithRoles>) {
    setLobbyMemberRoles((currentRoles) => {
      let changed = false;
      const nextRoles = { ...currentRoles };

      for (const member of members) {
        if (typeof member.publicId !== "number") {
          continue;
        }

        const roles = getMemberLobbyRoles(member);

        if (!hasLobbyRoles(roles)) {
          continue;
        }

        const currentMemberRoles = nextRoles[member.publicId];
        const mergedRoles = [
          roles[0] ?? currentMemberRoles?.[0],
          roles[1] ?? currentMemberRoles?.[1],
        ] satisfies LobbyRoleSelection;

        if (
          currentMemberRoles?.[0] === mergedRoles[0] &&
          currentMemberRoles?.[1] === mergedRoles[1]
        ) {
          continue;
        }

        nextRoles[member.publicId] = mergedRoles;
        changed = true;
      }

      return changed ? nextRoles : currentRoles;
    });
  }

  function rememberLobbyRolesFromStatuses(statuses: UserStatusSnapshot[] = []) {
    rememberLobbyMemberRoles(
      statuses
        .filter((status) => typeof status.publicId === "number")
        .map((status) => {
          const roles = getLobbyRolesFromPresenceMode(status.mode);

          return {
            publicId: status.publicId,
            primaryRole: roles[0] ? toApiLobbyRole(roles[0]) : undefined,
            secondaryRole: roles[1] ? toApiLobbyRole(roles[1]) : undefined,
          } satisfies LobbyMemberWithRoles;
        }),
    );
  }

  function getEffectiveLobbyMemberRoles(member?: LobbyMember) {
    const snapshotRoles = getMemberLobbyRoles(member);

    if (typeof member?.publicId === "number") {
      const cachedRoles = lobbyMemberRoles[member.publicId];

      if (cachedRoles) {
        return [
          snapshotRoles[0] ?? cachedRoles[0],
          snapshotRoles[1] ?? cachedRoles[1],
        ] satisfies LobbyRoleSelection;
      }
    }

    return snapshotRoles;
  }

  function getActiveLobbyWithCachedRoles() {
    if (!activeLobby?.members) {
      return activeLobby;
    }

    return {
      ...activeLobby,
      members: activeLobby.members.map((member) => {
        if (typeof member.publicId !== "number") {
          return member;
        }

        const roles = lobbyMemberRoles[member.publicId];

        if (!roles || hasLobbyRoles(getMemberLobbyRoles(member))) {
          return member;
        }

        return {
          ...member,
          primaryRole: roles[0] ? toApiLobbyRole(roles[0]) : undefined,
          secondaryRole: roles[1] ? toApiLobbyRole(roles[1]) : undefined,
        } satisfies LobbyMemberWithRoles;
      }),
    };
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
  const partyInviteOnlinePublicIdSet = useMemo(
    () =>
      new Set(
        partyInviteOnlineUsers
          .map((user) => toPublicId(user.publicId))
          .filter((publicId): publicId is number => typeof publicId === "number"),
      ),
    [partyInviteOnlineUsers],
  );
  const partyInviteableFriendPublicIdSet = useMemo(() => {
    return new Set(
      partyInviteFriends
        .filter((friend) => isInviteablePresence(friend.status))
        .map((friend) => friend.publicId)
        .filter((publicId): publicId is number => typeof publicId === "number"),
    );
  }, [partyInviteFriends]);
  const canShowInviteCandidateEmail = useCallback(
    (candidate: PartyInviteCandidate) => (
      candidate.source === "friend" ||
      (
        typeof candidate.publicId === "number" &&
        userEmailVisibilityByPublicId.get(candidate.publicId) === true
      )
    ),
    [userEmailVisibilityByPublicId],
  );
  useEffect(() => {
    if (!partyInviteOpen) {
      return;
    }

    const publicIds = new Set<number>();
    const addPublicId = (value: unknown) => {
      const publicId = toPublicId(value);

      if (
        typeof publicId === "number" &&
        publicId !== profilePublicId &&
        !friendPublicIds.has(publicId) &&
        !userEmailVisibilityByPublicId.has(publicId)
      ) {
        publicIds.add(publicId);
      }
    };

    for (const user of partyInviteSearchResults) {
      addPublicId(user.publicId);
    }

    for (const user of partyInviteOnlineUsers) {
      addPublicId(user.publicId);
    }

    if (publicIds.size === 0) {
      return;
    }

    let cancelled = false;

    async function loadUserSettingsSummaries() {
      const summaries = await Promise.all(
        [...publicIds].map(async (publicId) => {
          const result = await getUserSettingsSummary(publicId);
          const showEmailPublic =
            result.data?.showEmailPublic ?? result.data?.show_email_public ?? false;

          return [publicId, result.error ? false : showEmailPublic] as const;
        }),
      );

      if (cancelled) {
        return;
      }

      setUserEmailVisibilityByPublicId((currentVisibility) => {
        const nextVisibility = new Map(currentVisibility);

        for (const [publicId, showEmailPublic] of summaries) {
          nextVisibility.set(publicId, showEmailPublic);
        }

        return nextVisibility;
      });
    }

    void loadUserSettingsSummaries();

    return () => {
      cancelled = true;
    };
  }, [
    friendPublicIds,
    partyInviteOnlineUsers,
    partyInviteOpen,
    partyInviteSearchResults,
    profilePublicId,
    userEmailVisibilityByPublicId,
  ]);
  const filteredInviteCandidates = useMemo(() => {
    const query = partyInviteSearch.trim().toLowerCase();
    const candidatesById = new Map<number | string, PartyInviteCandidate>();

    if (!query) {
      return [];
    }

    const matchesQuery = (candidate: PartyInviteCandidate) => (
      candidate.name.toLowerCase().includes(query) ||
      candidate.tagId?.toLowerCase().includes(query) ||
      (
        canShowInviteCandidateEmail(candidate) &&
        candidate.email?.toLowerCase().includes(query)
      ) ||
      String(candidate.publicId ?? "").includes(query)
    );

    for (const friend of partyInviteFriends) {
      if (
        typeof friend.publicId !== "number" ||
        !partyInviteOnlinePublicIdSet.has(friend.publicId) ||
        !isInviteablePresence(friend.status)
      ) {
        continue;
      }

      const candidate = mapFriendToInviteCandidate(friend);

      if (matchesQuery(candidate)) {
        candidatesById.set(getInviteCandidateKey(candidate), candidate);
      }
    }

    for (const user of partyInviteSearchResults) {
      const candidate = mapUserToInviteCandidate(user);
      const key = getInviteCandidateKey(candidate);

      if (!candidatesById.has(key) && matchesQuery(candidate)) {
        candidatesById.set(key, candidate);
      }
    }

    for (const user of partyInviteOnlineUsers) {
      const candidate = mapOnlineUserToInviteCandidate(user);
      const key = getInviteCandidateKey(candidate);

      if (!candidatesById.has(key) && matchesQuery(candidate)) {
        candidatesById.set(key, candidate);
      }
    }

    return [...candidatesById.values()].filter((candidate) => {
      if (typeof candidate.publicId === "number") {
        return (
          candidate.publicId !== profilePublicId &&
          candidate.publicId !== activeLobbyCurrentMember?.publicId
        );
      }

      const currentMemberName = activeLobbyCurrentMember
        ? normalizeLobbyIdentityName(getMemberName(activeLobbyCurrentMember))
        : undefined;
      const candidateName = normalizeLobbyIdentityName(candidate.name);

      return !currentMemberName || currentMemberName !== candidateName;
    });
  }, [
    activeLobbyCurrentMember,
    canShowInviteCandidateEmail,
    partyInviteFriends,
    partyInviteOnlineUsers,
    partyInviteOnlinePublicIdSet,
    partyInviteSearch,
    partyInviteableFriendPublicIdSet,
    partyInviteSearchResults,
    profilePublicId,
  ]);
  const partyInviteCandidateTotalPages = Math.ceil(
    filteredInviteCandidates.length / PARTY_INVITE_ONLINE_LIMIT,
  );
  const partyInviteShowPagination =
    partyInviteSearch.trim().length >= 1 && partyInviteCandidateTotalPages > 1;
  const partyInviteCanPagePrevious =
    partyInviteOnlinePage > 0 && !partyInviteSearching;
  const partyInviteCanPageNext =
    partyInviteOnlinePage + 1 < partyInviteCandidateTotalPages &&
    !partyInviteSearching;
  const inviteCandidates = useMemo(
    () =>
      filteredInviteCandidates.slice(
        partyInviteOnlinePage * PARTY_INVITE_ONLINE_LIMIT,
        (partyInviteOnlinePage + 1) * PARTY_INVITE_ONLINE_LIMIT,
      ),
    [filteredInviteCandidates, partyInviteOnlinePage],
  );
  useEffect(() => {
    activeLobbyRef.current = activeLobby;
  }, [activeLobby]);

  useEffect(() => {
    rememberLobbyMemberRoles(activeLobby?.members ?? []);
  }, [activeLobby?.members]);

  useEffect(() => {
    const currentMember = getCurrentLobbyMember(
      activeLobby,
      profilePublicId,
      profileName,
    );

    if (!currentMember) {
      return;
    }

    const currentMemberRoles = getEffectiveLobbyMemberRoles(currentMember);

    if (!hasLobbyRoles(currentMemberRoles)) {
      return;
    }

    setSelectedLobbyRoles(currentMemberRoles);
  }, [activeLobby, lobbyMemberRoles, profileName, profilePublicId]);

  useEffect(() => {
    if (!activeLobby?.id) {
      setLobbyMemberRoles({});
      return;
    }

    let active = true;
    const lobbyId = activeLobby.id;

    async function refreshLobbyRoles() {
      const result = await getLobbyRoles({
        baseUrl: LIVE_API_BASE_URL,
        fallbackBaseUrls: [API_BASE_URL, MATCHMAKING_API_BASE_URL],
        path: { lobbyId },
      });

      if (!active || result.error || !result.data?.members) {
        return;
      }

      rememberLobbyMemberRoles(result.data.members);
    }

    void refreshLobbyRoles();

    return () => {
      active = false;
    };
  }, [activeLobby?.id, activeLobby?.members?.length]);

  useEffect(() => {
    if (activeLobby) {
      return;
    }

    setSelectedLobbyRoles([undefined, undefined]);
    setOpenLobbyRolePicker(undefined);
    setLobbyMemberRoles({});
  }, [activeLobby]);

  useEffect(() => {
    if (!partyInvitesLocked) {
      return;
    }

    setPartyInviteOpen(false);
    setOpenLobbyRolePicker(undefined);
  }, [partyInvitesLocked]);

  useEffect(() => {
    if (!partyInviteOpen) {
      return;
    }

    setPartyInviteOnlinePage(0);
  }, [partyInviteOpen, partyInviteSearch]);

  useEffect(() => {
    if (!partyInviteOpen) {
      return;
    }

    setPartyInviteOnlinePage((currentPage) =>
      Math.min(currentPage, Math.max(0, partyInviteCandidateTotalPages - 1)),
    );
  }, [partyInviteCandidateTotalPages, partyInviteOpen]);

  useEffect(() => {
    if (!lobbyIsFull) {
      return;
    }

    setOpenLobbyRolePicker((openSlot) => (openSlot === 1 ? undefined : openSlot));

    if (!selectedLobbyRoles[1]) {
      return;
    }

    const nextSelectedRoles = [
      selectedLobbyRoles[0],
      undefined,
    ] satisfies LobbyRoleSelection;

    setSelectedLobbyRoles(nextSelectedRoles);
    setActiveLobby((currentLobby) => {
      if (!currentLobby?.members) {
        return currentLobby;
      }

      const currentMember = getCurrentLobbyMember(
        currentLobby,
        profilePublicId,
        profileName,
      );

      return {
        ...currentLobby,
        members: currentLobby.members.map((member) => {
          if (!isSameLobbyMember(member, currentMember)) {
            return member;
          }

          return {
            ...member,
            primaryRole: nextSelectedRoles[0]
              ? toApiLobbyRole(nextSelectedRoles[0])
              : undefined,
            secondaryRole: undefined,
          } satisfies LobbyMemberWithRoles;
        }),
      };
    });

    if (typeof profilePublicId === "number") {
      setLobbyMemberRoles((currentRoles) => ({
        ...currentRoles,
        [profilePublicId]: nextSelectedRoles,
      }));
    }

    if (activeLobbyRef.current?.status === "SEARCHING") {
      setPresenceStatus("inqueue");
      publishActivePresence("IN_QUEUE", nextSelectedRoles);
    } else if (activeLobbyRef.current) {
      setPresenceStatus("inlobby");
      void publishPresence(
        "IN_LOBBY",
        getLobbyPresenceMode(selectedGameMode, nextSelectedRoles),
      );
    }

    void saveLobbyMemberRoles(nextSelectedRoles);
  }, [
    lobbyIsFull,
    profileName,
    profilePublicId,
    selectedGameMode,
    selectedLobbyRoles,
  ]);

  useEffect(() => {
    championSelectionMatchRef.current = championSelectionMatch;
  }, [championSelectionMatch]);

  function setCurrentChampionSelectionMatch(match: ApiMatchResponse | undefined) {
    championSelectionMatchRef.current = match;
    setChampionSelectionMatch(match);
  }

  useEffect(() => {
    gameInProgressRef.current = gameInProgress;
  }, [gameInProgress]);

  useEffect(() => {
    gameLaunchParametersRef.current = gameLaunchParameters;
  }, [gameLaunchParameters]);

  useEffect(() => {
    const storedSession = readStoredGameSession();

    if (
      !storedSession ||
      (typeof storedSession.playerPublicId === "number" &&
        storedSession.playerPublicId !== profilePublicId)
    ) {
      return;
    }

    const session = storedSession;
    let active = true;

    async function restoreStoredGameSession() {
      const result = await getMatch({
        baseUrl: MATCHMAKING_API_BASE_URL,
        path: { matchId: session.parameters.matchId },
      });

      if (!active) {
        return;
      }

      if (
        result.response?.status === 404 ||
        result.response?.status === 410 ||
        !result.data ||
        isFinishedMatchStatus(result.data.status)
      ) {
        clearStoredGameSession();
        setGameLaunchParameters(undefined);
        setGameInProgress(false);
        setGameClientRunning(false);
        setGameClientClosedByClient(false);
        setPresenceStatus("online");
        void publishPresence("ONLINE");
        return;
      }

      setGameLaunchParameters(session.parameters);
      setGameInProgress(true);
      setGameClientRunning(false);
      setGameClientClosedByClient(Boolean(session.closedByClient));
      setPresenceStatus("ingame");
      publishActivePresence("IN_GAME");
    }

    void restoreStoredGameSession();

    return () => {
      active = false;
    };
  }, [profilePublicId]);

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

      if (!active) {
        return;
      }

      if (
        result.response?.status === 404 ||
        result.response?.status === 410 ||
        !result.data
      ) {
        finishGameSession();
        return;
      }

      if (isFinishedMatchStatus(result.data.status)) {
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
    if (!pendingMatch || (!matchFoundStartedAt && !pendingMatch.phaseEndsAt)) {
      return;
    }

    setMatchFoundNow(Date.now());

    let animationFrameId = 0;
    const updateMatchFoundNow = () => {
      setMatchFoundNow(Date.now());
      animationFrameId = window.requestAnimationFrame(updateMatchFoundNow);
    };
    animationFrameId = window.requestAnimationFrame(updateMatchFoundNow);

    return () => {
      window.cancelAnimationFrame(animationFrameId);
    };
  }, [matchFoundStartedAt, pendingMatch]);

  useEffect(() => {
    if (!pendingMatch?.matchId || !currentPlayerAccepted) {
      setMatchFoundHexAligned(false);
      return;
    }

    setMatchFoundHexAligned(false);
    const animationFrameId = window.requestAnimationFrame(() => {
      setMatchFoundHexAligned(true);
    });

    return () => {
      window.cancelAnimationFrame(animationFrameId);
    };
  }, [currentPlayerAccepted, pendingMatch?.matchId]);

  useEffect(() => {
    const warmupMatch = championSelectionMatch;

    if (
      !isWarmupMatch(warmupMatch) ||
      !warmupMatch.matchId
    ) {
      return;
    }

    const phaseEndsAt = parseApiTimestamp(warmupMatch.phaseEndsAt);

    if (phaseEndsAt === undefined) {
      return;
    }

    const timeoutId = window.setTimeout(() => {
      void getMatch({
        baseUrl: MATCHMAKING_API_BASE_URL,
        path: { matchId: warmupMatch.matchId as string },
      }).then((result) => {
        if (!result.error) {
          applyMatch(result.data);
        }
      });
    }, Math.max(0, phaseEndsAt - Date.now()) + 100);

    return () => {
      window.clearTimeout(timeoutId);
    };
  }, [championSelectionMatch?.matchId, championSelectionMatch?.phaseEndsAt, championSelectionMatch?.status]);

  useEffect(() => {
    const serverNow = parseApiTimestamp(pendingMatch?.serverNow);

    if (serverNow === undefined) {
      return;
    }

    setMatchFoundServerClockOffsetMs(serverNow - Date.now());
  }, [pendingMatch?.matchId, pendingMatch?.serverNow]);

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
        setCurrentChampionSelectionMatch(hydrateMatch(normalizeMatchResponse(result.data)));
        return;
      }

      const fallbackResult = await markChampionsReadyDuplicate({
        baseUrl: MATCHMAKING_API_BASE_URL,
        path: { matchId },
      });

      if (!fallbackResult.error && fallbackResult.data) {
        setCurrentChampionSelectionMatch(hydrateMatch(fallbackResult.data));
      }
    });
  }, [championSelectionMatch, championsReadyMarkedMatchId]);

  function getLobbyInvitationUpdatedAtMs(invitation: LobbyInvitation) {
    const timestamp = Date.parse(invitation.updatedAt ?? "");

    return Number.isFinite(timestamp) ? timestamp : undefined;
  }

  function shouldHideDeclinedLobbyInvitation(invitation: LobbyInvitation) {
    const normalizedInvitation = normalizeLobbyInvitation(invitation);

    if (!normalizedInvitation.lobbyId) {
      return false;
    }

    const declinedAt = declinedLobbyInvitationTimestampsRef.current.get(
      normalizedInvitation.lobbyId,
    );

    if (declinedAt === undefined) {
      return false;
    }

    const invitationUpdatedAt = getLobbyInvitationUpdatedAtMs(normalizedInvitation);

    return invitationUpdatedAt === undefined || invitationUpdatedAt <= declinedAt;
  }

  function rememberDeclinedLobbyInvitation(invitation: LobbyInvitation) {
    const normalizedInvitation = normalizeLobbyInvitation(invitation);

    if (!normalizedInvitation.lobbyId) {
      return;
    }

    declinedLobbyInvitationTimestampsRef.current.set(
      normalizedInvitation.lobbyId,
      getLobbyInvitationUpdatedAtMs(normalizedInvitation) ?? Date.now(),
    );
  }

  function forgetDeclinedLobbyInvitation(lobbyId: string | undefined) {
    if (!lobbyId) {
      return;
    }

    declinedLobbyInvitationTimestampsRef.current.delete(lobbyId);
  }

  function applyLobbyInvitations(nextInvitations: LobbyInvitation[]) {
    const visibleInvitations = nextInvitations.filter((invitation) => {
      return !shouldHideDeclinedLobbyInvitation(invitation);
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
      return !shouldHideDeclinedLobbyInvitation(invitation);
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

  async function listLobbyInvitationsWithFallback() {
    for (const baseUrl of [
      LIVE_API_BASE_URL,
      API_BASE_URL,
      MATCHMAKING_API_BASE_URL,
    ]) {
      const result = await listLobbyInvitations({
        baseUrl,
      }).catch(() => undefined);

      if (result && !result.error) {
        return result.data ?? [];
      }
    }

    return undefined;
  }

  function hydrateMatch(match: ApiMatchResponse) {
    const knownPlayers = new Map<number, MatchPlayerResponse>();

    function rememberPlayer(player?: MatchPlayerResponse | LobbyMember) {
      if (typeof player?.publicId !== "number") {
        return;
      }

      const currentPlayer = knownPlayers.get(player.publicId);
      knownPlayers.set(player.publicId, mergeKnownMatchPlayer(player, currentPlayer));
    }

    for (const lobby of championSelectionMatchRef.current?.lobbies ?? []) {
      for (const player of lobby.players ?? []) {
        rememberPlayer(player);
      }
    }

    for (const lobby of match.lobbies ?? []) {
      for (const player of lobby.players ?? []) {
        rememberPlayer(player);
      }
    }

    for (const member of activeLobbyRef.current?.members ?? []) {
      rememberPlayer(member);
    }

    if (typeof currentMatchPlayerProfile.publicId === "number") {
      rememberPlayer({
        avatarUrl: currentMatchPlayerProfile.avatarUrl,
        displayName: currentMatchPlayerProfile.displayName,
        publicId: currentMatchPlayerProfile.publicId,
      });
    }

    return enrichMatchPlayers(
      {
        ...match,
        gameServer:
          match.gameServer ??
          championSelectionMatchRef.current?.gameServer ??
          pendingMatch?.gameServer,
      },
      knownPlayers,
    );
  }

  async function restartMatchSearchForLobby(lobby: LobbySnapshot) {
    if (!lobby.id || requeueingLobbyIdsRef.current.has(lobby.id)) {
      return;
    }

    requeueingLobbyIdsRef.current.add(lobby.id);
    const lobbyWithCachedRoles =
      lobby.id === activeLobbyRef.current?.id
        ? getActiveLobbyWithCachedRoles() ?? lobby
        : lobby;

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
          players: mapLobbyToMatchPlayers(
            lobbyWithCachedRoles,
            currentMatchPlayerProfile,
            publicUsersByPublicId,
          ),
        },
      });

      if (!result.error) {
        applyMatch(result.data?.match, { keepSearchingOnCancel: true });
      }
    } finally {
      requeueingLobbyIdsRef.current.delete(lobby.id);
    }
  }

  function handleChampionSelectionPlayerLeft(
    event: ChampionSelectionPlayerLeftEvent,
  ) {
    const currentMatchId =
      championSelectionMatchRef.current?.matchId ?? pendingMatch?.matchId;

    if (!currentMatchId || event.matchId !== currentMatchId) {
      return;
    }

    void deleteChampionSelectionChatRooms(
      championSelectionMatchRef.current ?? pendingMatch,
    );
    setPendingMatch(undefined);
    setMatchFoundStartedAt(undefined);
    setMatchAutoDeclinedId(undefined);
    setCurrentChampionSelectionMatch(undefined);
    setChampionsReadyMarkedMatchId(undefined);
    setLobbySearchStartedAt(undefined);
    setLobbySearchAbortedLobbyId(undefined);
    setActiveLobby((currentLobby) =>
      currentLobby
        ? {
            ...currentLobby,
            status: "OPEN",
          }
        : currentLobby,
    );
    setPresenceStatus("inlobby");
    void publishPresence(
      "IN_LOBBY",
      getLobbyPresenceMode(selectedGameMode, selectedLobbyRoles),
    );
  }

  function applyMatch(
    match: ApiMatchResponse | undefined,
    options: { keepSearchingOnCancel?: boolean } = {},
  ) {
    if (!match) {
      return;
    }

    let hydratedMatch: ApiMatchResponse = hydrateMatch(match);

    if (
      shouldTreatChampionSelectionAsWarmup(hydratedMatch) &&
      !championSelectionMatchRef.current
    ) {
      hydratedMatch = withServerEventType(
        hydratedMatch,
        "MATCH_CHAMPION_SELECTION_STARTED",
      );
    }

    if (gameInProgress) {
      if (
        isFinishedMatchStatus(hydratedMatch.status) &&
        (!gameLaunchParameters?.matchId ||
          hydratedMatch.matchId === gameLaunchParameters.matchId)
      ) {
        finishGameSession(hydratedMatch);
      }

      return;
    }

    const currentChampionSelectionMatch = championSelectionMatchRef.current;

    if (
      currentChampionSelectionMatch?.matchId &&
      hydratedMatch.matchId !== currentChampionSelectionMatch.matchId
    ) {
      return;
    }

    if (
      currentChampionSelectionMatch &&
      currentChampionSelectionMatch.matchId === hydratedMatch.matchId
    ) {
      if (
        isWarmupMatch(currentChampionSelectionMatch) &&
        isWarmupMatch(hydratedMatch) &&
        hydratedMatch.status === "CHAMPION_SELECTION" &&
        isWarmupActive(currentChampionSelectionMatch)
      ) {
        return;
      }
    }

    if (hydratedMatch.status === "CANCELLED") {
      const lobby = activeLobbyRef.current;
      const keepSearching =
        options.keepSearchingOnCancel ?? lobby?.status === "SEARCHING";

      suppressMatchLobbyInvitations(hydratedMatch);
      setLobbyInvitations((currentInvitations) =>
        currentInvitations.filter((invitation) => {
          const lobbyId = normalizeLobbyInvitation(invitation).lobbyId;

          return !lobbyId || !hydratedMatch.lobbies?.some((matchLobby) => matchLobby.lobbyId === lobbyId);
        }),
      );
      setPendingMatch(undefined);
      setMatchFoundStartedAt(undefined);
      setMatchAutoDeclinedId(undefined);
      setCurrentChampionSelectionMatch(undefined);
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

        if (
          isSameLobbyMember(
            getLobbyHost(lobby),
            getCurrentLobbyMember(lobby, profilePublicId, profileName),
          )
        ) {
          void restartMatchSearchForLobby(lobby);
        }
      } else {
        setLobbySearchStartedAt(undefined);
        setLobbySearchAbortedLobbyId(undefined);
        setActiveLobby((currentLobby) =>
          currentLobby
            ? {
                ...currentLobby,
                status: "OPEN",
              }
            : currentLobby,
        );
      }

      return;
    }

    if (isMatchReady(hydratedMatch)) {
      setPendingMatch(undefined);
      setMatchFoundStartedAt(undefined);
      setMatchAutoDeclinedId(undefined);
      setCurrentChampionSelectionMatch(hydratedMatch);

      if (isMatchGameStarted(hydratedMatch)) {
        setPresenceStatus("ingame");
        publishActivePresence("IN_GAME");
      }

      return;
    }

    if (hydratedMatch.status === "PENDING_ACCEPTANCE") {
      setMatchFoundStartedAt((currentStartedAt) => {
        if (currentStartedAt && pendingMatch?.matchId === hydratedMatch.matchId) {
          return currentStartedAt;
        }

        return Date.now();
      });
      setMatchFoundNow(Date.now());
      if (pendingMatch?.matchId !== hydratedMatch.matchId) {
        setMatchAutoDeclinedId(undefined);
      }
      setPendingMatch(hydratedMatch);
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

  async function refreshLobbyFriendProfiles(page = partyInviteOnlinePage) {
    const [result, onlineResult] = await Promise.all([
      liveBootstrap({
        baseUrl: LIVE_API_BASE_URL,
      }),
      listOnlineUsers({
        baseUrl: LIVE_API_BASE_URL,
        query: { limit: PARTY_INVITE_ONLINE_LIMIT, page },
      }),
    ]);

    if (result.error) {
      notifyLobbyError(t("friend-api-error"));
      return;
    }

    const onlineUsers = onlineResult.error ? [] : (onlineResult.data?.users ?? []);
    const onlinePublicIds = new Set(
      onlineUsers
        .map((user) => toPublicId(user.publicId))
        .filter((publicId): publicId is number => typeof publicId === "number"),
    );
    const friendStatusesByPublicId = new Map(
      (result.data?.friendStatuses?.statuses ?? [])
        .map((status) => [toPublicId(status.publicId), status] as const)
        .filter(
          (entry): entry is readonly [number, UserStatusSnapshot] =>
            typeof entry[0] === "number",
        ),
    );

    console.info("[mira:lobby-invite] /api/users/online", {
      error: onlineResult.error,
      pagination: onlineResult.data
        ? {
            limit: onlineResult.data.limit,
            page: onlineResult.data.page,
            total: onlineResult.data.total,
            totalPages: onlineResult.data.totalPages,
          }
        : undefined,
      users: onlineUsers,
    });
    console.info("[mira:lobby-invite] liveBootstrap friends/statuses", {
      friends: result.data?.friends?.friends ?? [],
      friendStatuses: result.data?.friendStatuses?.statuses ?? [],
    });

    setPartyInviteFriends(
      (result.data?.friends?.friends ?? []).map((friend) => {
        const publicId = toPublicId(friend.publicId);

        return mapFriendUserToProfile(
          friend,
          typeof publicId === "number"
            ? friendStatusesByPublicId.get(publicId)
            : undefined,
          onlinePublicIds,
        );
      }),
    );
    setPartyInviteOnlineUsers(onlineUsers);

    rememberLobbyRolesFromStatuses(result.data?.friendStatuses?.statuses ?? []);
    replaceLobbyInvitations(result.data?.lobbyInvitations ?? []);
    setLobbyError(undefined);
  }

  async function listAllPartyInviteOnlineUsers() {
    const firstResult = await listOnlineUsers({
      baseUrl: LIVE_API_BASE_URL,
      query: { limit: PARTY_INVITE_ONLINE_LIMIT, page: 0 },
    });

    if (firstResult.error) {
      return {
        error: firstResult.error,
        pageResults: [firstResult],
        users: [] as OnlineInviteUser[],
      };
    }

    const firstPageUsers = firstResult.data?.users ?? [];
    const totalPages = Math.max(0, firstResult.data?.totalPages ?? 0);
    const remainingPages = Array.from(
      { length: Math.max(0, totalPages - 1) },
      (_, index) => index + 1,
    );
    const remainingResults = await Promise.all(
      remainingPages.map((page) =>
        listOnlineUsers({
          baseUrl: LIVE_API_BASE_URL,
          query: { limit: PARTY_INVITE_ONLINE_LIMIT, page },
        }),
      ),
    );
    const usersByPublicId = new Map<number | string, OnlineInviteUser>();

    for (const user of firstPageUsers) {
      usersByPublicId.set(toPublicId(user.publicId) ?? `page-0-${usersByPublicId.size}`, user);
    }

    for (const result of remainingResults) {
      if (result.error) {
        continue;
      }

      for (const user of result.data?.users ?? []) {
        usersByPublicId.set(
          toPublicId(user.publicId) ?? `page-rest-${usersByPublicId.size}`,
          user,
        );
      }
    }

    return {
      error: remainingResults.find((result) => result.error)?.error,
      pageResults: [firstResult, ...remainingResults],
      users: [...usersByPublicId.values()],
    };
  }

  useEffect(() => {
    let active = true;

    async function refreshInvitations() {
      const nextInvitations = await listLobbyInvitationsWithFallback();

      if (!active || !nextInvitations) {
        return;
      }

      replaceLobbyInvitations(nextInvitations);
    }

    void refreshInvitations();

    const intervalId = window.setInterval(refreshInvitations, 3_000);

    return () => {
      active = false;
      window.clearInterval(intervalId);
    };
  }, [activeLobby?.id, profileName, profilePublicId]);

  useEffect(() => {
    if (!activeLobby) {
      return;
    }

    void refreshLobbyFriendProfiles();
  }, [activeLobby?.id]);

  useEffect(() => {
    if (!activeLobby?.id) {
      return;
    }

    let active = true;

    async function refreshLobbyPeerRoles() {
      const result = await liveBootstrap({
        baseUrl: LIVE_API_BASE_URL,
      });

      if (!active || result.error) {
        return;
      }

      rememberLobbyRolesFromStatuses(result.data?.friendStatuses?.statuses ?? []);
    }

    void refreshLobbyPeerRoles();

    const intervalId = window.setInterval(() => {
      void refreshLobbyPeerRoles();
    }, 1_500);

    return () => {
      active = false;
      window.clearInterval(intervalId);
    };
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

    if (query.length < 1) {
      setPartyInviteSearchResults([]);
      setPartyInviteSearching(false);
      return;
    }

    let active = true;
    setPartyInviteSearching(true);

    const timeoutId = window.setTimeout(async () => {
      const [result, onlineResult] = await Promise.all([
        query.length >= 2
          ? searchUsers({
              query: { q: query },
            })
          : Promise.resolve(undefined),
        listAllPartyInviteOnlineUsers(),
      ]);

      if (!active) {
        return;
      }

      const onlineUsers = onlineResult.error ? [] : onlineResult.users;
      const onlinePublicIds = new Set(
        onlineUsers
          .map((user) => toPublicId(user.publicId))
          .filter((publicId): publicId is number => typeof publicId === "number"),
      );

      console.info("[mira:lobby-invite] /api/users/online search refresh", {
        error: onlineResult.error,
        pageResults: onlineResult.pageResults.map((pageResult) => ({
          error: pageResult.error,
          limit: pageResult.data?.limit,
          page: pageResult.data?.page,
          total: pageResult.data?.total,
          totalPages: pageResult.data?.totalPages,
          userCount: pageResult.data?.users?.length ?? 0,
        })),
        query,
        users: onlineUsers,
      });
      setPartyInviteOnlineUsers(onlineUsers);

      if (result?.error) {
        console.info("[mira:lobby-invite] /api/users/search skipped/failed", {
          error: result.error,
          query,
        });
        if (onlineResult.error) {
          notifyLobbyError(t("friend-api-error"));
        }
        setPartyInviteSearchResults([]);
      } else if (result?.data) {
        const rawUsers = result.data?.users ?? [];
        const filteredUsers = rawUsers.filter((user) => {
          const publicId = toPublicId(user.publicId);

          return (
            typeof publicId === "number" &&
            onlinePublicIds.has(publicId)
          );
        });

        console.info("[mira:lobby-invite] /api/users/search", {
          filteredUsers,
          onlinePublicIds: [...onlinePublicIds],
          query,
          rawUsers,
        });
        setPartyInviteSearchResults(filteredUsers);
      } else {
        if (onlineResult.error) {
          notifyLobbyError(t("friend-api-error"));
        }
        setPartyInviteSearchResults([]);
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

  async function publishPresence(
    status: ApiPresenceStatus,
    mode?: string,
    options?: { force?: boolean },
  ) {
    const presenceKey = `${status}:${mode ?? ""}`;

    if (shuttingDownRef.current && status !== "OFFLINE") {
      return;
    }

    currentPresenceRef.current = { status, mode };

    if (!options?.force && remotePresenceRef.current === presenceKey) {
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

  async function publishOfflinePresence() {
    setPresenceStatus("offline");
    sendPresenceKeepalive("OFFLINE");
    await publishPresence("OFFLINE", undefined, { force: true });
  }

  function getSelectedPresenceMode(roles = selectedLobbyRoles) {
    return getLobbyPresenceMode(selectedGameMode, roles);
  }

  function publishActivePresence(
    status: Extract<ApiPresenceStatus, "IN_QUEUE" | "CHAMPION_SELECTION" | "IN_GAME">,
    roles = selectedLobbyRoles,
  ) {
    void publishPresence(status, getSelectedPresenceMode(roles));
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
    if (shuttingDownRef.current) {
      return;
    }

    if (!presenceInitializedRef.current) {
      return;
    }

    if (gameInProgressRef.current) {
      setPresenceStatus("ingame");
      publishActivePresence("IN_GAME");
      return;
    }

    if (isMatchGameStarted(championSelectionMatchRef.current)) {
      setPresenceStatus("ingame");
      publishActivePresence("IN_GAME");
      return;
    }

    if (championSelectionMatchRef.current) {
      setPresenceStatus("championselection");
      publishActivePresence("CHAMPION_SELECTION");
      return;
    }

    if (activeLobbyRef.current?.status === "SEARCHING") {
      setPresenceStatus("inqueue");
      publishActivePresence("IN_QUEUE");
      return;
    }

    if (activeLobbyRef.current) {
      setPresenceStatus("inlobby");
      void publishPresence(
        "IN_LOBBY",
        getLobbyPresenceMode(selectedGameMode, selectedLobbyRoles),
      );
      return;
    }

    const nextStatus = getIdlePresenceStatus();
    setPresenceStatus(mapUserStatusToPresence(nextStatus));
    void publishPresence(nextStatus);
  }

  function suppressMatchLobbyInvitations(match?: ApiMatchResponse) {
    for (const lobby of match?.lobbies ?? []) {
      if (lobby.lobbyId) {
        declinedLobbyInvitationTimestampsRef.current.set(lobby.lobbyId, Date.now());
      }
    }
  }

  function finishGameSession(match?: ApiMatchResponse) {
    suppressMatchLobbyInvitations(match);
    clearStoredGameSession();
    activeLobbyRef.current = undefined;
    championSelectionMatchRef.current = undefined;
    setLobbyInvitations([]);
    setPendingMatch(undefined);
    setMatchFoundStartedAt(undefined);
    setMatchAutoDeclinedId(undefined);
    setCurrentChampionSelectionMatch(undefined);
    setChampionsReadyMarkedMatchId(undefined);
    setGameInProgress(false);
    setGameClientRunning(false);
    setGameClientClosedByClient(false);
    setGameLaunchParameters(undefined);
    setGameReconnectBusy(false);
    setLobbySearchStartedAt(undefined);
    setLobbySearchAbortedLobbyId(undefined);
    setActiveLobby(undefined);
    setLobbyPageOpen(false);
    setGameSelectorOpen(false);
    setGameSelectorBackTarget("main");
    setLobbyPageBackTarget("main");
    setPresenceStatus("online");
    void publishPresence("ONLINE");
  }

  function handleRemovedFromActiveLobby() {
    activeLobbyRef.current = undefined;
    setLobbySearchStartedAt(undefined);
    setLobbySearchAbortedLobbyId(undefined);
    setLobbyMemberContextMenu(undefined);
    setLobbyMemberActionBusyId(undefined);
    setLobbyIdContextMenuOpen(false);
    setPartyInviteOpen(false);
    setActiveLobby(undefined);
    setLobbyPageOpen(false);
    setGameSelectorBackTarget("main");
    setLobbyPageBackTarget("main");
    setPresenceStatus("online");
    void publishPresence("ONLINE");
  }

  function userStatusRemovedCurrentPlayerFromActiveLobby(
    userStatus: UserStatusSnapshot,
  ) {
    if (
      typeof profilePublicId !== "number" ||
      userStatus.publicId !== profilePublicId ||
      isActivePresenceStatus(userStatus.status)
    ) {
      return false;
    }

    const lobby = activeLobbyRef.current;

    if (!lobby) {
      return false;
    }

    const statusUpdatedAt = Date.parse(userStatus.updatedAt ?? "");
    const lobbyChangedAt = Date.parse(lobby.updatedAt ?? lobby.createdAt ?? "");

    return (
      Number.isFinite(statusUpdatedAt) &&
      (!Number.isFinite(lobbyChangedAt) || statusUpdatedAt >= lobbyChangedAt)
    );
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
        currentPresenceRef.current = {
          status: currentStatus.data.status,
          mode: currentStatus.data.mode,
        };
      }

      const storedSession = readStoredGameSession();

      if (
        storedSession &&
        (typeof storedSession.playerPublicId !== "number" ||
          storedSession.playerPublicId === profilePublicId)
      ) {
        const matchStatus = await getMatch({
          baseUrl: MATCHMAKING_API_BASE_URL,
          path: { matchId: storedSession.parameters.matchId },
        });

        if (!active) {
          return;
        }

        if (
          matchStatus.response?.status === 404 ||
          matchStatus.response?.status === 410 ||
          !matchStatus.data ||
          isFinishedMatchStatus(matchStatus.data.status)
        ) {
          clearStoredGameSession();
        } else {
          presenceInitializedRef.current = true;
          setPresenceStatus("ingame");
          publishActivePresence("IN_GAME");
          return;
        }
      }

      if (isActivePresenceStatus(currentStatus.data?.status)) {
        presenceInitializedRef.current = true;
        return;
      }

      lastActivityRef.current = Date.now();
      presenceInitializedRef.current = true;
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
  }, [
    activeLobby?.id,
    activeLobby?.status,
    championSelectionMatch?.matchId,
    gameInProgress,
    selectedGameMode,
    selectedLobbyRoles,
  ]);

  useEffect(() => {
    function markActivity() {
      if (shuttingDownRef.current) {
        return;
      }

      lastActivityRef.current = Date.now();

      if (!document.hidden) {
        hiddenSinceRef.current = undefined;
      }

      if (
        !gameInProgressRef.current &&
        !activeLobbyRef.current &&
        !championSelectionMatchRef.current
      ) {
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
    function persistUnloadState() {
      const championSelectionMatch = championSelectionMatchRef.current;
      if (championSelectionMatch?.matchId && !gameInProgressRef.current) {
        sendChampionSelectionLeaveKeepalive(
          championSelectionMatch.matchId,
          "DISCONNECTED",
        );
      }

      const gameLaunchParameters = gameLaunchParametersRef.current;
      if (gameInProgressRef.current && gameLaunchParameters) {
        writeStoredGameSession({
          closedByClient: true,
          parameters: gameLaunchParameters,
          playerPublicId: profilePublicId,
        });

        if (isTauri()) {
          void invoke("stop_game_client").catch(() => undefined);
        }
      }

      sendPresenceKeepalive("OFFLINE");
    }

    window.addEventListener("pagehide", persistUnloadState);
    window.addEventListener("beforeunload", persistUnloadState);

    return () => {
      window.removeEventListener("pagehide", persistUnloadState);
      window.removeEventListener("beforeunload", persistUnloadState);
      sendPresenceKeepalive("OFFLINE");
    };
  }, [profilePublicId]);

  async function leaveCurrentLobby() {
    const lobby = activeLobbyRef.current;

    if (!lobby?.id) {
      return;
    }

    const lobbyChatRooms = await getCurrentUserGroupChatRooms(["LOBBY"], [lobby.id]);
    const lastMemberLobbyChatRoomIds = lobbyChatRooms
      .filter(currentUserIsOnlyChatRoomParticipant)
      .map(getClientChatRoomId)
      .filter((roomId): roomId is string => Boolean(roomId));

    if (lastMemberLobbyChatRoomIds.length > 0 || currentUserIsLastLobbyMember(lobby)) {
      await deleteChatRoomsById(
        lastMemberLobbyChatRoomIds.length > 0
          ? lastMemberLobbyChatRoomIds
          : lobbyChatRooms
              .map(getClientChatRoomId)
              .filter((roomId): roomId is string => Boolean(roomId)),
      );
    }

    const result = await leaveLobby({
      baseUrl: LIVE_API_BASE_URL,
      path: { lobbyId: lobby.id },
    });

    const remainingMembers = result.data?.members ?? [];

    if (remainingMembers.length === 0) {
      await deleteChatRoomsById(
        lobbyChatRooms
          .map(getClientChatRoomId)
          .filter((roomId): roomId is string => Boolean(roomId)),
      );
    }

    activeLobbyRef.current = undefined;
    setActiveLobby(undefined);
    setLobbyPageOpen(false);
    setGameSelectorOpen(false);
    setGameSelectorBackTarget("main");
    setLobbyPageBackTarget("main");
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

      rememberLobbyRolesFromStatuses(result.data?.friendStatuses?.statuses ?? []);

      const lobby = result.data?.openFriendLobbies?.find((openLobby) => {
        return openLobby.id === activeLobbyId;
      });

      if (!lobby) {
        return;
      }

      if (!getCurrentLobbyMember(lobby, profilePublicId, profileName)) {
        handleRemovedFromActiveLobby();
        return;
      }

      setActiveLobby(lobby);
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
          const lobbyRolesSnapshot = findLobbyRolesSnapshot(_event);
          const userStatusSnapshot = findUserStatusSnapshot(_event);
          const desktopSessionConflictEvent = findDesktopSessionConflictEvent(_event);
          const match = findMatchResponse(_event);
          const championSelectionPlayerLeftEvent =
            findChampionSelectionPlayerLeftEvent(_event);

          if (
            desktopSessionConflictEvent &&
            (typeof desktopSessionConflictEvent.publicId !== "number" ||
              typeof profilePublicId !== "number" ||
              desktopSessionConflictEvent.publicId === profilePublicId)
          ) {
            const eventKey = getDesktopSessionConflictKey(desktopSessionConflictEvent);

            if (!seenDesktopSessionConflictIdsRef.current.has(eventKey)) {
              seenDesktopSessionConflictIdsRef.current.add(eventKey);
              notify({
                type: "warning",
                title: t("auth-login-attempt-conflict-title"),
                message: t("auth-login-attempt-conflict-message"),
              });
            }
          }

          if (championSelectionPlayerLeftEvent) {
            handleChampionSelectionPlayerLeft(championSelectionPlayerLeftEvent);
            continue;
          }

          if (match) {
            applyMatch(match, { keepSearchingOnCancel: false });
          }

          if (invitation) {
            addInvitation(invitation);

            if (invitation.lobby?.id === activeLobbyRef.current?.id) {
              setActiveLobby(invitation.lobby);
            }
          }

          if (
            lobbyRolesSnapshot?.lobbyId &&
            lobbyRolesSnapshot.lobbyId === activeLobbyRef.current?.id
          ) {
            rememberLobbyMemberRoles(lobbyRolesSnapshot.members ?? []);
          }

          if (userStatusSnapshot) {
            rememberLobbyRolesFromStatuses([userStatusSnapshot]);

            if (userStatusRemovedCurrentPlayerFromActiveLobby(userStatusSnapshot)) {
              handleRemovedFromActiveLobby();
              continue;
            }
          }

          if (lobbySnapshot && lobbySnapshot.id === activeLobbyRef.current?.id) {
            const stillInLobby = Boolean(
              getCurrentLobbyMember(lobbySnapshot, profilePublicId, profileName),
            );

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
  }, [activeLobby?.id, notify, profilePublicId, t]);

  function handleTopButtonClick() {
    if (championTabFocused) {
      setChampionTabBackSignal((signal) => signal + 1);
      return;
    }

    if (userPageOpen) {
      setUserPageOpen(false);
      return;
    }

    if (lobbyPageOpen) {
      if (lobbyPageBackTarget === "gameSelector") {
        setLobbyPageOpen(false);
        setGameSelectorBackTarget("main");
        setGameSelectorOpen(true);
        return;
      }

      setLobbyPageOpen(false);
      return;
    }

    if (gameSelectorBackTarget === "lobby" && activeLobby) {
      setGameSelectorOpen(false);
      setLobbyPageBackTarget("main");
      setLobbyPageOpen(true);
      return;
    }

    setGameSelectorOpen(false);
    setGameSelectorBackTarget("main");
  }

  function handlePlayButtonClick() {
    setUserPageOpen(false);

    if (activeLobby) {
      setGameSelectorOpen(false);
      setLobbyPageBackTarget("main");
      setLobbyPageOpen(true);
      return;
    }

    setGameSelectorBackTarget("main");
    setGameSelectorOpen(true);
  }

  function handleChangeModeClick() {
    setUserPageOpen(false);
    setLobbyPageOpen(false);
    setGameSelectorBackTarget("lobby");
    setGameSelectorOpen(true);
  }

  function handleGameModePrimaryAction() {
    if (activeLobby) {
      setGameSelectorOpen(false);
      setLobbyPageBackTarget("gameSelector");
      setLobbyPageOpen(true);
      return;
    }

    void handleCreateLobby();
  }

  function clearTopActionDragTimer() {
    if (topActionDragTimerRef.current !== undefined) {
      window.clearTimeout(topActionDragTimerRef.current);
      topActionDragTimerRef.current = undefined;
    }
  }

  function handleTopActionPointerDown(event: PointerEvent<HTMLElement>) {
    if (event.button !== 0) {
      return;
    }

    topActionDraggingRef.current = false;
    clearTopActionDragTimer();
    event.currentTarget.setPointerCapture(event.pointerId);
    topActionDragTimerRef.current = window.setTimeout(() => {
      topActionDragTimerRef.current = undefined;
      topActionDraggingRef.current = true;

      if (isTauri()) {
        void getCurrentWindow().startDragging();
      }
    }, 500);
  }

  function handleTopActionPointerEnd(event: PointerEvent<HTMLElement>) {
    clearTopActionDragTimer();
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }

  function consumeTopActionDragClick(event: MouseEvent<HTMLElement>) {
    if (!topActionDraggingRef.current) {
      return false;
    }

    topActionDraggingRef.current = false;
    event.preventDefault();
    event.stopPropagation();
    return true;
  }

  function mergeUserPageProfileWithPublicUser(
    userPageProfile: UserPageProfile,
    publicUser: FriendUserResponse,
  ): UserPageProfile {
    return {
      avatarUrl:
        getPublicAvatarUrl(publicUser) ??
        userPageProfile.avatarUrl,
      level: getFriendUserLevel(publicUser) ?? userPageProfile.level,
      name: getFriendUserName(publicUser) || userPageProfile.name,
      publicId: publicUser.publicId ?? userPageProfile.publicId,
      tagId: getFriendUserTagId(publicUser) ?? userPageProfile.tagId,
    };
  }

  async function loadUserPagePublicProfile(publicId: number) {
    const result = await usersByPublicIds({
      baseUrl: API_BASE_URL,
      query: { publicIds: [publicId] },
    });

    if (result.error) {
      return;
    }

    const publicUser = result.data?.users?.find(
      (user) => user.publicId === publicId,
    );

    if (!publicUser) {
      return;
    }

    setPublicUsersByPublicId((currentUsers) => {
      const nextUsers = new Map(currentUsers);
      nextUsers.set(publicId, publicUser);
      return nextUsers;
    });
    setViewedUserPageProfile((currentProfile) =>
      currentProfile?.publicId === publicId
        ? mergeUserPageProfileWithPublicUser(currentProfile, publicUser)
        : currentProfile,
    );
  }

  function handleProfileOpen(profile?: Partial<UserPageProfile>) {
    setLobbyIdContextMenuOpen(false);
    setLobbyMemberContextMenu(undefined);
    setPartyInviteOpen(false);
    const nextProfile = profile?.name
      ? {
          avatarUrl: profile.avatarUrl,
          level: profile.level ?? 1,
          name: profile.name,
          publicId: profile.publicId,
          tagId: profile.tagId,
        } satisfies UserPageProfile
      : undefined;
    const cachedPublicUser =
      typeof nextProfile?.publicId === "number"
        ? publicUsersByPublicId.get(nextProfile.publicId)
        : undefined;

    setViewedUserPageProfile(
      nextProfile && cachedPublicUser
        ? mergeUserPageProfileWithPublicUser(nextProfile, cachedPublicUser)
        : nextProfile,
    );
    setActiveUserPageCategory("overview");
    setChampionTabFocused(false);
    setUserPageOpen(true);

    if (typeof nextProfile?.publicId === "number") {
      void loadUserPagePublicProfile(nextProfile.publicId);
    }
  }

  async function leaveActiveChampionSelection(status: ChampionSelectionLeaveStatus) {
    const match = championSelectionMatchRef.current;
    const matchId = match?.matchId;

    if (!matchId) {
      return;
    }

    await deleteChampionSelectionChatRooms(match);

    const liveLeaveResult = await notifyChampionSelectionLeft({
      baseUrl: LIVE_API_BASE_URL,
      body: { status },
      path: { matchId },
    }).catch(() => undefined);
    const leaveResult =
      liveLeaveResult && !liveLeaveResult.error
        ? liveLeaveResult
        : await notifyChampionSelectionLeft({
            baseUrl: MATCHMAKING_API_BASE_URL,
            body: { status },
            path: { matchId },
          }).catch(() => undefined);

    if (leaveResult && !leaveResult.error && leaveResult.data) {
      applyMatch(normalizeMatchResponse(leaveResult.data), {
        keepSearchingOnCancel: false,
      });

      return;
    }

    sendCancelChampionPhaseKeepalive(matchId);

    const result = await cancelChampionPhase({
      baseUrl: LIVE_API_BASE_URL,
      path: { matchId },
    }).catch(() => undefined);

    if ((!leaveResult || leaveResult.error) && (!result || result.error)) {
      await cancelChampionPhase({
        baseUrl: MATCHMAKING_API_BASE_URL,
        path: { matchId },
      }).catch(() => undefined);
      await cancelChampionPhaseDuplicate({
        baseUrl: MATCHMAKING_API_BASE_URL,
        path: { matchId },
      }).catch(() => undefined);
    }
  }

  async function stopRunningGameClientForShutdown() {
    const parameters = gameLaunchParametersRef.current;

    if (!gameInProgressRef.current || !parameters) {
      return;
    }

    writeStoredGameSession({
      closedByClient: true,
      parameters,
      playerPublicId: profilePublicId,
    });
    setGameClientClosedByClient(true);
    setGameClientRunning(false);

    if (isTauri()) {
      await invoke("stop_game_client").catch((caughtError) => {
        console.error(caughtError);
      });
    }
  }

  async function prepareClientShutdown(options: {
    championSelectionLeaveStatus: ChampionSelectionLeaveStatus;
    leaveLobby: boolean;
  }) {
    if (championSelectionMatchRef.current?.matchId) {
      await leaveActiveChampionSelection(options.championSelectionLeaveStatus);
    }

    await stopRunningGameClientForShutdown();

    if (options.leaveLobby) {
      await leaveCurrentLobby();
    }
  }

  async function handleClientLogout() {
    shuttingDownRef.current = true;
    await prepareClientShutdown({
      championSelectionLeaveStatus: "LEAVE",
      leaveLobby: true,
    });
    await publishOfflinePresence();
    await onLogout();
  }

  async function handleClientQuit() {
    shuttingDownRef.current = true;
    await prepareClientShutdown({
      championSelectionLeaveStatus: "QUIT",
      leaveLobby: !championSelectionMatchRef.current?.matchId,
    });
    await publishOfflinePresence();
    await onQuit();
  }

  async function handleCreateLobby() {
    if (selectedGameMode !== "ranked") {
      return;
    }

    setLobbyError(undefined);
    setGameInProgress(false);
    setGameLaunchParameters(undefined);
    setGameClientClosedByClient(false);
    clearStoredGameSession();

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
    setLobbyPageBackTarget("gameSelector");
    setLobbyPageOpen(true);
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
      setPresenceStatus("inlobby");
      void publishPresence(
        "IN_LOBBY",
        getLobbyPresenceMode(selectedGameMode, selectedLobbyRoles),
      );
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

    if (!allLobbyMembersHaveRoles) {
      notifyLobbyError(t("lobby-roles-required"));
      return;
    }

    const roleLimitError = getLobbyRoleLimitError(
      getActiveLobbyWithCachedRoles() ?? activeLobby,
    );

    if (roleLimitError) {
      notifyLobbyError(t(roleLimitError));
      return;
    }

    const wasLocallyAborted = activeLobby.id === lobbySearchAbortedLobbyId;
    setLobbySearchAbortedLobbyId(undefined);
    const activeLobbyWithCachedRoles = getActiveLobbyWithCachedRoles() ?? activeLobby;

    if (!wasLocallyAborted && activeLobby.status === "SEARCHING") {
      const startedAt = activeLobby.updatedAt ? Date.parse(activeLobby.updatedAt) : Date.now();

      setLobbySearchStartedAt(Number.isFinite(startedAt) ? startedAt : Date.now());
      setLobbySearchNow(Date.now());
      setPresenceStatus("inqueue");
      publishActivePresence("IN_QUEUE");

      const result = await startSearch({
        baseUrl: MATCHMAKING_API_BASE_URL,
        body: {
          lobbyId: activeLobby.id,
          mode: "RANKED",
          players: mapLobbyToMatchPlayers(
            activeLobbyWithCachedRoles,
            currentMatchPlayerProfile,
            publicUsersByPublicId,
          ),
        },
      });

      if (!result.error) {
        applyMatch(result.data?.match);
      }

      return;
    }

    setPresenceStatus("inqueue");
    publishActivePresence("IN_QUEUE");

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
          players: mapLobbyToMatchPlayers(
            activeLobbyWithCachedRoles,
            currentMatchPlayerProfile,
            publicUsersByPublicId,
          ),
        },
      }),
    ]);

    if (result.error && matchSearchResult.error) {
      setPresenceStatus("inlobby");
      void publishPresence(
        "IN_LOBBY",
        getLobbyPresenceMode(selectedGameMode, selectedLobbyRoles),
      );
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

  function getUserPageNameTag(profile: UserPageProfile) {
    const tagId = normalizeTagId(profile.tagId);

    return tagId ? `${profile.name}#${tagId}` : profile.name;
  }

  async function handleCopyUserPageNameTag() {
    await navigator.clipboard.writeText(getUserPageNameTag(activeUserPageProfile));
  }

  async function handleCopyUserPagePublicId() {
    if (typeof activeUserPageProfile.publicId !== "number") {
      return;
    }

    await navigator.clipboard.writeText(`#${activeUserPageProfile.publicId}`);
  }

  async function handleCopyLobbyRosterMember(member: LobbyMember) {
    await navigator.clipboard.writeText(getLobbyMemberNameTag(member));
  }

  async function handleMatchDecision(decision: MatchDecision) {
    if (!pendingMatch?.matchId) {
      return;
    }

    const matchId = pendingMatch.matchId;
    setMatchDecisionBusy(decision);

    const result = await (decision === "accept" ? accept : decline)({
      baseUrl: LIVE_API_BASE_URL,
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
        void cancelChampionPhaseDuplicate({
          baseUrl: MATCHMAKING_API_BASE_URL,
          path: { matchId },
        }).catch(() => undefined);
      }

      setPendingMatch(undefined);
      setMatchFoundStartedAt(undefined);
      setMatchAutoDeclinedId(undefined);
      setLobbySearchStartedAt(undefined);
      setLobbySearchAbortedLobbyId(undefined);
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
      !pendingMatch.matchId ||
      !matchFoundStartedAt ||
      currentPlayerAccepted ||
      matchDecisionBusy ||
      matchAutoDeclinedId === pendingMatch.matchId ||
      matchFoundRemainingMs > 0
    ) {
      return;
    }

    setMatchAutoDeclinedId(pendingMatch.matchId);
    const matchId = pendingMatch.matchId;
    setPendingMatch(undefined);
    setMatchFoundStartedAt(undefined);
    setMatchDecisionBusy(undefined);
    setLobbySearchStartedAt(undefined);
    setLobbySearchAbortedLobbyId(undefined);
    setActiveLobby((currentLobby) =>
      currentLobby
        ? {
            ...currentLobby,
            status: "OPEN",
          }
        : currentLobby,
    );
    void (async () => {
      const declineResult = await decline({
        baseUrl: LIVE_API_BASE_URL,
        path: { matchId },
      }).catch(() => undefined);

      if (declineResult && !declineResult.error) {
        return;
      }

      if (typeof profilePublicId === "number") {
        const decideResult = await decide({
          baseUrl: MATCHMAKING_API_BASE_URL,
          body: {
            playerPublicId: profilePublicId,
            decision: "DECLINED",
          },
          path: { matchId },
        }).catch(() => undefined);

        if (decideResult && !decideResult.error) {
          return;
        }
      }

      if (activeLobbyRef.current?.id) {
        await abortSearch({
          baseUrl: MATCHMAKING_API_BASE_URL,
          path: { lobbyId: activeLobbyRef.current.id },
        }).catch(() => undefined);
      }
    })();
  }, [
    currentPlayerAccepted,
    matchAutoDeclinedId,
    matchDecisionBusy,
    matchFoundRemainingMs,
    matchFoundStartedAt,
    pendingMatch,
  ]);

  async function sendLobbyInvite(lobbyId: string, targetPublicId: number) {
    for (const baseUrl of [
      LIVE_API_BASE_URL,
      API_BASE_URL,
      MATCHMAKING_API_BASE_URL,
    ]) {
      const result = await invite({
        baseUrl,
        body: { targetPublicId },
        path: { lobbyId },
      }).catch(() => undefined);

      if (!result || result.error) {
        continue;
      }

      const invitation = result.data
        ? normalizeLobbyInvitation(result.data)
        : undefined;
      const invitationLobby = invitation?.lobby;

      if (invitationLobby?.id === activeLobbyRef.current?.id) {
        setActiveLobby(invitationLobby);
      }

      setLobbyError(undefined);
      return true;
    }

    console.error("Lobby invite failed on all known services", {
      lobbyId,
      targetPublicId,
    });
    return false;
  }

  async function handleLobbyFriendDrop(friend: FriendProfile) {
    if (
      !activeLobby?.id ||
      partyInvitesLocked ||
      typeof friend.publicId !== "number" ||
      !isInviteablePresence(friend.status)
    ) {
      return;
    }

    const invited = await sendLobbyInvite(activeLobby.id, friend.publicId);

    if (!invited) {
      notifyLobbyError(t("lobby-invite-error"));
    }
  }

  function openPartyInviteDialog() {
    if (!activeLobby?.id || partyInvitesLocked) {
      return;
    }

    setPartyInviteOpen(true);
    setPartyInviteSearch("");
    setPartyInviteSearchResults([]);
    setPartyInviteOnlinePage(0);
    setLobbyError(undefined);
  }

  async function saveLobbyMemberRoles(
    roles: [LobbyRoleId | undefined, LobbyRoleId | undefined],
  ) {
    if (!activeLobby?.id || activeLobby.status !== "OPEN") {
      return false;
    }

    const [primaryRole, secondaryRole] = roles;

    if (primaryRole && secondaryRole && primaryRole === secondaryRole) {
      notifyLobbyError(t("lobby-role-update-error"));
      return false;
    }

    const result = await updateLobbyMemberRoles({
      baseUrl: LIVE_API_BASE_URL,
      body: {
        ...(primaryRole ? { primaryRole: toApiLobbyRole(primaryRole) } : {}),
        secondaryRole: secondaryRole ? toApiLobbyRole(secondaryRole) : null,
      },
      fallbackBaseUrls: [API_BASE_URL, MATCHMAKING_API_BASE_URL],
      path: { lobbyId: activeLobby.id },
    });

    if (result.response?.status === 404) {
      return true;
    }

    if (result.error || !result.data) {
      console.error("Lobby role update failed", {
        error: result.error,
        status: result.response?.status,
      });
      notifyLobbyError(
        result.response?.status
          ? `${t("lobby-role-update-error")} (${result.response.status})`
          : t("lobby-role-update-error"),
      );
      return false;
    }

    rememberLobbyMemberRoles(result.data.members ?? []);
    return true;
  }

  async function handleLobbyRoleSelect(slot: 0 | 1, roleId: LobbyRoleId) {
    if (lobbyIsFull && slot === 1) {
      return;
    }

    const duplicateSlot = slot === 0 ? 1 : 0;

    if (selectedLobbyRoles[duplicateSlot] === roleId) {
      return;
    }

    const nextSelectedRoles = normalizeLobbyRoleSelection([
      slot === 0 ? roleId : selectedLobbyRoles[0],
      lobbyIsFull ? undefined : slot === 1 ? roleId : selectedLobbyRoles[1],
    ]);

    const previousSelectedRoles = selectedLobbyRoles;
    const previousCachedRoles =
      typeof profilePublicId === "number" ? lobbyMemberRoles[profilePublicId] : undefined;
    setSelectedLobbyRoles(nextSelectedRoles);
    setActiveLobby((currentLobby) => {
      if (!currentLobby?.members) {
        return currentLobby;
      }

      const currentMember = getCurrentLobbyMember(
        currentLobby,
        profilePublicId,
        profileName,
      );

      return {
        ...currentLobby,
        members: currentLobby.members.map((member) => {
          if (!isSameLobbyMember(member, currentMember)) {
            return member;
          }

          return {
            ...member,
            primaryRole: nextSelectedRoles[0]
              ? toApiLobbyRole(nextSelectedRoles[0])
              : undefined,
            secondaryRole: nextSelectedRoles[1]
              ? toApiLobbyRole(nextSelectedRoles[1])
              : undefined,
          } satisfies LobbyMemberWithRoles;
        }),
      };
    });
    if (typeof profilePublicId === "number") {
      setLobbyMemberRoles((currentRoles) => ({
        ...currentRoles,
        [profilePublicId]: nextSelectedRoles,
      }));
    }
    setOpenLobbyRolePicker(undefined);

    if (activeLobbyRef.current) {
      setPresenceStatus("inlobby");
      void publishPresence(
        "IN_LOBBY",
        getLobbyPresenceMode(selectedGameMode, nextSelectedRoles),
      );
    }

    const updated = await saveLobbyMemberRoles(nextSelectedRoles);

    if (!updated) {
      setSelectedLobbyRoles(previousSelectedRoles);
      setActiveLobby(activeLobby);
      if (typeof profilePublicId === "number") {
        setLobbyMemberRoles((currentRoles) => {
          const nextRoles = { ...currentRoles };

          if (previousCachedRoles) {
            nextRoles[profilePublicId] = previousCachedRoles;
          } else {
            delete nextRoles[profilePublicId];
          }

          return nextRoles;
        });
      }
    }
  }

  async function handleInviteCandidate(candidate: PartyInviteCandidate) {
    if (
      !activeLobby?.id ||
      partyInvitesLocked ||
      typeof candidate.publicId !== "number" ||
      (!partyInviteOnlinePublicIdSet.has(candidate.publicId) &&
        (candidate.source !== "friend" ||
          !partyInviteableFriendPublicIdSet.has(candidate.publicId)))
    ) {
      return;
    }

    setPartyInviteBusyId(candidate.publicId);

    const invited = await sendLobbyInvite(activeLobby.id, candidate.publicId);

    setPartyInviteBusyId(undefined);

    if (!invited) {
      notifyLobbyError(t("lobby-invite-error"));
    }
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
    const member = lobbyMemberContextMenu?.member;

    setLobbyMemberContextMenu(undefined);

    if (!member) {
      return;
    }

    if (isSameLobbyMember(member, activeLobbyCurrentMember)) {
      handleProfileOpen();
      return;
    }

    handleProfileOpen({
      avatarUrl: getLobbyMemberPublicAvatarUrl(member, publicUsersByPublicId),
      level: 0,
      name: getMemberName(member),
      publicId: member.publicId,
    });
  }

  async function handleAddLobbyMemberFriend(member: LobbyMember) {
    if (
      typeof member.publicId !== "number" ||
      isSameLobbyMember(member, activeLobbyCurrentMember)
    ) {
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
    if (
      !activeLobby?.id ||
      typeof member.publicId !== "number" ||
      isSameLobbyMember(member, activeLobbyCurrentMember)
    ) {
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
      !isSameLobbyMember(lobbyHost, activeLobbyCurrentMember) ||
      typeof member.publicId !== "number" ||
      isSameLobbyMember(member, activeLobbyCurrentMember)
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
    setLobbyPageBackTarget("main");
    setLobbyPageOpen(true);
    setGameSelectorOpen(false);
  }

  async function handleAcceptInvite(invitation: LobbyInvitation) {
    if (!invitation.lobbyId) {
      return;
    }

    forgetDeclinedLobbyInvitation(invitation.lobbyId);

    const result = await joinLobby({
      baseUrl: LIVE_API_BASE_URL,
      path: { lobbyId: invitation.lobbyId },
    });

    if (result.error || !result.data) {
      notifyLobbyError(t("lobby-join-error"));
      return;
    }

    setActiveLobby(result.data);
    setLobbyPageBackTarget("main");
    setLobbyPageOpen(true);
    setLobbyInvitations((currentInvitations) =>
      currentInvitations.filter(
        (currentInvitation) => currentInvitation.lobbyId !== invitation.lobbyId,
      ),
    );
    setGameSelectorOpen(false);
  }

  function handleDeclineInvite(invitation: LobbyInvitation) {
    rememberDeclinedLobbyInvitation(invitation);

    setLobbyInvitations((currentInvitations) =>
      currentInvitations.filter(
        (currentInvitation) => currentInvitation.lobbyId !== invitation.lobbyId,
      ),
    );
  }

  async function handleChampionSelect(champion: string) {
    const currentMatch = championSelectionMatchRef.current ?? championSelectionMatch;
    const matchId = currentMatch?.matchId;

    if (!matchId) {
      return false;
    }

    let nextMatch: ApiMatchResponse | undefined;
    const attempts: Array<{
      endpoint: string;
      status?: number;
      error?: unknown;
    }> = [];

    for (const baseUrl of [
      LIVE_API_BASE_URL,
      MATCHMAKING_API_BASE_URL,
      API_BASE_URL,
    ]) {
      try {
        const result = await selectChampion({
          baseUrl,
          body: { champion },
          path: { matchId },
        });

        if (!result.error && result.data) {
          nextMatch = normalizeMatchResponse(result.data);

          if (nextMatch) {
            break;
          }
        }

        attempts.push({
          endpoint: `${baseUrl}/api/matches/${matchId}/champion-selection`,
          error: result.error,
          status: result.response?.status,
        });
      } catch (error) {
        attempts.push({
          endpoint: `${baseUrl}/api/matches/${matchId}/champion-selection`,
          error,
        });
      }
    }

    if (!nextMatch && typeof profilePublicId === "number") {
      for (const baseUrl of [
        MATCHMAKING_API_BASE_URL,
        LIVE_API_BASE_URL,
        API_BASE_URL,
      ]) {
        try {
          const fallbackResult = await selectChampionDuplicate({
            baseUrl,
            body: {
              champion,
              playerPublicId: profilePublicId,
            },
            path: { matchId },
          });

          if (!fallbackResult.error && fallbackResult.data) {
            nextMatch = normalizeMatchResponse(fallbackResult.data);

            if (nextMatch) {
              break;
            }
          }

          attempts.push({
            endpoint: `${baseUrl}/internal/matches/${matchId}/champion-selections`,
            error: fallbackResult.error,
            status: fallbackResult.response?.status,
          });
        } catch (error) {
          attempts.push({
            endpoint: `${baseUrl}/internal/matches/${matchId}/champion-selections`,
            error,
          });
        }
      }
    }

    if (!nextMatch) {
      console.error("Champion selection failed on all known endpoints", {
        attempts,
        champion,
        matchId,
        profilePublicId,
      });
      notifyLobbyError(t("match-decision-error"));
      return false;
    }

    setCurrentChampionSelectionMatch(hydrateMatch(nextMatch));
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
        setCurrentChampionSelectionMatch(
          championSelectionMatchRef.current
            ? hydrateMatch(
                mergeMatchChampionHovers(
                  championSelectionMatchRef.current,
                  result.data.hovers,
                ),
              )
            : undefined,
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
        setCurrentChampionSelectionMatch(
          championSelectionMatchRef.current
            ? hydrateMatch(
                mergeMatchChampionHovers(
                  championSelectionMatchRef.current,
                  fallbackResult.data.hovers,
                ),
              )
            : undefined,
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
      setCurrentChampionSelectionMatch(
        championSelectionMatchRef.current
          ? hydrateMatch(
              mergeMatchChampionHovers(
                championSelectionMatchRef.current,
                result.data.hovers,
              ),
            )
          : undefined,
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
      setCurrentChampionSelectionMatch(
        championSelectionMatchRef.current
          ? hydrateMatch(
              mergeMatchChampionHovers(
                championSelectionMatchRef.current,
                fallbackResult.data.hovers,
              ),
            )
          : undefined,
      );
    }
  }

  async function handleChampionSelectionTimeout(
    timedOutPickPublicIds: number[] = [],
  ) {
    if (championSelectionTimeoutInFlightRef.current) {
      return;
    }

    championSelectionTimeoutInFlightRef.current = true;

    try {
      const timedOutMatch = championSelectionMatchRef.current;
      const matchId = timedOutMatch?.matchId;

      if (matchId) {
        await deleteChampionSelectionChatRooms(timedOutMatch);
        await new Promise((resolve) => window.setTimeout(resolve, 350));

        const latestMatch = await getMatch({
          baseUrl: MATCHMAKING_API_BASE_URL,
          path: { matchId },
        }).catch(() => undefined);

        if (latestMatch && !latestMatch.error && latestMatch.data) {
          const hydratedLatestMatch = hydrateMatch(normalizeMatchResponse(latestMatch.data));
          const latestPhaseEndsAt = parseApiTimestamp(hydratedLatestMatch.phaseEndsAt);
          const latestPhase = hydratedLatestMatch.phase?.trim().toUpperCase();
          const latestServerNow =
            parseApiTimestamp(hydratedLatestMatch.serverNow) ?? Date.now();
          const latestSelectedPublicIds = new Set(
            hydratedLatestMatch.championSelections
              ?.map((selection) => selection.playerPublicId)
              .filter((publicId): publicId is number => typeof publicId === "number") ?? [],
          );
          const timedOutPickGroupComplete =
            timedOutPickPublicIds.length > 0 &&
            timedOutPickPublicIds.every((publicId) =>
              latestSelectedPublicIds.has(publicId),
            );

          if (
            hydratedLatestMatch.status === "READY" ||
            latestPhase === "READY" ||
            timedOutPickGroupComplete ||
            (timedOutPickPublicIds.length === 0 &&
              latestPhase === "PICK" &&
              latestPhaseEndsAt !== undefined &&
              latestPhaseEndsAt > latestServerNow) ||
            latestPhase === "WARMUP"
          ) {
            setCurrentChampionSelectionMatch(hydratedLatestMatch);
            return;
          }
        }

        const liveLeaveResult = await notifyChampionSelectionLeft({
          baseUrl: LIVE_API_BASE_URL,
          body: { status: "LEAVE" },
          path: { matchId },
        }).catch(() => undefined);
        const fallbackLeaveResult =
          liveLeaveResult && !liveLeaveResult.error
            ? undefined
            : await notifyChampionSelectionLeft({
                baseUrl: MATCHMAKING_API_BASE_URL,
                body: { status: "LEAVE" },
                path: { matchId },
              }).catch(() => undefined);
        const cancelledMatch =
          liveLeaveResult && !liveLeaveResult.error && liveLeaveResult.data
            ? normalizeMatchResponse(liveLeaveResult.data)
            : fallbackLeaveResult &&
                !fallbackLeaveResult.error &&
                fallbackLeaveResult.data
              ? normalizeMatchResponse(fallbackLeaveResult.data)
              : undefined;

        if (cancelledMatch) {
          applyMatch(cancelledMatch, { keepSearchingOnCancel: false });
        } else {
          await cancelChampionPhase({
            baseUrl: LIVE_API_BASE_URL,
            path: { matchId },
          }).catch(() => undefined);
          await cancelChampionPhase({
            baseUrl: MATCHMAKING_API_BASE_URL,
            path: { matchId },
          }).catch(() => undefined);
        }
      }

      setCurrentChampionSelectionMatch(undefined);
      setPendingMatch(undefined);
      setMatchFoundStartedAt(undefined);
      setMatchAutoDeclinedId(undefined);
      setChampionsReadyMarkedMatchId(undefined);
      setLobbySearchStartedAt(undefined);
      setLobbySearchAbortedLobbyId(undefined);
      setActiveLobby((currentLobby) =>
        currentLobby
          ? {
              ...currentLobby,
              status: "OPEN",
            }
          : currentLobby,
      );
    } finally {
      championSelectionTimeoutInFlightRef.current = false;
    }
  }

  function createGameLaunchParameters(match: ApiMatchResponse): GameLaunchParameters {
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
      matchManifestJson: JSON.stringify(createGameMatchManifest(match, match.matchId)),
      matchId: match.matchId,
      matchmakingApiBaseUrl: MATCHMAKING_API_BASE_URL,
      playerPublicId: profilePublicId,
      serverHost,
      serverControlBaseUrl,
      port,
      screen: gameScreenMode,
      team,
    };
  }

  function hasGameServerLaunchInfo(match: ApiMatchResponse) {
    return (
      typeof getMatchPort(match) === "number" &&
      Boolean(getMatchHost(match)) &&
      Boolean(getMatchControlBaseUrl(match))
    );
  }

  async function getLaunchableMatch(match: ApiMatchResponse) {
    let latestMatch = hydrateMatch(match);

    for (let attempt = 0; attempt < 6; attempt += 1) {
      if (hasGameServerLaunchInfo(latestMatch)) {
        return latestMatch;
      }

      if (!latestMatch.matchId) {
        return latestMatch;
      }

      await new Promise((resolve) => window.setTimeout(resolve, attempt === 0 ? 250 : 1_000));

      const result = await getMatch({
        baseUrl: MATCHMAKING_API_BASE_URL,
        path: { matchId: latestMatch.matchId },
      });

      if (!result.error && result.data) {
        latestMatch = hydrateMatch(result.data);
        setCurrentChampionSelectionMatch(latestMatch);
      }
    }

    return latestMatch;
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
      screen: parameters.screen ?? gameScreenMode,
      forceRestart,
    };

    await invoke("launch_game", { request });
    setGameLaunchParameters(parameters);
    setGameClientRunning(true);
    setGameClientClosedByClient(false);
    setPresenceStatus("ingame");
    publishActivePresence("IN_GAME");
    writeStoredGameSession({
      closedByClient: false,
      parameters,
      playerPublicId: profilePublicId,
    });
  }

  async function finishGameStart() {
    await deleteChampionSelectionChatRooms(championSelectionMatchRef.current);
    setCurrentChampionSelectionMatch(undefined);
    setPendingMatch(undefined);
    setMatchFoundStartedAt(undefined);
    setMatchAutoDeclinedId(undefined);
    setChampionsReadyMarkedMatchId(undefined);
    setLobbySearchStartedAt(undefined);
    setLobbySearchAbortedLobbyId(undefined);
    setActiveLobby(undefined);
    setLobbyPageOpen(false);
    setGameSelectorOpen(false);
    setGameSelectorBackTarget("main");
    setLobbyPageBackTarget("main");
    setGameInProgress(true);
    setPresenceStatus("ingame");
    publishActivePresence("IN_GAME");
  }

  async function handleReadyPhaseComplete() {
    const match = championSelectionMatch;

    if (!match) {
      return;
    }

    try {
      const launchableMatch = await getLaunchableMatch(match);
      const launchParameters = createGameLaunchParameters(launchableMatch);

      await launchGameClient(launchParameters, true);
      await finishGameStart();
    } catch (caughtError) {
      console.error(caughtError);
      notifyGameStartError(caughtError);
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
          : createGameLaunchParameters(await getLaunchableMatch(latestMatch.data));

      await launchGameClient(launchParameters, true);
    } catch (caughtError) {
      console.error(caughtError);
      notifyGameStartError(caughtError);
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
          onPickTimeout={(activePickPublicIds) =>
            void handleChampionSelectionTimeout(activePickPublicIds)
          }
          onReadyPhaseComplete={handleReadyPhaseComplete}
        />
        <ChatDock
          autoRooms={championSelectionChatRooms}
          chatPosition={chatPosition}
          currentUserPublicId={profilePublicId}
          locale={locale}
          placement="fullscreen"
          t={t}
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
        clientSettingsFolders={clientSettingsFolders}
        forceOnlinePublicIds={forceOnlinePublicIds}
        onClientSettingsFoldersChange={onClientSettingsFoldersChange}
        onFriendPartyInvite={handleLobbyFriendDrop}
        onFriendPartyJoin={handleJoinFriendParty}
        onLobbyFriendDrop={handleLobbyFriendDrop}
        onProfileOpen={handleProfileOpen}
        partyInviteEnabled={Boolean(activeLobby) && !partyInvitesLocked}
        presenceStatus={presenceStatus}
        profileAvatarUrl={profileAvatarUrl}
        profileName={profileName}
        profilePublicId={profilePublicId}
        t={t}
      />
      <ChatDock
        autoRooms={lobbyChatRooms}
        chatPosition={chatPosition}
        currentUserPublicId={profilePublicId}
        locale={locale}
        t={t}
      />

      {!gameSelectorOpen &&
      !gameInProgress &&
      (!activeLobby || !lobbyPageOpen || userPageOpen) ? (
        <div
          className={
            lobbyIsSearching
              ? "client-top-action client-top-action-searching"
              : "client-top-action"
          }
        >
          <button
            aria-pressed={gameSelectorOpen}
            className="client-play-button"
            data-animated={playButtonAnimated}
            type="button"
            onClick={(event) => {
              if (consumeTopActionDragClick(event)) {
                return;
              }

              handlePlayButtonClick();
            }}
            onPointerCancel={handleTopActionPointerEnd}
            onPointerDown={handleTopActionPointerDown}
            onPointerUp={handleTopActionPointerEnd}
          >
            <span>{activeLobby ? t("client-lobby") : t("client-play")}</span>
            {activeLobby ? (
              <span className="client-play-button-timer">{lobbySearchTime}</span>
            ) : null}
          </button>
          {activeLobby ? (
            <button
              aria-expanded={lobbyRosterOpen}
              aria-label={t("lobby-title")}
              className={
                lobbyRosterOpen
                  ? "client-lobby-roster client-lobby-roster-open"
                  : "client-lobby-roster"
              }
              type="button"
              onClick={(event) => {
                if (consumeTopActionDragClick(event)) {
                  return;
                }

                setLobbyRosterOpen((open) => !open);
              }}
              onPointerCancel={handleTopActionPointerEnd}
              onPointerDown={handleTopActionPointerDown}
              onPointerUp={handleTopActionPointerEnd}
            >
              <span className="client-lobby-roster-avatars" aria-hidden="true">
                {lobbyRosterMembers.map((member) => (
                  <span
                    className="client-lobby-roster-avatar"
                    key={member.publicId}
                    title={getLobbyMemberNameTag(member)}
                    onContextMenu={(event) => {
                      event.preventDefault();
                      event.stopPropagation();
                      void handleCopyLobbyRosterMember(member);
                    }}
                  >
                    {(member.publicId === profilePublicId
                      ? profileAvatarUrl
                      : getLobbyMemberPublicAvatarUrl(member, publicUsersByPublicId)) ? (
                      <img
                        alt=""
                        src={
                          member.publicId === profilePublicId
                            ? profileAvatarUrl
                            : getLobbyMemberPublicAvatarUrl(
                                member,
                                publicUsersByPublicId,
                              )
                        }
                      />
                    ) : (
                      getMemberName(member).charAt(0).toUpperCase()
                    )}
                  </span>
                ))}
              </span>
              <ChevronRight className="client-lobby-roster-chevron" size={18} />
            </button>
          ) : null}
        </div>
      ) : null}

      <section className="dashboard-panel" aria-label="Dashboard">
        {gameInProgress ? (
          <div className="client-game-running-message" role="status">
            <strong>
              {t(
                gameClientRunning
                  ? "client-game-running"
                  : gameClientClosedByClient
                    ? "client-game-closed-reconnect"
                    : "client-game-closed",
              )}
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
        {gameSelectorOpen || lobbyPageOpen || userPageOpen ? (
          <div className="client-page-actions">
            <button
              className="client-page-back-button"
              type="button"
              onClick={handleTopButtonClick}
            >
              <span className="client-page-back-arrow" aria-hidden="true">
                <ArrowLeft size={20} />
              </span>
              <span>{t("client-back")}</span>
            </button>
            {lobbyPageOpen && !userPageOpen && isCurrentUserLobbyHost ? (
              <button
                className="client-page-change-mode-button"
                type="button"
                onClick={handleChangeModeClick}
              >
                <SlidersHorizontal size={18} />
                <span>{t("client-change-mode")}</span>
              </button>
            ) : null}
          </div>
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
                      {inviters.slice(0, 3).map((inviter, index) => {
                        const inviterAvatarUrl = getLobbyMemberPublicAvatarUrl(
                          inviter,
                          publicUsersByPublicId,
                        );

                        return (
                          <span
                            className="lobby-member-avatar lobby-invite-avatar"
                            key={inviter.publicId ?? index}
                            style={{ zIndex: 3 - index }}
                          >
                            {inviterAvatarUrl ? (
                              <img alt="" src={inviterAvatarUrl} />
                            ) : (
                              getMemberName(inviter).charAt(0).toUpperCase()
                            )}
                          </span>
                        );
                      })}
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
                  onClick={handleGameModePrimaryAction}
                >
                  {activeLobby ? t("game-mode-change") : t("game-mode-create")}
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

        <section
          aria-hidden={!userPageOpen}
          aria-label={activeUserPageProfile.name}
          className={[
            "user-page",
            userPageOpen ? "user-page-open" : "",
            activeUserPageCategory === "overview" ? "" : "user-page-fullscreen",
          ]
            .filter(Boolean)
            .join(" ")}
        >
          <div
            className="user-page-wallpaper"
            style={{ "--user-page-wallpaper": `url(${liraWallpaper})` } as CSSProperties}
          >
            <div className="user-page-wallpaper-content">
              <div className="user-page-profile-banner" aria-label="Profile summary">
                <div
                  aria-label={
                    userPageShowsSelf ? locale === "de" ? "Bearbeiten" : "Edit" : undefined
                  }
                  className="user-page-avatar"
                  title={userPageShowsSelf ? locale === "de" ? "Bearbeiten" : "Edit" : undefined}
                >
                  {getProfileInitials(activeUserPageProfile.name)}
                  {activeUserPageProfile.avatarUrl ? (
                    <img
                      alt=""
                      src={activeUserPageProfile.avatarUrl}
                      onError={(event) => {
                        event.currentTarget.hidden = true;
                      }}
                    />
                  ) : null}
                  {userPageShowsSelf ? (
                    <span className="user-page-avatar-edit" aria-hidden="true">
                      <Pencil size={34} />
                    </span>
                  ) : null}
                </div>
                <span
                  className="user-page-banner-level"
                  aria-label={`Level ${activeUserPageProfile.level}`}
                >
                  {activeUserPageProfile.level}
                </span>
                <div className="user-page-identity">
                  <div className="user-page-name-row">
                    <h1
                      className={getUserPageNameClassName(activeUserPageProfile.name)}
                      title={activeUserPageProfile.name}
                    >
                      {activeUserPageProfile.name}
                    </h1>
                    {activeUserPageProfile.tagId ? (
                      <span className="user-page-inline-tag">
                        {formatTagId(activeUserPageProfile.tagId)}
                      </span>
                    ) : null}
                    <button
                      aria-label={t("profile-copy-name-tag")}
                      className="user-page-copy-id-button"
                      title={t("profile-copy-name-tag")}
                      type="button"
                      onClick={() => void handleCopyUserPageNameTag()}
                    >
                      <Copy size={15} />
                    </button>
                  </div>
                  {typeof activeUserPageProfile.publicId === "number" ? (
                    <div className="user-page-meta-row user-page-user-id-row">
                      <span>{`#${activeUserPageProfile.publicId}`}</span>
                      <button
                        aria-label={t("profile-copy-user-id")}
                        className="user-page-copy-id-button"
                        title={t("profile-copy-user-id")}
                        type="button"
                        onClick={() => void handleCopyUserPagePublicId()}
                      >
                        <Copy size={15} />
                      </button>
                    </div>
                  ) : null}
                </div>
              </div>
            </div>
          </div>
          {userPageShowsSelf && activeUserPageCategory === "champions" ? (
            <ProfileChampionsTab
              backSignal={championTabBackSignal}
              onFocusChange={setChampionTabFocused}
              t={t}
              userId={profilePublicId}
            />
          ) : null}
          <div className="user-page-details" />
          <nav className="user-page-categories" aria-label="Profile sections">
            {userPageCategories
              .filter((category) => userPageShowsSelf || category.id !== "champions")
              .map((category) => (
                <button
                  aria-current={activeUserPageCategory === category.id ? "page" : undefined}
                  aria-disabled={category.disabled ? true : undefined}
                  className={
                    [
                      "user-page-category",
                      activeUserPageCategory === category.id
                        ? "user-page-category-active"
                        : "",
                      category.disabled ? "user-page-category-disabled" : "",
                    ]
                      .filter(Boolean)
                      .join(" ")
                  }
                  disabled={category.disabled}
                  key={category.id}
                  tabIndex={userPageOpen && !category.disabled ? 0 : -1}
                  type="button"
                  onClick={() => {
                    if (category.disabled) {
                      return;
                    }

                    setChampionTabFocused(false);
                    setActiveUserPageCategory(category.id);
                  }}
                >
                  <span>{t(category.labelKey)}</span>
                </button>
              ))}
          </nav>
        </section>

        {activeLobby && lobbyPageOpen ? (
          <section
            className="lobby-page"
            aria-label={t("lobby-title")}
            style={
              {
                "--lobby-leave-color": lobbyLeaveColor,
              } as CSSProperties
            }
            onMouseDown={() => setOpenLobbyRolePicker(undefined)}
          >
            <div className="lobby-mode-label">
              <span>{getLobbyModeLabel(activeLobby, t)}</span>
            </div>
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
                const isCurrentUser = isSameLobbyMember(member, activeLobbyCurrentMember);
                const isHost = isSameLobbyMember(member, lobbyHost);
                const canInviteSlot = !member && !partyInvitesLocked;
                const canOpenMemberMenu = Boolean(member);
                const memberIdentity = member
                  ? getLobbyMemberDisplayIdentity(member)
                  : undefined;
                const memberName = member
                  ? memberIdentity?.name
                  : isCurrentUser
                    ? getLobbyDisplayName(profileName)
                    : undefined;
                const memberTagId = memberIdentity?.tagId;
                const memberLobbyRoles = isCurrentUser
                  ? selectedLobbyRoles
                  : getEffectiveLobbyMemberRoles(member);
                const visibleMemberLobbyRoles = visibleLobbyRoleSlots.map((roleSlot) => {
                  return memberLobbyRoles[roleSlot];
                });
                const hasVisibleMemberLobbyRoles = visibleMemberLobbyRoles.some(Boolean);

                return (
                  <div
                    className={
                      [
                        "lobby-player-slot",
                        isCurrentUser ? "lobby-player-slot-owner" : "",
                        isCurrentUser && openLobbyRolePicker !== undefined
                          ? "lobby-player-slot-role-picker-open"
                          : "",
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
                          (isCurrentUser
                            ? profileAvatarUrl
                            : getLobbyMemberPublicAvatarUrl(
                                member,
                                publicUsersByPublicId,
                              )) ? (
                            <img
                              alt=""
                              src={
                                isCurrentUser
                                  ? profileAvatarUrl
                                  : getLobbyMemberPublicAvatarUrl(
                                      member,
                                      publicUsersByPublicId,
                                    )
                              }
                            />
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
                      <span
                        className="lobby-player-name"
                        title={
                          memberName
                            ? memberTagId
                              ? `${memberName} ${memberTagId}`
                              : memberName
                            : t("lobby-slot-open")
                        }
                      >
                        <span className="lobby-player-name-text">
                          {memberName ?? t("lobby-slot-open")}
                        </span>
                        {memberTagId ? (
                          <span className="lobby-player-tag-id">{memberTagId}</span>
                        ) : null}
                      </span>
                      <small>
                        {member
                          ? isHost
                            ? "Host"
                            : t("lobby-slot-ready")
                          : t("lobby-slot-invite")}
                      </small>
                    </div>
                    {isCurrentUser ? (
                      <>
                        <div
                          className={
                            lobbyIsFull
                              ? "lobby-owner-actions lobby-owner-actions-single"
                              : "lobby-owner-actions"
                          }
                        >
                          {visibleLobbyRoleSlots.map((roleSlot) => {
                            const selectedRoleId = selectedLobbyRoles[roleSlot];
                            const selectedRole = lobbyRoles.find((role) => {
                              return role.id === selectedRoleId;
                            });
                            const duplicateRoleSlot = roleSlot === 0 ? 1 : 0;

                            return (
                              <div
                                className={
                                  openLobbyRolePicker === roleSlot
                                    ? "lobby-role-picker lobby-role-picker-open"
                                    : "lobby-role-picker"
                                }
                                key={roleSlot}
                                onMouseDown={(event) => event.stopPropagation()}
                              >
                                <button
                                  aria-expanded={openLobbyRolePicker === roleSlot}
                                  aria-label={t("lobby-role-select")}
                                  className={
                                    selectedRole
                                      ? "lobby-role-trigger lobby-role-trigger-selected"
                                      : "lobby-role-trigger"
                                  }
                                  title={
                                    selectedRole
                                      ? t(selectedRole.labelKey)
                                      : t("lobby-role-select")
                                  }
                                  type="button"
                                  onClick={() =>
                                    setOpenLobbyRolePicker((openSlot) =>
                                      openSlot === roleSlot ? undefined : roleSlot,
                                    )
                                  }
                                >
                                  {selectedRole ? (
                                    <LobbyRoleIcon role={selectedRole.id} />
                                  ) : (
                                    <Plus size={18} />
                                  )}
                                </button>
                                {openLobbyRolePicker === roleSlot ? (
                                  <div
                                    className="lobby-role-wheel"
                                    role="menu"
                                    onMouseDown={(event) => event.stopPropagation()}
                                  >
                                    {lobbyRoles.map((role) => {
                                      const roleTaken =
                                        selectedLobbyRoles[duplicateRoleSlot] === role.id;
                                      const roleSelected = selectedRoleId === role.id;

                                      return (
                                        <button
                                          aria-checked={roleSelected}
                                          className={
                                            roleSelected
                                              ? "lobby-role-option lobby-role-option-selected"
                                              : "lobby-role-option"
                                          }
                                          disabled={roleTaken}
                                          key={role.id}
                                          role="menuitemradio"
                                          type="button"
                                          onClick={() =>
                                            handleLobbyRoleSelect(roleSlot, role.id)
                                          }
                                        >
                                          <span className="lobby-role-option-content">
                                            <LobbyRoleIcon role={role.id} />
                                          </span>
                                        </button>
                                      );
                                    })}
                                  </div>
                                ) : null}
                              </div>
                            );
                          })}
                        </div>
                      </>
                    ) : member && hasVisibleMemberLobbyRoles ? (
                      <div
                        className={
                          lobbyIsFull
                            ? "lobby-member-roles lobby-member-roles-single"
                            : "lobby-member-roles"
                        }
                        aria-label={t("lobby-role-select")}
                      >
                        {visibleMemberLobbyRoles.map((roleId, roleSlot) => {
                          if (!roleId) {
                            return (
                              <span
                                aria-hidden="true"
                                className="lobby-member-role-placeholder"
                                key={roleSlot}
                              />
                            );
                          }

                          const selectedRole = lobbyRoles.find((role) => {
                            return role.id === roleId;
                          });

                          return (
                            <span
                              className="lobby-member-role"
                              key={roleSlot}
                              title={selectedRole ? t(selectedRole.labelKey) : undefined}
                            >
                              <LobbyRoleIcon role={roleId} />
                            </span>
                          );
                        })}
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
              {lobbySearchDisabledByRoles ? (
                <p className="lobby-search-role-warning">
                  {t("lobby-roles-required")}
                </p>
              ) : null}
              <div className="lobby-search-actions">
                <button
                  aria-label={t("lobby-leave")}
                  className="lobby-leave-button"
                  type="button"
                  onClick={() => void leaveCurrentLobby()}
                >
                  <X size={22} />
                </button>
                <button
                  className="lobby-search-button"
                  disabled={!isCurrentUserLobbyHost || lobbySearchDisabledByRoles}
                  type="button"
                  onClick={() => void handleLobbySearch()}
                >
                  {lobbyIsSearching ? t("lobby-search-abort") : t("lobby-search")}
                </button>
              </div>
            </div>

          </section>
        ) : null}
      </section>

      {lobbyMemberContextMenu && activeLobby ? (() => {
        const member = lobbyMemberContextMenu.member;
        const memberPublicId = member.publicId;
        const isSelf = isSameLobbyMember(member, activeLobbyCurrentMember);
        const lobbyHost = getLobbyHost(activeLobby);
        const isCurrentUserHost = isSameLobbyMember(
          lobbyHost,
          activeLobbyCurrentMember,
        );
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

      {partyInviteOpen && activeLobby && !partyInvitesLocked ? (
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
                onChange={(event) => {
                  setPartyInviteSearch(event.target.value);
                  setPartyInviteOnlinePage(0);
                }}
              />
              {partyInviteSearching ? (
                <span>{t("friend-add-searching")}</span>
              ) : null}
            </label>

            <div className="friend-add-list">
              {inviteCandidates.length > 0 ? (
                inviteCandidates.map((candidate) => {
                  const candidateKey = getInviteCandidateKey(candidate);
                  const candidateSubtitle = getInviteCandidateSubtitle(candidate, {
                    showEmail: canShowInviteCandidateEmail(candidate),
                  });
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
                        <span title={candidate.name}>{candidate.name}</span>
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
                  {partyInviteSearch.trim().length >= 1
                    ? t("friend-add-no-results")
                    : t("lobby-invite-empty")}
                </p>
              )}
            </div>

            {partyInviteShowPagination ? (
              <div className="friend-add-pagination">
                <button
                  aria-label="Previous page"
                  disabled={!partyInviteCanPagePrevious}
                  type="button"
                  onClick={() =>
                    setPartyInviteOnlinePage((currentPage) =>
                      Math.max(0, currentPage - 1),
                    )
                  }
                >
                  <ChevronLeft size={16} />
                </button>
                <span>
                  {partyInviteOnlinePage + 1} / {partyInviteCandidateTotalPages}
                </span>
                <button
                  aria-label="Next page"
                  disabled={!partyInviteCanPageNext}
                  type="button"
                  onClick={() =>
                    setPartyInviteOnlinePage((currentPage) =>
                      Math.min(
                        Math.max(0, partyInviteCandidateTotalPages - 1),
                        currentPage + 1,
                      ),
                    )
                  }
                >
                  <ChevronRight size={16} />
                </button>
              </div>
            ) : null}
          </div>
        </div>
      ) : null}

      {pendingMatch ? (
        <MatchFoundDialog
          acceptedCount={matchFoundAcceptedCount}
          busy={Boolean(matchDecisionBusy)}
          countdownClassName={matchFoundCountdownClassName}
          countdownStyle={matchFoundCountdownStyle}
          currentPlayerAccepted={currentPlayerAccepted}
          overlayStroke={matchFoundOverlayStroke}
          remainingSeconds={matchFoundRemainingSeconds}
          requiredAcceptCount={matchFoundRequiredAcceptCount}
          t={t}
          onAccept={() => void handleMatchDecision("accept")}
          onDecline={() => void handleMatchDecision("decline")}
        />
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
          backgroundChampion={backgroundChampion}
          chatPosition={chatPosition}
          clientAnimation={clientAnimation}
          friendRequestPolicy={friendRequestPolicy}
          gameScreenMode={gameScreenMode}
          locale={locale}
          resolution={resolution}
          showEmailPublic={showEmailPublic}
          supportsFourKResolution={supportsFourKResolution}
          supportsTwoKResolution={supportsTwoKResolution}
          t={t}
          uiScale={uiScale}
          vision="Vision.ALL"
          onAccentColorChange={onAccentColorChange}
          onBackgroundChampionChange={onBackgroundChampionChange}
          onChatPositionChange={onChatPositionChange}
          onClientAnimationChange={onClientAnimationChange}
          onClose={onSettingsClose}
          onFriendRequestPolicyChange={onFriendRequestPolicyChange}
          onGameScreenModeChange={onGameScreenModeChange}
          onLocaleChange={onLocaleChange}
          onResolutionChange={onResolutionChange}
          onShowEmailPublicChange={onShowEmailPublicChange}
          onUiScaleChange={onUiScaleChange}
        />
      ) : null}
    </>
  );
}

export default Client;
