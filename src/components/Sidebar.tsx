import { Shield, Trophy, Users } from "lucide-react";
import { useState } from "react";
import FriendCard from "./FriendCard";
import type { FriendProfile, PresenceStatus, SidebarTab, Translate } from "../types/ui";
import { presenceMessageIds } from "../types/ui";
import { getProfileInitials } from "../utils/profile";

const friendProfiles: FriendProfile[] = [
  {
    name: "Lina",
    status: "ingame",
    gameMode: "Ranked",
    champion: "Vesper",
    rank: {
      name: "gold",
      label: "Gold",
      tier: "II",
    },
  },
  {
    name: "Noah",
    status: "online",
    rank: {
      name: "diamond",
      label: "Diamond",
      tier: "IV",
    },
  },
];

type SidebarProps = {
  presenceStatus: PresenceStatus;
  profileName: string;
  t: Translate;
};

function Sidebar({ presenceStatus, profileName, t }: SidebarProps) {
  const [activeSidebarTab, setActiveSidebarTab] = useState<SidebarTab>("friends");

  return (
    <aside className="app-sidebar">
      <div className="sidebar-user-card">
        <div className="user-avatar" aria-hidden="true">
          {getProfileInitials(profileName)}
          <span
            className={`presence-dot presence-${presenceStatus}`}
            title={t(presenceMessageIds[presenceStatus])}
          />
        </div>

        <div className="sidebar-user-copy">
          <p className="sidebar-user-name">{profileName}</p>
          <p className={`sidebar-user-status presence-text-${presenceStatus}`}>
            {t(presenceMessageIds[presenceStatus])}
          </p>
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
        <div className="friend-list" aria-label="Your Friends">
          {friendProfiles.map((friend) => (
            <FriendCard friend={friend} key={friend.name} t={t} />
          ))}
        </div>
      ) : null}
    </aside>
  );
}

export default Sidebar;
