import {
  Folder,
  Eye,
  LogIn,
  MessageCircle,
  MoreHorizontal,
  Send,
  UserMinus,
} from "lucide-react";
import type { PointerEvent } from "react";
import type { FriendFolder, FriendProfile, Translate } from "../types/ui";
import { presenceMessageIds } from "../types/ui";
import { getProfileInitials } from "../utils/profile";

type FriendCardProps = {
  folders: FriendFolder[];
  canInviteParty: boolean;
  canJoinParty: boolean;
  friend: FriendProfile;
  isDragging?: boolean;
  menuOpen: boolean;
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

function FriendCard({
  folders,
  canInviteParty,
  canJoinParty,
  friend,
  isDragging,
  menuOpen,
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

  return (
    <article
      className={`friend-card rank-frame-${friend.rank.name}${
        menuOpen ? " menu-open" : ""
      }${isDragging ? " dragging" : ""}`}
      onDoubleClick={() => onChat(friend.id)}
      onMouseEnter={(event) => onTooltipShow(friend.id, event.currentTarget)}
      onMouseLeave={onTooltipHide}
      onPointerDown={(event) => onDragPointerDown(friend.id, event)}
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
        <p className="friend-card-name">{friend.name}</p>
        <p className={`friend-card-status presence-text-${friend.status}`}>
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
              <button type="button" role="menuitem" onClick={() => onChat(friend.id)}>
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
              <p className="friend-context-label">
                <Folder size={14} />
                <span>{t("friend-move-to")}</span>
              </p>

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
            </>
          )}
        </div>
      ) : null}

    </article>
  );
}

export default FriendCard;
