import {
  Bell,
  Check,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Folder,
  FolderOpen,
  FolderPlus,
  MoreHorizontal,
  Pencil,
  Search,
  Shield,
  Trash2,
  Trophy,
  UserPlus,
  Users,
  X,
} from "lucide-react";
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type PointerEvent,
} from "react";
import { createPortal } from "react-dom";
import {
  bootstrap as liveBootstrap,
  client,
  friends as fetchFriendStatuses,
  liveAcceptRequest,
  liveDeclineRequest,
  liveRemoveFriend,
  liveRevokeRequest,
  liveSendRequest,
  search as searchUsers,
  type FriendRequest,
  type FriendRequestResponse,
  type FriendUser,
  type FriendUserResponse,
  type LobbySnapshot,
  type UserStatusSnapshot,
} from "../api/client";
import { LIVE_API_BASE_URL } from "../api/config";
import FriendCard from "./FriendCard";
import type {
  FriendFolder,
  FriendProfile,
  PresenceStatus,
  SidebarTab,
  Translate,
} from "../types/ui";
import { presenceMessageIds } from "../types/ui";
import {
  getAvatarUrl,
  getProfileInitials,
  getPublicDisplayName,
} from "../utils/profile";
import { useNotifications } from "../notifications";

const initialFolders: FriendFolder[] = [
  {
    id: "duo",
    name: "Duo Queue",
    open: true,
  },
];

const friendSidebarStorageKey = "mira-client-friend-sidebar-v2";

type DragState = {
  active: boolean;
  friendId: string;
  overFolderId?: string;
  startX: number;
  startY: number;
  x: number;
  y: number;
};

type FriendSidebarStorage = {
  friendFolders?: Record<string, string | undefined>;
  folders?: FriendFolder[];
  initialized?: boolean;
};

type FriendRequestsState = {
  incoming: FriendRequestItem[];
  outgoing: FriendRequestItem[];
};

type FriendAddTab = "add" | "incoming" | "outgoing";

type FriendTooltipState = {
  friendId: string;
  left: number;
  top: number;
};

type FriendRequestItem = FriendRequestResponse | FriendRequest;

type FriendUserItem = FriendUserResponse | FriendUser;

type FriendUserAvatarFields = FriendUserItem & {
  avatarUrl?: string;
  imageUrl?: string;
  picture?: string;
  pictureUrl?: string;
  profileImageUrl?: string;
};

type SidebarProps = {
  activeLobbyId?: string;
  activeLobbyMemberPublicIds?: number[];
  forceOnlinePublicIds?: number[];
  onLobbyFriendDrop?: (friend: FriendProfile) => void;
  onFriendPartyInvite?: (friend: FriendProfile) => void;
  onFriendPartyJoin?: (lobby: LobbySnapshot) => void;
  partyInviteEnabled?: boolean;
  profileAvatarUrl?: string;
  presenceStatus: PresenceStatus;
  profileName: string;
  profilePublicId?: number;
  t: Translate;
};

function isStoredFolder(value: unknown): value is FriendFolder {
  if (!value || typeof value !== "object") {
    return false;
  }

  const folder = value as FriendFolder;
  return (
    typeof folder.id === "string" &&
    typeof folder.name === "string" &&
    typeof folder.open === "boolean"
  );
}

function readStoredFriendSidebar() {
  try {
    const storedSidebar = localStorage.getItem(friendSidebarStorageKey);

    if (!storedSidebar) {
      return {};
    }

    return JSON.parse(storedSidebar) as FriendSidebarStorage;
  } catch {
    return {};
  }
}

function getInitialFolders(storedSidebar: FriendSidebarStorage) {
  const validStoredFolders = Array.isArray(storedSidebar.folders)
    ? storedSidebar.folders.filter(isStoredFolder)
    : [];

  if (storedSidebar.initialized) {
    return validStoredFolders;
  }

  const storedFolderIds = new Set(validStoredFolders.map((folder) => folder.id));

  return [
    ...validStoredFolders,
    ...initialFolders.filter((folder) => !storedFolderIds.has(folder.id)),
  ];
}

function getFriendUserId(user: FriendUserItem) {
  if (typeof user.publicId === "number") {
    return String(user.publicId);
  }

  return user.email ?? user.displayName ?? "unknown-user";
}

function getFriendUserName(user: FriendUserItem) {
  return getPublicDisplayName(
    user.displayName,
    `User ${user.publicId ?? ""}`.trim(),
  );
}

function getFriendUserSubtitle(user: FriendUserItem) {
  return typeof user.publicId === "number" ? `#${user.publicId}` : undefined;
}

function formatNotificationTime(createdAt: number) {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(createdAt));
}

function getFriendUserAvatarUrl(user?: FriendUserItem) {
  return getAvatarUrl(user as FriendUserAvatarFields | undefined);
}

function getRequestUser(
  request: FriendRequestItem,
  direction: "incoming" | "outgoing",
) {
  return direction === "incoming" ? request.requester : request.addressee;
}

function isPendingFriendRequest(request: FriendRequestItem) {
  return !request.status || request.status.toLowerCase() === "pending";
}

function getFriendApiErrorMessage(label: string, response?: Response) {
  return response ? `${label}: HTTP ${response.status}` : label;
}

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

function getFriendLobbyPresence(
  publicId: number | undefined,
  lobbies: LobbySnapshot[],
): PresenceStatus | undefined {
  if (typeof publicId !== "number") {
    return undefined;
  }

  const lobby = lobbies.find((currentLobby) =>
    currentLobby.members?.some((member) => member.publicId === publicId),
  );

  if (!lobby) {
    return undefined;
  }

  return lobby.status === "SEARCHING" ? "inqueue" : "inlobby";
}

