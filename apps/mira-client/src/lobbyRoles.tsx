import type { CSSProperties } from "react";
import adcRoleIcon from "../../../assets/icons/roles/adc.png";
import jungleRoleIcon from "../../../assets/icons/roles/jng.png";
import midRoleIcon from "../../../assets/icons/roles/mid-line-full.png";
import midRoleShadowIcon from "../../../assets/icons/roles/mid-shadow-full.png";
import supportRoleIcon from "../../../assets/icons/roles/sup.png";
import topRoleIcon from "../../../assets/icons/roles/top.png";
import type { LobbyMember, LobbySnapshot } from "./api/client";

export type GameMode = "normal" | "ranked";
export type LobbyRoleId = "adc" | "jungle" | "mid" | "support" | "top";
export type ApiLobbyRole = "TOP" | "JUNGLE" | "MID" | "ADC" | "SUPPORT";
export type LobbyMemberWithRoles = LobbyMember & {
  preferredRoles?: unknown[];
  primaryRole?: unknown;
  roles?: unknown[];
  secondaryRole?: unknown;
  selectedRoles?: unknown[];
};
export type LobbyRoleSelection = [LobbyRoleId | undefined, LobbyRoleId | undefined];

export const lobbyRoles: {
  icon: string;
  id: LobbyRoleId;
  labelKey: string;
  shadowIcon?: string;
}[] = [
  { icon: midRoleIcon, id: "mid", labelKey: "lobby-role-mid", shadowIcon: midRoleShadowIcon },
  { icon: jungleRoleIcon, id: "jungle", labelKey: "lobby-role-jungle" },
  { icon: adcRoleIcon, id: "adc", labelKey: "lobby-role-adc" },
  { icon: supportRoleIcon, id: "support", labelKey: "lobby-role-support" },
  { icon: topRoleIcon, id: "top", labelKey: "lobby-role-top" },
];

export function normalizeLobbyRoleId(value: unknown): LobbyRoleId | undefined {
  const normalizedValue = typeof value === "string" ? value.toLowerCase() : "";

  switch (normalizedValue) {
    case "adc":
    case "jungle":
    case "mid":
    case "support":
    case "top":
      return normalizedValue;
    case "jng":
      return "jungle";
    case "sup":
      return "support";
    default:
      return undefined;
  }
}

export function toApiLobbyRole(role: LobbyRoleId): ApiLobbyRole {
  switch (role) {
    case "adc":
      return "ADC";
    case "jungle":
      return "JUNGLE";
    case "mid":
      return "MID";
    case "support":
      return "SUPPORT";
    case "top":
      return "TOP";
  }
}

export function normalizeLobbyRoleSelection(roles: [unknown, unknown]) {
  const primaryRole = normalizeLobbyRoleId(roles[0]);
  const secondaryRole = normalizeLobbyRoleId(roles[1]);

  return [
    primaryRole,
    secondaryRole && secondaryRole !== primaryRole ? secondaryRole : undefined,
  ] satisfies LobbyRoleSelection;
}

function getLobbyRoleMode(roles: LobbyRoleSelection) {
  const apiRoles = roles
    .filter((role): role is LobbyRoleId => Boolean(role))
    .map(toApiLobbyRole);

  return apiRoles.length > 0 ? ` [roles=${apiRoles.join(",")}]` : "";
}

export function getLobbyPresenceMode(gameMode: GameMode, roles: LobbyRoleSelection) {
  const modeLabel = gameMode === "ranked" ? "Ranked" : "Normal";
  return `${modeLabel}${getLobbyRoleMode(roles)}`;
}

export function getLobbyRolesFromPresenceMode(mode: string | undefined) {
  const roleMatch = mode?.match(/\[roles=([^\]]+)\]/i);
  const [primaryRole, secondaryRole] =
    roleMatch?.[1]?.split(",").map(normalizeLobbyRoleId) ?? [];

  return normalizeLobbyRoleSelection([primaryRole, secondaryRole]);
}

export function getMemberLobbyRoles(member?: LobbyMember) {
  const memberWithRoles = member as LobbyMemberWithRoles | undefined;

  if (!memberWithRoles) {
    return [undefined, undefined] as LobbyRoleSelection;
  }

  const selectedRoles = normalizeLobbyRoleSelection([
    memberWithRoles.primaryRole,
    memberWithRoles.secondaryRole,
  ]);

  for (const role of [
    ...(memberWithRoles.roles ?? []),
    ...(memberWithRoles.preferredRoles ?? []),
    ...(memberWithRoles.selectedRoles ?? []),
  ]) {
    const normalizedRole = normalizeLobbyRoleId(role);

    if (!normalizedRole || selectedRoles.includes(normalizedRole)) {
      continue;
    }

    if (!selectedRoles[0]) {
      selectedRoles[0] = normalizedRole;
    } else if (!selectedRoles[1]) {
      selectedRoles[1] = normalizedRole;
    }
  }

  return selectedRoles;
}

export function hasLobbyRoles(roles: LobbyRoleSelection) {
  return Boolean(roles[0] || roles[1]);
}

export function getLobbyRoleLimitError(lobby: LobbySnapshot) {
  const roleCounts = new Map<LobbyRoleId, number>();

  for (const member of lobby.members ?? []) {
    for (const role of getMemberLobbyRoles(member)) {
      if (!role) {
        continue;
      }

      const nextCount = (roleCounts.get(role) ?? 0) + 1;

      if (nextCount >= 3) {
        return "lobby-roles-too-duplicate";
      }

      roleCounts.set(role, nextCount);
    }
  }

  return undefined;
}

export function LobbyRoleIcon({ role }: { role: LobbyRoleId }) {
  const roleConfig = lobbyRoles.find((candidate) => candidate.id === role);

  return (
    <span
      aria-hidden="true"
      className={`lobby-role-icon lobby-role-icon-${role}`}
      style={
        {
          "--role-icon-shadow-url": roleConfig?.shadowIcon
            ? `url(${roleConfig.shadowIcon})`
            : undefined,
          "--role-icon-url": roleConfig?.icon ? `url(${roleConfig.icon})` : undefined,
        } as CSSProperties
      }
    />
  );
}
