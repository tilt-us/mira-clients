export type PresenceStatus =
  | "online"
  | "afk"
  | "inlobby"
  | "inqueue"
  | "championselection"
  | "ingame"
  | "offline";
export type SidebarTab = "friends" | "teams" | "tournaments";
export type SettingsVision = "Vision.Auth" | "Vision.ALL";
export type Translate = (id: string) => string;

export type RankName =
  | "wood"
  | "bronze"
  | "silver"
  | "gold"
  | "ruby"
  | "diamond"
  | "master"
  | "grandmaster"
  | "demonic";

export type RankTier = "I" | "II" | "III" | "IV";

export type FriendFolder = {
  id: string;
  name: string;
  open: boolean;
  moveRule?: FriendFolderMoveRule;
};

export type FriendFolderMoveRule = "new-friend" | "new-tagged" | "none";

export type FriendProfile = {
  avatarUrl?: string;
  email?: string;
  folderId?: string;
  id: string;
  name: string;
  publicId?: number;
  tagId?: string;
  level?: number;
  gameStartedAt?: string;
  queueStartedAt?: string;
  status: PresenceStatus;
  gameMode?: string;
  champion?: string;
  rank: {
    name: RankName;
    label: string;
    tier: RankTier;
  };
};

export const presenceMessageIds: Record<PresenceStatus, string> = {
  online: "presence-online",
  afk: "presence-afk",
  inlobby: "presence-inlobby",
  inqueue: "presence-inqueue",
  championselection: "presence-championselection",
  ingame: "presence-ingame",
  offline: "presence-offline",
};