function mapApiFriendsToProfiles(
  apiFriends: FriendUserItem[],
  folders: FriendFolder[],
  friendFolders?: Record<string, string | undefined>,
  friendStatuses: UserStatusSnapshot[] = [],
  openLobbies: LobbySnapshot[] = [],
  forceOnlinePublicIds: number[] = [],
) {
  const folderIds = new Set(folders.map((folder) => folder.id));
  const forcedOnlinePublicIds = new Set(forceOnlinePublicIds);
  const statusesByPublicId = new Map(
    friendStatuses
      .filter((status) => typeof status.publicId === "number")
      .map((status) => [status.publicId, status]),
  );

  return apiFriends.map((friend) => {
    const id = getFriendUserId(friend);
    const folderId = friendFolders?.[id];
    const userStatus = statusesByPublicId.get(friend.publicId);
    const lobbyPresence = getFriendLobbyPresence(friend.publicId, openLobbies);
    const forcedOnline =
      typeof friend.publicId === "number" &&
      forcedOnlinePublicIds.has(friend.publicId);
    const apiPresence = userStatus
      ? mapUserStatusToPresence(userStatus.status, userStatus.mode)
      : undefined;
    const status =
      lobbyPresence && (!apiPresence || apiPresence === "offline" || apiPresence === "online")
        ? lobbyPresence
        : forcedOnline && apiPresence === "offline"
          ? "online"
          : apiPresence ?? lobbyPresence ?? "offline";
    const gameMode =
      status === "inqueue" || status === "championselection"
        ? undefined
        : userStatus?.mode;

    return {
      avatarUrl: getFriendUserAvatarUrl(friend),
      email: friend.email,
      folderId: folderIds.has(folderId ?? "") ? folderId : undefined,
      id,
      name: getFriendUserName(friend),
      publicId: friend.publicId,
      status,
      gameMode,
      rank: {
        name: "wood",
        label: "Wood",
        tier: "I",
      },
    } satisfies FriendProfile;
  });
}

