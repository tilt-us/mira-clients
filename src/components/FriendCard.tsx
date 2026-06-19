import type { FriendProfile, Translate } from "../types/ui";
import { presenceMessageIds } from "../types/ui";
import { getProfileInitials } from "../utils/profile";

type FriendCardProps = {
  friend: FriendProfile;
  t: Translate;
};

function FriendCard({ friend, t }: FriendCardProps) {
  const initials = getProfileInitials(friend.name);
  const presenceLabel = t(presenceMessageIds[friend.status]);

  return (
    <article className={`friend-card rank-frame-${friend.rank.name}`}>
      <div className="friend-card-avatar" aria-hidden="true">
        {initials}
        <span
          className={`friend-presence-dot presence-${friend.status}`}
          title={presenceLabel}
        />
      </div>

      <div className="friend-card-copy">
        <p className="friend-card-name">{friend.name}</p>
        <p className={`friend-card-status presence-text-${friend.status}`}>
          {presenceLabel}
          {friend.status === "ingame" && friend.gameMode ? ` · ${friend.gameMode}` : ""}
        </p>
      </div>

      <div className="friend-tooltip" role="tooltip">
        <div className="friend-tooltip-banner" />
        <div className="friend-tooltip-body">
          <div className="friend-tooltip-avatar" aria-hidden="true">
            {initials}
          </div>
          <div className="friend-tooltip-content">
            <p className={`friend-tooltip-status presence-text-${friend.status}`}>
              {presenceLabel}
              {friend.status === "ingame" && friend.gameMode
                ? ` · ${friend.gameMode}`
                : ""}
            </p>
            <p className="friend-tooltip-name">{friend.name}</p>
            <div className="friend-rank-row">
              <span className={`rank-emblem rank-${friend.rank.name}`}>
                {friend.rank.tier}
              </span>
              <span>
                {friend.rank.label} {friend.rank.tier}
              </span>
            </div>
            {friend.status === "ingame" && friend.champion ? (
              <p className="friend-tooltip-champion">{friend.champion}</p>
            ) : null}
          </div>
        </div>
      </div>
    </article>
  );
}

export default FriendCard;
