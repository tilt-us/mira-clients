import {
  ChevronRight,
  Folder,
  Eye,
  LogIn,
  MessageCircle,
  MoreHorizontal,
  Send,
  UserMinus,
} from "lucide-react";
import {
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type PointerEvent,
} from "react";
import { createPortal } from "react-dom";
import type { FriendFolder, FriendProfile, Translate } from "../types/ui";
import { presenceMessageIds } from "../types/ui";
import { formatTagId, getProfileInitials } from "../utils/profile";

type FriendCardProps = {
  folders: FriendFolder[];
  canInviteParty: boolean;
  canJoinParty: boolean;
  friend: FriendProfile;
  isDragging?: boolean;
  menuOpen: boolean;
  sidebarCollapsed?: boolean;
  queueActionsLocked?: boolean;
  timedPresenceLabel?: string;
  onChat: (friendId: string) => void;
  onDragPointerDown: (
    friendId: string,
    event: PointerEvent<HTMLElement>,
  ) => void;
  onInviteParty: (friendId: string) => void;
  onJoinParty: (friendId: string) => void;
  onMenuToggle: (friendId: string) => void;
  onMoveToFolder: (friendId: string, folderId: string) => void;
  onTooltipHide: () => void;
  onTooltipShow: (friendId: string, element: HTMLElement) => void;
  onUnfriend: (friendId: string) => void;
  onViewProfile: (friendId: string) => void;
  t: Translate;
};

type MoveSubmenuPosition = {
  left: number;
  maxHeight: number;
  top: number;
};

function getFriendCardNameClassName(name: string) {
  const nameLength = Array.from(name.trim()).length;

  if (nameLength > 20) {
    return "friend-card-name friend-card-name-overflow";
  }

  if (nameLength > 14) {
    return "friend-card-name friend-card-name-condensed";
  }

  if (nameLength > 10) {
    return "friend-card-name friend-card-name-compact";
  }

  return "friend-card-name";
}