function Sidebar({
  activeLobbyId,
  activeLobbyMemberPublicIds = [],
  forceOnlinePublicIds = [],
  onFriendPartyInvite,
  onFriendPartyJoin,
  onLobbyFriendDrop,
  partyInviteEnabled,
  presenceStatus,
  profileAvatarUrl,
  profileName,
  profilePublicId,
  t,
}: SidebarProps) {
  const [activeSidebarTab, setActiveSidebarTab] = useState<SidebarTab>("friends");
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const storedSidebar = useMemo(() => readStoredFriendSidebar(), []);
  const [folders, setFolders] = useState(() => {
    return getInitialFolders(storedSidebar);
  });
  const [friendFolders, setFriendFolders] = useState<
    Record<string, string | undefined>
  >(() => storedSidebar.friendFolders ?? {});
  const [friends, setFriends] = useState<FriendProfile[]>([]);
  const [friendRequests, setFriendRequests] = useState<FriendRequestsState>({
    incoming: [],
    outgoing: [],
  });
  const [openFriendLobbies, setOpenFriendLobbies] = useState<LobbySnapshot[]>([]);
  const [friendSearch, setFriendSearch] = useState("");
  const [friendAddOpen, setFriendAddOpen] = useState(false);
  const [friendAddTab, setFriendAddTab] = useState<FriendAddTab>("add");
  const [friendAddSearch, setFriendAddSearch] = useState("");
  const [friendAddSearching, setFriendAddSearching] = useState(false);
  const [friendSearchResults, setFriendSearchResults] = useState<
    FriendUserResponse[]
  >([]);
  const [friendActionBusyId, setFriendActionBusyId] = useState<number>();
  const [friendRequestBusyId, setFriendRequestBusyId] = useState<number>();
  const [, setFriendApiError] = useState<string>();
  const [openMenuFriendId, setOpenMenuFriendId] = useState<string>();
  const [openMenuFolderId, setOpenMenuFolderId] = useState<string>();
  const [folderCreateOpen, setFolderCreateOpen] = useState(false);
  const [folderCreateInput, setFolderCreateInput] = useState("");
  const [renamingFolderId, setRenamingFolderId] = useState<string>();
  const [folderRenameInput, setFolderRenameInput] = useState("");
  const [friendTooltip, setFriendTooltip] = useState<FriendTooltipState>();
  const [notificationsOpen, setNotificationsOpen] = useState(false);
  const [profileMenuOpen, setProfileMenuOpen] = useState(false);
  const [dragState, setDragState] = useState<DragState>();
  const dragStateRef = useRef<DragState | undefined>(undefined);
  const foldersRef = useRef(folders);
  const friendFoldersRef = useRef(friendFolders);
  const createFolderInputRef = useRef<HTMLInputElement | null>(null);
  const friendAddSearchInputRef = useRef<HTMLInputElement | null>(null);
  const renameInputRef = useRef<HTMLInputElement | null>(null);
  const notificationMenuRef = useRef<HTMLDivElement | null>(null);
  const profileMenuRef = useRef<HTMLDivElement | null>(null);
  const { clearNotifications, notifications, notify, removeNotification } =
    useNotifications();

  const normalizedSearch = friendSearch.trim().toLowerCase();
  const incomingFriendRequests = friendRequests.incoming.filter(
    isPendingFriendRequest,
  );
  const outgoingFriendRequests = friendRequests.outgoing.filter(
    isPendingFriendRequest,
  );
  const outgoingFriendPublicIds = new Set(
    outgoingFriendRequests
      .map((request) => request.addressee?.publicId)
      .filter((publicId): publicId is number => typeof publicId === "number"),
  );
  const friendPublicIds = new Set(
    friends
      .map((friend) => friend.publicId)
      .filter((publicId): publicId is number => typeof publicId === "number"),
  );
  const friendRequestCount = incomingFriendRequests.length;
  const notificationCount = notifications.length;

  function notifyFriendApiError(message: string) {
    setFriendApiError(message);
    notify({
      type: "error",
      message,
    });
  }

  const visibleFriends = useMemo(
    () =>
      normalizedSearch
        ? friends.filter((friend) =>
            friend.name.toLowerCase().includes(normalizedSearch),
          )
        : friends,
    [friends, normalizedSearch],
  );
  const unfiledFriends = visibleFriends.filter((friend) => !friend.folderId);
  const draggingFriend = dragState
    ? friends.find((friend) => friend.id === dragState.friendId)
    : undefined;
  const dragInProgress = Boolean(dragState);
  const tooltipFriend = friendTooltip
    ? friends.find((friend) => friend.id === friendTooltip.friendId)
    : undefined;
  const overlayRoot =
    typeof document === "undefined" ? undefined : document.body;
  const sidebarToggleLabel = sidebarCollapsed
    ? t("sidebar-expand")
    : t("sidebar-collapse");
  const notificationMenuClassName = sidebarCollapsed
    ? "sidebar-notification-menu sidebar-notification-menu-profile"
    : "sidebar-notification-menu";

  function renderNotificationMenu() {
    return (
      <div className={notificationMenuClassName} role="menu">
        <div className="sidebar-notification-menu-header">
          <strong>{t("notification-title")}</strong>
          <button
            disabled={notificationCount === 0}
            type="button"
            onClick={clearNotifications}
          >
            {t("notification-clear-all")}
          </button>
        </div>

        <div className="sidebar-notification-list">
          {notifications.length > 0 ? (
            notifications.map((notification) => (
              <div
                className={`sidebar-notification-item notification-${notification.type}`}
                key={notification.id}
                role="menuitem"
              >
                <div className="sidebar-notification-item-copy">
                  <div className="sidebar-notification-meta">
                    <span>{t(`notification-${notification.type}`)}</span>
                    <time dateTime={new Date(notification.createdAt).toISOString()}>
                      {formatNotificationTime(notification.createdAt)}
                    </time>
                  </div>
                  {notification.title ? <strong>{notification.title}</strong> : null}
                  <p>{notification.message}</p>
                </div>
                <button
                  className="sidebar-notification-delete"
                  type="button"
                  aria-label={t("notification-delete")}
                  onClick={() => removeNotification(notification.id)}
                >
                  <Trash2 size={15} />
                </button>
              </div>
            ))
          ) : (
            <p className="sidebar-notification-empty">{t("notification-empty")}</p>
          )}
        </div>
      </div>
    );
  }

  useEffect(() => {
    dragStateRef.current = dragState;
  }, [dragState]);

  useEffect(() => {
    foldersRef.current = folders;
  }, [folders]);

  useEffect(() => {
    friendFoldersRef.current = friendFolders;
  }, [friendFolders]);

  useEffect(() => {
    if (!notificationsOpen && !profileMenuOpen) {
      return;
    }

    function closeUserMenusOnOutsidePointer(event: globalThis.PointerEvent) {
      if (
        event.target instanceof Node &&
        (notificationMenuRef.current?.contains(event.target) ||
          profileMenuRef.current?.contains(event.target))
      ) {
        return;
      }

      setNotificationsOpen(false);
      setProfileMenuOpen(false);
    }

    window.addEventListener("pointerdown", closeUserMenusOnOutsidePointer);

    return () => {
      window.removeEventListener("pointerdown", closeUserMenusOnOutsidePointer);
    };
  }, [notificationsOpen, profileMenuOpen]);

  useEffect(() => {
    localStorage.setItem(
      friendSidebarStorageKey,
      JSON.stringify({
        folders,
        friendFolders,
        initialized: true,
      }),
    );
  }, [folders, friendFolders]);

  useEffect(() => {
    let active = true;
    const abortController = new AbortController();

    async function listenForLiveFriendEvents() {
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

          await refreshLiveData();
        }
      } catch {
        if (!active || abortController.signal.aborted) {
          return;
        }

        notifyFriendApiError(t("friend-api-error"));
      }
    }

    void refreshLiveData();
    void listenForLiveFriendEvents();
    const refreshIntervalId = window.setInterval(() => {
      void refreshLiveData();
    }, 30_000);

    return () => {
      active = false;
      abortController.abort();
      window.clearInterval(refreshIntervalId);
    };
  }, [t]);

  useEffect(() => {
    void refreshLiveData();
  }, [activeLobbyId, forceOnlinePublicIds]);

  useEffect(() => {
    const query = friendAddSearch.trim();

    if (!friendAddOpen || query.length < 2) {
      setFriendSearchResults([]);
      setFriendAddSearching(false);
      return;
    }

    let active = true;
    setFriendAddSearching(true);

    const searchTimeout = window.setTimeout(async () => {
      const result = await searchUsers({ query: { q: query } });

      if (!active) {
        return;
      }

      if (result.error) {
        notifyFriendApiError(t("friend-api-error"));
        setFriendSearchResults([]);
      } else {
        setFriendSearchResults(result.data?.users ?? []);
        setFriendApiError(undefined);
      }

      setFriendAddSearching(false);
    }, 240);

    return () => {
      active = false;
      window.clearTimeout(searchTimeout);
    };
  }, [friendAddOpen, friendAddSearch, t]);

  useEffect(() => {
    if (!renamingFolderId) {
      return;
    }

    renameInputRef.current?.focus();
    renameInputRef.current?.select();
  }, [renamingFolderId]);

  useEffect(() => {
    if (!folderCreateOpen) {
      return;
    }

    createFolderInputRef.current?.focus();
    createFolderInputRef.current?.select();
  }, [folderCreateOpen]);

  useEffect(() => {
    if (!friendAddOpen || friendAddTab !== "add") {
      return;
    }

    friendAddSearchInputRef.current?.focus();
  }, [friendAddOpen, friendAddTab]);

  useEffect(() => {
    if (!openMenuFriendId && !openMenuFolderId) {
      return;
    }

    function closeMenu() {
      setOpenMenuFriendId(undefined);
      setOpenMenuFolderId(undefined);
    }

    window.addEventListener("click", closeMenu);

    return () => {
      window.removeEventListener("click", closeMenu);
    };
  }, [openMenuFriendId, openMenuFolderId]);

  useEffect(() => {
    if (!dragState) {
      return;
    }

    function handlePointerMove(event: globalThis.PointerEvent) {
      const currentDragState = dragStateRef.current;

      if (!currentDragState) {
        return;
      }

      const deltaX = event.clientX - currentDragState.startX;
      const deltaY = event.clientY - currentDragState.startY;
      const isActive =
        currentDragState.active || Math.hypot(deltaX, deltaY) > 4;
      const dropTarget = document
        .elementFromPoint(event.clientX, event.clientY)
        ?.closest<HTMLElement>("[data-folder-drop-id]");

      setDragState({
        ...currentDragState,
        active: isActive,
        overFolderId: dropTarget?.dataset.folderDropId,
        x: event.clientX,
        y: event.clientY,
      });
    }

    function handlePointerUp(event: globalThis.PointerEvent) {
      const currentDragState = dragStateRef.current;

      if (!currentDragState) {
        setDragState(undefined);
        return;
      }

      if (currentDragState.active && currentDragState.overFolderId) {
        moveFriendToFolder(
          currentDragState.friendId,
          currentDragState.overFolderId,
        );
      }

      const lobbyDropTarget = document
        .elementFromPoint(event.clientX, event.clientY)
        ?.closest<HTMLElement>("[data-lobby-invite-drop]");
      const droppedFriend = friends.find(
        (friend) => friend.id === currentDragState.friendId,
      );

      if (currentDragState.active && lobbyDropTarget && droppedFriend) {
        onLobbyFriendDrop?.(droppedFriend);
      }

      setDragState(undefined);
    }

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp, { once: true });

    return () => {
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    };
  }, [dragInProgress, friends, onLobbyFriendDrop]);

  async function refreshLiveData(
    nextFolders = foldersRef.current,
    nextFriendFolders = friendFoldersRef.current,
  ) {
    const [result, statusesResult] = await Promise.all([
      liveBootstrap({
        baseUrl: LIVE_API_BASE_URL,
      }),
      fetchFriendStatuses({
        baseUrl: LIVE_API_BASE_URL,
      }),
    ]);

    if (result.error) {
      notifyFriendApiError(
        getFriendApiErrorMessage(t("friend-api-error"), result.response),
      );
      return;
    }

    const openLobbies = result.data?.openFriendLobbies ?? [];
    const friendStatuses =
      statusesResult.data?.statuses ?? result.data?.friendStatuses?.statuses ?? [];

    setFriends(
      mapApiFriendsToProfiles(
        result.data?.friends?.friends ?? [],
        nextFolders,
        nextFriendFolders,
        friendStatuses,
        openLobbies,
        forceOnlinePublicIds,
      ),
    );
    setFriendRequests({
      incoming: result.data?.friendRequests?.incoming ?? [],
      outgoing: result.data?.friendRequests?.outgoing ?? [],
    });
    setOpenFriendLobbies(openLobbies);
    setFriendApiError(undefined);
  }

  function handleCreateFolder() {
    const folderNumber = folders.length + 1;

    setFolderCreateInput(`${t("friend-folder")} ${folderNumber}`);
    setFolderCreateOpen(true);
  }

  function commitCreateFolder() {
    const folderNumber = folders.length + 1;
    const folderName = folderCreateInput.trim() || `${t("friend-folder")} ${folderNumber}`;

    setFolders((currentFolders) => [
      ...currentFolders,
      {
        id: `folder-${Date.now()}`,
        name: folderName,
        open: true,
      },
    ]);
    setFolderCreateOpen(false);
  }

  function startRenameFolder(folderId: string) {
    const folder = folders.find((currentFolder) => currentFolder.id === folderId);

    if (!folder) {
      return;
    }

    setFolderRenameInput(folder.name);
    setOpenMenuFolderId(undefined);
    setRenamingFolderId(folderId);
  }

  function commitRenameFolder() {
    if (!renamingFolderId) {
      return;
    }

    const nextName = folderRenameInput.trim();

    if (!nextName) {
      setRenamingFolderId(undefined);
      return;
    }

    setFolders((currentFolders) =>
      currentFolders.map((currentFolder) =>
        currentFolder.id === renamingFolderId
          ? { ...currentFolder, name: nextName }
          : currentFolder,
      ),
    );
    setRenamingFolderId(undefined);
  }

  function handleRenameKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Enter") {
      commitRenameFolder();
    }

    if (event.key === "Escape") {
      setRenamingFolderId(undefined);
    }
  }

  function handleDeleteFolder(folderId: string) {
    setFriendFolders((currentFriendFolders) =>
      Object.fromEntries(
        Object.entries(currentFriendFolders).filter(([, currentFolderId]) => {
          return currentFolderId !== folderId;
        }),
      ),
    );
    setFolders((currentFolders) =>
      currentFolders.filter((folder) => folder.id !== folderId),
    );
    setFriends((currentFriends) =>
      currentFriends.map((friend) =>
        friend.folderId === folderId ? { ...friend, folderId: undefined } : friend,
      ),
    );
    setOpenMenuFolderId(undefined);
  }

  function moveFriendToFolder(friendId: string, folderId: string) {
    setFriendFolders((currentFriendFolders) => ({
      ...currentFriendFolders,
      [friendId]: folderId,
    }));
    setFriends((currentFriends) =>
      currentFriends.map((friend) =>
        friend.id === friendId ? { ...friend, folderId } : friend,
      ),
    );
    setFolders((currentFolders) =>
      currentFolders.map((folder) =>
        folder.id === folderId ? { ...folder, open: true } : folder,
      ),
    );
    setOpenMenuFriendId(undefined);
    setOpenMenuFolderId(undefined);
  }

  async function handleUnfriend(friendId: string) {
    const friend = friends.find((currentFriend) => currentFriend.id === friendId);

    setOpenMenuFriendId(undefined);

    if (typeof friend?.publicId !== "number") {
      return;
    }

    setFriendActionBusyId(friend.publicId);
    const result = await liveRemoveFriend({
      baseUrl: LIVE_API_BASE_URL,
      path: { friendPublicId: friend.publicId },
    });
    setFriendActionBusyId(undefined);

    if (result.error) {
      notifyFriendApiError(t("friend-api-error"));
      return;
    }

    setFriendFolders((currentFriendFolders) => {
      const nextFriendFolders = { ...currentFriendFolders };
      delete nextFriendFolders[friendId];
      return nextFriendFolders;
    });
    await refreshLiveData();
  }

  function handleChat(friendId: string) {
    window.dispatchEvent(
      new CustomEvent("mira:chat-request", {
        detail: { friendId },
      }),
    );
    setOpenMenuFriendId(undefined);
    setOpenMenuFolderId(undefined);
  }

  function handleJoinParty(friendId: string) {
    const friend = friends.find((currentFriend) => currentFriend.id === friendId);

    if (typeof friend?.publicId !== "number") {
      return;
    }

    const friendLobby = openFriendLobbies.find((lobby) => {
      const friendIsMember = lobby.members?.some((member) => {
        return member.publicId === friend.publicId;
      });
      const selfIsMember = lobby.members?.some((member) => {
        return member.publicId === profilePublicId;
      });

      return (
        friendIsMember &&
        lobby.id !== activeLobbyId &&
        !selfIsMember
      );
    });

    if (!friendLobby) {
      return;
    }

    onFriendPartyJoin?.(friendLobby);
    setOpenMenuFriendId(undefined);
    setOpenMenuFolderId(undefined);
  }

  function handleInviteParty(friendId: string) {
    const friend = friends.find((currentFriend) => currentFriend.id === friendId);

    if (!friend) {
      return;
    }

    onFriendPartyInvite?.(friend);
    setOpenMenuFriendId(undefined);
    setOpenMenuFolderId(undefined);
  }

  function handleFriendPointerDown(
    friendId: string,
    event: PointerEvent<HTMLElement>,
  ) {
    if (event.button !== 0) {
      return;
    }

    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    setFriendTooltip(undefined);
    setDragState({
      active: true,
      friendId,
      startX: event.clientX,
      startY: event.clientY,
      x: event.clientX,
      y: event.clientY,
    });
  }

  function toggleFolder(folderId: string) {
    setFolders((currentFolders) =>
      currentFolders.map((folder) =>
        folder.id === folderId ? { ...folder, open: !folder.open } : folder,
      ),
    );
  }

  async function handleSendFriendRequest(targetPublicId?: number) {
    if (typeof targetPublicId !== "number") {
      return;
    }

    setFriendActionBusyId(targetPublicId);
    const result = await liveSendRequest({
      baseUrl: LIVE_API_BASE_URL,
      body: { targetPublicId },
    });
    setFriendActionBusyId(undefined);

    if (result.error) {
      notifyFriendApiError(t("friend-api-error"));
      return;
    }

    setFriendApiError(undefined);
    await refreshLiveData();
  }

  async function handleAcceptRequest(requestId?: number) {
    if (typeof requestId !== "number") {
      return;
    }

    setFriendRequestBusyId(requestId);
    const result = await liveAcceptRequest({
      baseUrl: LIVE_API_BASE_URL,
      path: { requestId },
    });
    setFriendRequestBusyId(undefined);

    if (result.error) {
      notifyFriendApiError(t("friend-api-error"));
      return;
    }

    await refreshLiveData();
  }

  async function handleDeclineRequest(requestId?: number) {
    if (typeof requestId !== "number") {
      return;
    }

    setFriendRequestBusyId(requestId);
    const result = await liveDeclineRequest({
      baseUrl: LIVE_API_BASE_URL,
      path: { requestId },
    });
    setFriendRequestBusyId(undefined);

    if (result.error) {
      notifyFriendApiError(t("friend-api-error"));
      return;
    }

    await refreshLiveData();
  }

  async function handleRevokeRequest(requestId?: number) {
    if (typeof requestId !== "number") {
      return;
    }

    setFriendRequestBusyId(requestId);
    const result = await liveRevokeRequest({
      baseUrl: LIVE_API_BASE_URL,
      path: { requestId },
    });
    setFriendRequestBusyId(undefined);

    if (result.error) {
      notifyFriendApiError(t("friend-api-error"));
      return;
    }

    await refreshLiveData();
  }

  function renderFriendUserAvatar(user?: FriendUserItem) {
    const avatarUrl = getFriendUserAvatarUrl(user);
    const name = getFriendUserName(user ?? {});

    return (
      <span className="friend-add-avatar" aria-hidden="true">
        {getProfileInitials(name)}
        {avatarUrl ? (
          <img
            alt=""
            className="friend-avatar-image"
            referrerPolicy="no-referrer"
            src={avatarUrl}
            onError={(event) => {
              event.currentTarget.hidden = true;
            }}
          />
        ) : null}
      </span>
    );
  }

  function renderFriendCards(folderFriends: FriendProfile[]) {
    return folderFriends.map((friend) => {
      const friendIsInActiveLobby =
        typeof friend.publicId === "number" &&
        activeLobbyMemberPublicIds.includes(friend.publicId);
      const canJoinParty =
        typeof friend.publicId === "number" &&
        openFriendLobbies.some((lobby) => {
          const friendIsMember = lobby.members?.some((member) => {
            return member.publicId === friend.publicId;
          });
          const selfIsMember = lobby.members?.some((member) => {
            return member.publicId === profilePublicId;
          });

          return (
            friendIsMember &&
            lobby.id !== activeLobbyId &&
            !selfIsMember
          );
        });

      return (
        <FriendCard
          canInviteParty={Boolean(partyInviteEnabled) && !friendIsInActiveLobby}
          canJoinParty={canJoinParty}
          folders={folders}
          friend={friend}
          isDragging={dragState?.active && dragState.friendId === friend.id}
          key={friend.id}
          menuOpen={openMenuFriendId === friend.id}
          t={t}
          onChat={handleChat}
          onDragPointerDown={handleFriendPointerDown}
          onInviteParty={handleInviteParty}
          onJoinParty={handleJoinParty}
          onMenuToggle={(friendId) =>
            setOpenMenuFriendId((currentFriendId) => {
              setOpenMenuFolderId(undefined);
              setFriendTooltip(undefined);
              return currentFriendId === friendId ? undefined : friendId;
            })
          }
          onMoveToFolder={moveFriendToFolder}
          onTooltipHide={() => setFriendTooltip(undefined)}
          onTooltipShow={(friendId, element) => {
            if (dragStateRef.current?.active || openMenuFriendId === friendId) {
              return;
            }

            const rect = element.getBoundingClientRect();
            const tooltipHeight = 136;
            const viewportPadding = 12;
            setFriendTooltip({
              friendId,
              left: rect.right + 14,
              top: Math.max(
                tooltipHeight / 2 + viewportPadding,
                Math.min(
                  window.innerHeight - tooltipHeight / 2 - viewportPadding,
                  rect.top + rect.height / 2,
                ),
              ),
            });
          }}
          onUnfriend={handleUnfriend}
        />
      );
    });
  }

  const overlays = overlayRoot
    ? createPortal(
        <>
          {dragState?.active && draggingFriend ? (
            <>
              {dragState.overFolderId ? (
                <div
                  className="friend-drag-cue"
                  style={{
                    left: dragState.x + 10,
                    top: dragState.y - 10,
                  }}
                >
                  <FolderPlus size={16} />
                </div>
              ) : null}
              <div
                className="friend-drag-preview"
                style={{
                  left: dragState.x + 22,
                  top: dragState.y + 12,
                }}
              >
                <span className="friend-drag-avatar" aria-hidden="true">
                  {getProfileInitials(draggingFriend.name)}
                  {draggingFriend.avatarUrl ? (
                    <img
                      alt=""
                      className="friend-avatar-image"
                      referrerPolicy="no-referrer"
                      src={draggingFriend.avatarUrl}
                      onError={(event) => {
                        event.currentTarget.hidden = true;
                      }}
                    />
                  ) : null}
                </span>
                <span>{draggingFriend.name}</span>
              </div>
            </>
          ) : null}

          {tooltipFriend && friendTooltip && !dragState?.active ? (
            <div
              className={`friend-tooltip rank-frame-${tooltipFriend.rank.name}`}
              role="tooltip"
              style={{
                left: friendTooltip.left,
                top: friendTooltip.top,
              }}
            >
              <div className="friend-tooltip-banner" />
              <div className="friend-tooltip-body">
                <div className="friend-tooltip-avatar" aria-hidden="true">
                  {getProfileInitials(tooltipFriend.name)}
                  {tooltipFriend.avatarUrl ? (
                    <img
                      alt=""
                      className="friend-avatar-image"
                      referrerPolicy="no-referrer"
                      src={tooltipFriend.avatarUrl}
                      onError={(event) => {
                        event.currentTarget.hidden = true;
                      }}
                    />
                  ) : null}
                </div>
                <div className="friend-tooltip-content">
                  <p
                    className={`friend-tooltip-status presence-text-${tooltipFriend.status}`}
                  >
	                    {t(presenceMessageIds[tooltipFriend.status])}
	                    {tooltipFriend.gameMode
	                      ? ` · ${tooltipFriend.gameMode}`
	                      : ""}
                  </p>
                  <p className="friend-tooltip-name">{tooltipFriend.name}</p>
                  <div className="friend-rank-row">
                    <span className={`rank-emblem rank-${tooltipFriend.rank.name}`}>
                      {tooltipFriend.rank.tier}
                    </span>
                    <span>
                      {tooltipFriend.rank.label} {tooltipFriend.rank.tier}
                    </span>
                  </div>
                  {tooltipFriend.status === "ingame" && tooltipFriend.champion ? (
                    <p className="friend-tooltip-champion">
                      {tooltipFriend.champion}
                    </p>
                  ) : null}
                </div>
              </div>
            </div>
          ) : null}

          {friendAddOpen ? (
            <div
              className="dialog-backdrop friend-add-dialog-backdrop"
              role="presentation"
              onMouseDown={() => setFriendAddOpen(false)}
            >
              <section
                aria-labelledby="friend-add-dialog-title"
                aria-modal="true"
                className="friend-add-dialog"
                role="dialog"
                onMouseDown={(event) => event.stopPropagation()}
              >
                <div className="friend-add-dialog-header">
                  <h2 id="friend-add-dialog-title">{t("friend-add")}</h2>
                  <button
                    aria-label={t("settings-close")}
                    className="friend-add-close-button"
                    type="button"
                    onClick={() => setFriendAddOpen(false)}
                  >
                    <X size={18} />
                  </button>
                </div>

                <div className="friend-add-tabs" role="tablist">
                  <button
                    aria-selected={friendAddTab === "add"}
                    className={friendAddTab === "add" ? "active" : ""}
                    role="tab"
                    type="button"
                    onClick={() => setFriendAddTab("add")}
                  >
                    {t("friend-add-tab")}
                  </button>
                  <button
                    aria-selected={friendAddTab === "incoming"}
                    className={friendAddTab === "incoming" ? "active" : ""}
                    role="tab"
                    type="button"
                    onClick={() => setFriendAddTab("incoming")}
                  >
                    {t("friend-incoming-tab")}
                    {incomingFriendRequests.length > 0 ? (
                      <span>{incomingFriendRequests.length}</span>
                    ) : null}
                  </button>
                  <button
                    aria-selected={friendAddTab === "outgoing"}
                    className={friendAddTab === "outgoing" ? "active" : ""}
                    role="tab"
                    type="button"
                    onClick={() => setFriendAddTab("outgoing")}
                  >
                    {t("friend-outgoing-tab")}
                    {outgoingFriendRequests.length > 0 ? (
                      <span>{outgoingFriendRequests.length}</span>
                    ) : null}
                  </button>
                </div>

                <div className="friend-add-body">
                  {friendAddTab === "add" ? (
                    <>
                      <label className="friend-add-search">
                        <Search size={16} />
                        <input
                          aria-label={t("friend-add-search")}
                          placeholder={t("friend-add-search")}
                          ref={friendAddSearchInputRef}
                          value={friendAddSearch}
                          onChange={(event) =>
                            setFriendAddSearch(event.target.value)
                          }
                        />
                        {friendAddSearching ? (
                          <span>{t("friend-add-searching")}</span>
                        ) : null}
                      </label>

                      <div className="friend-add-list">
                        {friendSearchResults.length > 0 ? (
                          friendSearchResults.map((user) => {
                            const alreadyFriend =
                              typeof user.publicId === "number" &&
                              friendPublicIds.has(user.publicId);
                            const alreadyRequested =
                              typeof user.publicId === "number" &&
                              outgoingFriendPublicIds.has(user.publicId);
                            const canRequest =
                              typeof user.publicId === "number" &&
                              !alreadyFriend &&
                              !alreadyRequested;

                            return (
                              <div
                                className="friend-add-row"
                                key={getFriendUserId(user)}
                              >
                                {renderFriendUserAvatar(user)}
                                <span className="friend-add-row-copy">
                                  <span>{getFriendUserName(user)}</span>
                                  <span>{getFriendUserSubtitle(user)}</span>
                                </span>
                                <button
                                  className="friend-add-action-button"
                                  disabled={
                                    !canRequest ||
                                    friendActionBusyId === user.publicId
                                  }
                                  type="button"
                                  onClick={() =>
                                    void handleSendFriendRequest(user.publicId)
                                  }
                                >
                                  {alreadyFriend
                                    ? t("friend-add-already-friend")
                                    : alreadyRequested
                                      ? t("friend-request-pending")
                                      : t("friend-request-send")}
                                </button>
                              </div>
                            );
                          })
                        ) : (
                          <p className="friend-add-empty">
                            {friendAddSearch.trim().length >= 2
                              ? t("friend-add-no-results")
                              : t("friend-add-search-empty")}
                          </p>
                        )}
                      </div>
                    </>
                  ) : null}

                  {friendAddTab === "incoming" ? (
                    <div className="friend-add-list friend-add-list-only">
                      {incomingFriendRequests.length > 0 ? (
                        incomingFriendRequests.map((request) => {
                          const requestUser = getRequestUser(request, "incoming");

                          return (
                            <div className="friend-add-row" key={request.id}>
                              {renderFriendUserAvatar(requestUser)}
                              <span className="friend-add-row-copy">
                                <span>{getFriendUserName(requestUser ?? {})}</span>
                                <span>
                                  {getFriendUserSubtitle(requestUser ?? {})}
                                </span>
                              </span>
                              <span className="friend-add-row-actions">
                                <button
                                  aria-label={t("friend-request-accept")}
                                  disabled={friendRequestBusyId === request.id}
                                  type="button"
                                  onClick={() =>
                                    void handleAcceptRequest(request.id)
                                  }
                                >
                                  <Check size={15} />
                                </button>
                                <button
                                  aria-label={t("friend-request-decline")}
                                  disabled={friendRequestBusyId === request.id}
                                  type="button"
                                  onClick={() =>
                                    void handleDeclineRequest(request.id)
                                  }
                                >
                                  <X size={15} />
                                </button>
                              </span>
                            </div>
                          );
                        })
                      ) : (
                        <p className="friend-add-empty">
                          {t("friend-requests-empty")}
                        </p>
                      )}
                    </div>
                  ) : null}

                  {friendAddTab === "outgoing" ? (
                    <div className="friend-add-list friend-add-list-only">
                      {outgoingFriendRequests.length > 0 ? (
                        outgoingFriendRequests.map((request) => {
                          const requestUser = getRequestUser(request, "outgoing");

                          return (
                            <div className="friend-add-row" key={request.id}>
                              {renderFriendUserAvatar(requestUser)}
                              <span className="friend-add-row-copy">
                                <span>{getFriendUserName(requestUser ?? {})}</span>
                                <span>
                                  {getFriendUserSubtitle(requestUser ?? {})}
                                </span>
                              </span>
                              <button
                                className="friend-add-action-button"
                                disabled={friendRequestBusyId === request.id}
                                type="button"
                                onClick={() => void handleRevokeRequest(request.id)}
                              >
                                {t("friend-request-revoke")}
                              </button>
                            </div>
                          );
                        })
                      ) : (
                        <p className="friend-add-empty">
                          {t("friend-requests-empty")}
                        </p>
                      )}
                    </div>
                  ) : null}
                </div>
              </section>
            </div>
          ) : null}

          {folderCreateOpen ? (
            <div
              className="dialog-backdrop folder-dialog-backdrop"
              role="presentation"
              onMouseDown={() => setFolderCreateOpen(false)}
            >
              <form
                aria-labelledby="folder-dialog-title"
                aria-modal="true"
                className="folder-dialog"
                role="dialog"
                onMouseDown={(event) => event.stopPropagation()}
                onSubmit={(event) => {
                  event.preventDefault();
                  commitCreateFolder();
                }}
              >
                <h2 id="folder-dialog-title">
                  {t("friend-folder-create-title")}
                </h2>
                <input
                  aria-label={t("friend-folder-name")}
                  ref={createFolderInputRef}
                  value={folderCreateInput}
                  onChange={(event) => setFolderCreateInput(event.target.value)}
                />
                <div className="folder-dialog-actions">
                  <button
                    className="secondary-button"
                    type="button"
                    onClick={() => setFolderCreateOpen(false)}
                  >
                    {t("friend-folder-cancel")}
                  </button>
                  <button className="login-button" type="submit">
                    {t("friend-folder-create")}
                  </button>
                </div>
              </form>
            </div>
          ) : null}
        </>,
        overlayRoot,
      )
    : null;

  return (
    <>
    <aside className={sidebarCollapsed ? "app-sidebar collapsed" : "app-sidebar"}>
      <button
        aria-label={sidebarToggleLabel}
        className="sidebar-collapse-button"
        title={sidebarToggleLabel}
        type="button"
        onClick={() => {
          setFriendTooltip(undefined);
          setOpenMenuFolderId(undefined);
          setOpenMenuFriendId(undefined);
          setNotificationsOpen(false);
          setProfileMenuOpen(false);
          setSidebarCollapsed((collapsed) => !collapsed);
        }}
      >
        {sidebarCollapsed ? <ChevronRight size={18} /> : <ChevronLeft size={18} />}
      </button>

      <div className="sidebar-user-card">
        <div className="sidebar-profile-area" ref={profileMenuRef}>
          <button
            aria-expanded={sidebarCollapsed ? profileMenuOpen : undefined}
            aria-label={sidebarCollapsed ? t("profile-menu-open") : profileName}
            className="user-avatar user-avatar-button"
            type="button"
            onClick={() => {
              if (!sidebarCollapsed) {
                return;
              }

              setNotificationsOpen(false);
              setProfileMenuOpen((open) => !open);
            }}
          >
            {getProfileInitials(profileName)}
            {profileAvatarUrl ? (
              <img
                alt=""
                className="user-avatar-image"
                src={profileAvatarUrl}
                onError={(event) => {
                  event.currentTarget.hidden = true;
                }}
              />
            ) : null}
            <span
              className={`presence-dot presence-${presenceStatus}`}
              title={t(presenceMessageIds[presenceStatus])}
            />
          </button>

          {sidebarCollapsed && profileMenuOpen ? (
            <div className="sidebar-profile-menu" role="menu">
              <button
                type="button"
                role="menuitem"
                onClick={() => {
                  setProfileMenuOpen(false);
                  setNotificationsOpen(true);
                }}
              >
                <span>{t("profile-menu-notification")}</span>
                {notificationCount > 0 ? (
                  <span className="profile-menu-badge">
                    {notificationCount > 99 ? "99+" : notificationCount}
                  </span>
                ) : null}
              </button>
              <button
                type="button"
                role="menuitem"
                onClick={() => setProfileMenuOpen(false)}
              >
                {t("profile-menu-profile")}
              </button>
            </div>
          ) : null}

          {sidebarCollapsed && notificationsOpen ? renderNotificationMenu() : null}
        </div>

        <div className="sidebar-user-copy">
          <p className="sidebar-user-name">{profileName}</p>
          <p className={`sidebar-user-status presence-text-${presenceStatus}`}>
            {t(presenceMessageIds[presenceStatus])}
          </p>
        </div>

        <div className="sidebar-notification-area" ref={notificationMenuRef}>
          <button
            aria-expanded={notificationsOpen}
            aria-label={t("notification-open")}
            className="sidebar-notification-button"
            title={t("notification-open")}
            type="button"
            onClick={() => setNotificationsOpen((open) => !open)}
          >
            <Bell size={16} />
            {notificationCount > 0 ? (
              <span className="notification-badge">
                {notificationCount > 99 ? "99+" : notificationCount}
              </span>
            ) : null}
          </button>

          {!sidebarCollapsed && notificationsOpen ? renderNotificationMenu() : null}
        </div>
      </div>

      <div className="sidebar-divider" />

      <nav className="sidebar-toolbar" aria-label="Sidebar tools">
        <button
          aria-label="Your Friends"
          className={activeSidebarTab === "friends" ? "active" : ""}
          title={t("sidebar-friends")}
          type="button"
          onClick={() => setActiveSidebarTab("friends")}
        >
          <Users size={18} />
          {friendRequestCount > 0 ? (
            <span className="friend-request-badge">{friendRequestCount}</span>
          ) : null}
        </button>
        <button
          aria-label="Your Teams"
          className={activeSidebarTab === "teams" ? "active" : ""}
          title={t("sidebar-teams")}
          type="button"
          onClick={() => setActiveSidebarTab("teams")}
        >
          <Shield size={18} />
        </button>
        <button
          aria-label="Tournaments"
          className={activeSidebarTab === "tournaments" ? "active" : ""}
          title={t("sidebar-tournaments")}
          type="button"
          onClick={() => setActiveSidebarTab("tournaments")}
        >
          <Trophy size={18} />
        </button>
      </nav>

      <div className="sidebar-divider" />

      {activeSidebarTab === "friends" ? (
        <div className="friend-panel" aria-label="Your Friends">
          <div className="friend-list-tools">
            <label className="friend-search">
              <Search size={16} />
              <input
                aria-label={t("friend-search")}
                placeholder={t("friend-search")}
                value={friendSearch}
                onChange={(event) => setFriendSearch(event.target.value)}
              />
            </label>
            <button
              aria-label={t("friend-add")}
              className="friend-tool-button friend-add-button"
              title={t("friend-add")}
              type="button"
              onClick={() => {
                setFriendApiError(undefined);
                setFriendAddTab("add");
                setFriendAddOpen(true);
                void refreshLiveData();
              }}
            >
              <UserPlus size={17} />
              {friendRequestCount > 0 ? (
                <span className="friend-request-badge">{friendRequestCount}</span>
              ) : null}
            </button>
            <button
              aria-label={t("friend-folder-add")}
              className="friend-tool-button"
              title={t("friend-folder-add")}
              type="button"
              onClick={handleCreateFolder}
            >
              <FolderPlus size={17} />
            </button>
          </div>

          <div className="friend-groups">
            <section className="friend-folder-section">
              <div className="friend-folder-heading">
                <span>{t("sidebar-friends")}</span>
                <span>{visibleFriends.length}</span>
              </div>
              <div className="friend-list">{renderFriendCards(unfiledFriends)}</div>
            </section>

            {folders.map((folder) => {
              const folderFriends = visibleFriends.filter(
                (friend) => friend.folderId === folder.id,
              );
              const folderIsDropTarget = dragState?.overFolderId === folder.id;

              return (
                <section
                  className="friend-folder-section"
                  data-folder-drop-id={folder.id}
                  key={folder.id}
                >
                  <div
                    className={
                      folderIsDropTarget
                        ? "friend-folder-row drag-over"
                        : "friend-folder-row"
                    }
                  >
                    {renamingFolderId === folder.id ? (
                      <div className="friend-folder-toggle friend-folder-rename-row">
                        {folder.open ? (
                          <ChevronDown size={15} />
                        ) : (
                          <ChevronRight size={15} />
                        )}
                        {folder.open ? (
                          <FolderOpen size={16} />
                        ) : (
                          <Folder size={16} />
                        )}
                      <input
                        className="friend-folder-rename-input"
                        ref={renameInputRef}
                        value={folderRenameInput}
                        onBlur={commitRenameFolder}
                        onChange={(event) =>
                          setFolderRenameInput(event.target.value)
                        }
                        onClick={(event) => event.stopPropagation()}
                        onKeyDown={handleRenameKeyDown}
                        onPointerDown={(event) => event.stopPropagation()}
                      />
                        <span>{folderFriends.length}</span>
                      </div>
                    ) : (
                      <button
                        className="friend-folder-toggle"
                        type="button"
                        onClick={() => toggleFolder(folder.id)}
                        onDoubleClick={(event) => {
                          event.stopPropagation();
                          startRenameFolder(folder.id);
                        }}
                      >
                        {folder.open ? (
                          <ChevronDown size={15} />
                        ) : (
                          <ChevronRight size={15} />
                        )}
                        {folder.open ? (
                          <FolderOpen size={16} />
                        ) : (
                          <Folder size={16} />
                        )}
                        <span>{folder.name}</span>
                        <span>{folderFriends.length}</span>
                      </button>
                    )}

                    <button
                      aria-expanded={openMenuFolderId === folder.id}
                      aria-label={t("friend-folder-actions")}
                      className="friend-folder-menu-button"
                      type="button"
                      onClick={(event) => {
                        event.stopPropagation();
                        setOpenMenuFriendId(undefined);
                        setOpenMenuFolderId((currentFolderId) =>
                          currentFolderId === folder.id ? undefined : folder.id,
                        );
                      }}
                    >
                      <MoreHorizontal size={16} />
                    </button>

                    {openMenuFolderId === folder.id ? (
                      <div
                        className="friend-context-menu folder-context-menu"
                        role="menu"
                        onClick={(event) => event.stopPropagation()}
                        onPointerDown={(event) => event.stopPropagation()}
                      >
                        <button
                          type="button"
                          role="menuitem"
                          onClick={() => startRenameFolder(folder.id)}
                        >
                          <Pencil size={15} />
                          <span>{t("friend-folder-rename")}</span>
                        </button>
                        <button
                          className="danger"
                          type="button"
                          role="menuitem"
                          onClick={() => handleDeleteFolder(folder.id)}
                        >
                          <Trash2 size={15} />
                          <span>{t("friend-folder-delete")}</span>
                        </button>
                      </div>
                    ) : null}
                  </div>

                  {folder.open ? (
                    <div className="friend-list">
                      {renderFriendCards(folderFriends)}
                    </div>
                  ) : null}
                </section>
              );
            })}
          </div>

        </div>
      ) : null}
    </aside>
    {overlays}
    </>
  );
}

export default Sidebar;
