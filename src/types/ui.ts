export type PresenceStatus = "online" | "afk" | "ingame" | "no-connection";
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

export type FriendProfile = {
  name: string;
  status: PresenceStatus;
  gameMode?: "Ranked" | "Unranked";
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
  ingame: "presence-ingame",
  "no-connection": "presence-no-connection",
};