function FriendCard({
  folders,
  canInviteParty,
  canJoinParty,
  friend,
  isDragging,
  menuOpen,
  sidebarCollapsed = false,
  queueActionsLocked = false,
  timedPresenceLabel,
  onChat,
  onDragPointerDown,
  onInviteParty,
  onJoinParty,
  onMenuToggle,
  onMoveToFolder,
  onTooltipHide,
  onTooltipShow,
  onUnfriend,
  onViewProfile,
  t,
}: FriendCardProps) {
  const initials = getProfileInitials(friend.name);
  const presenceLabel = t(presenceMessageIds[friend.status]);
  const friendTagId = formatTagId(friend.tagId);
  const closeMoveSubmenuTimeoutRef = useRef<number | undefined>(undefined);
  const [moveSubmenuPosition, setMoveSubmenuPosition] =
    useState<MoveSubmenuPosition>();
  const moveSubmenuStyle: CSSProperties | undefined = moveSubmenuPosition
    ? {
        left: moveSubmenuPosition.left,
        maxHeight: moveSubmenuPosition.maxHeight,
        top: moveSubmenuPosition.top,
      }
    : undefined;

  function clearMoveSubmenuCloseTimeout() {
    if (closeMoveSubmenuTimeoutRef.current === undefined) {
      return;
    }

    window.clearTimeout(closeMoveSubmenuTimeoutRef.current);
    closeMoveSubmenuTimeoutRef.current = undefined;
  }

  function openMoveSubmenu(element: HTMLElement) {
    clearMoveSubmenuCloseTimeout();

    const rect = element.getBoundingClientRect();
    const panelWidth = 190;
    const panelHeight = folders.length > 5 ? 184 : Math.max(44, folders.length * 35 + 12);
    const viewportPadding = 8;
    const rightSideLeft = rect.right - 1;
    const leftSideLeft = rect.left - panelWidth + 1;
    const left =
      rightSideLeft + panelWidth > window.innerWidth - viewportPadding
        ? Math.max(viewportPadding, leftSideLeft)
        : rightSideLeft;
    const top = Math.min(
      Math.max(viewportPadding, rect.top - 6),
      Math.max(viewportPadding, window.innerHeight - panelHeight - viewportPadding),
    );

    setMoveSubmenuPosition({
      left,
      maxHeight: Math.min(184, window.innerHeight - top - viewportPadding),
      top,
    });
  }

  function closeMoveSubmenuSoon() {
    clearMoveSubmenuCloseTimeout();
    closeMoveSubmenuTimeoutRef.current = window.setTimeout(() => {
      setMoveSubmenuPosition(undefined);
    }, 120);
  }

  useEffect(() => {
    if (!menuOpen) {
      setMoveSubmenuPosition(undefined);
    }
  }, [menuOpen]);

  useEffect(() => {
    return () => clearMoveSubmenuCloseTimeout();
  }, []);

  function renderMoveSubmenuPanel() {
    if (!moveSubmenuPosition || typeof document === "undefined") {
      return null;
    }

    return createPortal(
      <div
        aria-label={t("friend-move-to")}
        className={`friend-context-submenu-panel fixed${
          folders.length > 5 ? " scrollable" : ""
        }`}
        role="menu"
        style={moveSubmenuStyle}
        onClick={(event) => event.stopPropagation()}
        onMouseEnter={clearMoveSubmenuCloseTimeout}
        onMouseLeave={closeMoveSubmenuSoon}
        onPointerDown={(event) => event.stopPropagation()}
      >
        {folders.length > 0 ? (
          folders.map((folder) => (
            <button
              disabled={friend.folderId === folder.id}
              key={folder.id}
              type="button"
              role="menuitem"
              onClick={() => onMoveToFolder(friend.id, folder.id)}
            >
              <Folder size={15} />
              <span>{folder.name}</span>
            </button>
          ))
        ) : (
          <p className="friend-context-empty">{t("friend-no-folders")}</p>
        )}
      </div>,
      document.body,
    );
  }

  return (
    <article
      className={`friend-card rank-frame-${friend.rank.name}${
        menuOpen ? " menu-open" : ""
      }${isDragging ? " dragging" : ""}${sidebarCollapsed ? " friend-card-collapsed" : ""}`}
      onClick={(event) => {
        if (!sidebarCollapsed) {
          return;
        }

        event.stopPropagation();
        onMenuToggle(friend.id);
      }}
      onContextMenu={(event) => {
        if (!sidebarCollapsed) {
          return;
        }

        event.preventDefault();
        event.stopPropagation();
        onMenuToggle(friend.id);
      }}
      onDoubleClick={() => {
        if (!sidebarCollapsed) {
          onChat(friend.id);
        }
      }}
      onMouseEnter={(event) => onTooltipShow(friend.id, event.currentTarget)}
      onMouseLeave={onTooltipHide}
      onPointerDown={(event) => {
        if (sidebarCollapsed) {
          return;
        }

        onDragPointerDown(friend.id, event);
      }}
    >
      <div className="friend-card-avatar" aria-hidden="true">
        {initials}
        {friend.avatarUrl ? (
          <img
            alt=""
            className="friend-avatar-image"
            referrerPolicy="no-referrer"
            src={friend.avatarUrl}
            onError={(event) => {
              event.currentTarget.hidden = true;
            }}
          />
        ) : null}
        <span
          className={`friend-presence-dot presence-${friend.status}`}
          title={presenceLabel}
        />
      </div>

      <div className="friend-card-copy">
        <p className={getFriendCardNameClassName(friend.name)} title={friend.name}>
          {friend.name}
        </p>
        <p className={`friend-card-status presence-text-${friend.status}`}>
          {friendTagId ? `${friendTagId} · ` : ""}
          {timedPresenceLabel ?? presenceLabel}
          {friend.gameMode ? ` · ${friend.gameMode}` : ""}
        </p>
      </div>

      <button
        aria-expanded={menuOpen}
        aria-label={t("friend-actions")}
        className="friend-card-menu-button"
        type="button"
        onClick={(event) => {
          event.stopPropagation();
          onMenuToggle(friend.id);
        }}
        onPointerDown={(event) => event.stopPropagation()}
      >
        <MoreHorizontal size={17} />
      </button>

      {menuOpen ? (
        <div
          className="friend-context-menu"
          role="menu"
          onClick={(event) => event.stopPropagation()}
          onPointerDown={(event) => event.stopPropagation()}
        >
          <button
            type="button"
            role="menuitem"
            onClick={() => onViewProfile(friend.id)}
          >
            <Eye size={15} />
            <span>{t("friend-view-profile")}</span>
          </button>

          {queueActionsLocked ? null : (
            <>
              <button
                className="friend-context-field"
                type="button"
                role="menuitem"
                onClick={() => onChat(friend.id)}
              >
                <MessageCircle size={15} />
                <span>{t("friend-chat")}</span>
              </button>
              {canJoinParty ? (
                <button
                  type="button"
                  role="menuitem"
                  onClick={() => onJoinParty(friend.id)}
                >
                  <LogIn size={15} />
                  <span>{t("friend-join-party")}</span>
                </button>
              ) : null}
              {canInviteParty ? (
                <button
                  type="button"
                  role="menuitem"
                  onClick={() => onInviteParty(friend.id)}
                >
                  <Send size={15} />
                  <span>{t("friend-invite-party")}</span>
                </button>
              ) : null}
              <button
                className="danger"
                type="button"
                role="menuitem"
                onClick={() => onUnfriend(friend.id)}
              >
                <UserMinus size={15} />
                <span>{t("friend-unfriend")}</span>
              </button>

              <div className="friend-context-divider" />
              <div className="friend-context-submenu">
                <button
                  aria-haspopup="menu"
                  className="friend-context-submenu-trigger"
                  type="button"
                  role="menuitem"
                  onFocus={(event) => openMoveSubmenu(event.currentTarget)}
                  onMouseEnter={(event) => openMoveSubmenu(event.currentTarget)}
                  onMouseLeave={closeMoveSubmenuSoon}
                >
                  <Folder size={15} />
                  <span>{t("friend-move-to")}</span>
                  <ChevronRight size={14} />
                </button>
              </div>
            </>
          )}
        </div>
      ) : null}

      {renderMoveSubmenuPanel()}
    </article>
  );
}

export default FriendCard;
