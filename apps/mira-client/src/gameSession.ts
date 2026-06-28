import type {
  ApiMatchResponse,
  MatchLobbyResponse,
  MatchPlayerResponse,
} from "./api/client";
import { MATCHMAKING_API_BASE_URL } from "./api/config";
import { readTokens } from "./auth/storage";
import type { GameScreenMode } from "./settings";
import { getPublicDisplayName } from "./utils/profile";

export type GameTeam = "dark" | "light";

export type LaunchGameRequest = {
  accessToken: string;
  accentColor: string;
  champion: string;
  forceRestart?: boolean;
  matchManifestJson: string;
  matchId: string;
  matchmakingApiBaseUrl: string;
  playerPublicId: number;
  serverHost: string;
  serverControlBaseUrl: string;
  port: number;
  screen: GameScreenMode;
  team: GameTeam;
};

export type GameLaunchParameters = Omit<LaunchGameRequest, "accessToken" | "accentColor">;

type GameMatchManifestPlayer = {
  avatarUrl?: string;
  champion: string;
  championId: number;
  displayName?: string;
  playerPublicId: number;
  team: "Dark" | "Light";
};

type GameMatchManifest = {
  matchId: string;
  players: GameMatchManifestPlayer[];
};

type GameServerAddressOverrides = {
  publicControlBaseUrl?: string;
  publicControlHost?: string;
  publicHost?: string;
  publicPort?: number;
  gamePort?: number;
  serverPort?: number;
  udpPort?: number;
};

export type StoredGameSession = {
  closedByClient?: boolean;
  parameters: GameLaunchParameters;
  playerPublicId?: number;
};

export type GameClientStatus = {
  running: boolean;
  pid?: number;
};

const storedGameSessionKey = "mira:last-game-session";

export function getMatchChampionForPlayer(
  match: ApiMatchResponse,
  playerPublicId: number,
) {
  return match.championSelections?.find((selection) => {
    return selection.playerPublicId === playerPublicId;
  })?.champion;
}

export function getGameClientChampionId(champion: string) {
  return champion.trim().toLowerCase();
}

function getGameServerChampionId(champion: string) {
  switch (getGameClientChampionId(champion)) {
    case "lira":
      return 6606;
    case "ignara":
      return 6607;
    case "sophia":
      return 6608;
    case "yuna":
      return 6609;
    default:
      return 6606;
  }
}

export function createGameMatchManifest(
  match: ApiMatchResponse,
  matchId: string,
): GameMatchManifest {
  const teams = getMatchTeams(match);
  const players: GameMatchManifestPlayer[] = [];

  teams.forEach((team, teamIndex) => {
    const teamName = teamIndex === 0 ? "Dark" : "Light";

    for (const player of team.players ?? []) {
      if (typeof player.publicId !== "number") {
        continue;
      }

      const champion = getMatchChampionForPlayer(match, player.publicId);

      if (!champion) {
        continue;
      }

      const displayName = getPublicDisplayName(player.displayName, "");

      players.push({
        ...(player.avatarUrl ? { avatarUrl: player.avatarUrl } : {}),
        champion,
        championId: getGameServerChampionId(champion),
        ...(displayName ? { displayName } : {}),
        playerPublicId: player.publicId,
        team: teamName,
      });
    }
  });

  return {
    matchId,
    players,
  };
}

export function getMatchPort(match: ApiMatchResponse) {
  const gameServer = match.gameServer as
    | (NonNullable<ApiMatchResponse["gameServer"]> & GameServerAddressOverrides)
    | undefined;
  const explicitPort =
    gameServer?.publicPort ??
    gameServer?.gamePort ??
    gameServer?.serverPort ??
    gameServer?.udpPort ??
    gameServer?.port;

  if (typeof explicitPort === "number") {
    return explicitPort;
  }

  if (typeof gameServer?.controlPort === "number" && gameServer.controlPort > 1000) {
    return gameServer.controlPort - 1000;
  }

  return undefined;
}

export function getMatchHost(match: ApiMatchResponse) {
  const gameServer = match.gameServer as
    | (NonNullable<ApiMatchResponse["gameServer"]> & GameServerAddressOverrides)
    | undefined;

  return resolvePublishedGameServerHost(gameServer?.publicHost ?? gameServer?.host);
}

export function getMatchControlBaseUrl(match: ApiMatchResponse) {
  const gameServer = match.gameServer as
    | (NonNullable<ApiMatchResponse["gameServer"]> & GameServerAddressOverrides)
    | undefined;
  const explicitBaseUrl = gameServer?.publicControlBaseUrl ?? gameServer?.controlBaseUrl;

  if (explicitBaseUrl) {
    return resolvePublishedGameServerBaseUrl(explicitBaseUrl);
  }

  const controlHost = gameServer?.publicControlHost ?? gameServer?.controlHost;
  const controlPort = gameServer?.controlPort;

  if (!controlHost || typeof controlPort !== "number") {
    return undefined;
  }

  const protocol = gameServer?.controlProtocol ?? "http";
  const publishedControlHost = resolvePublishedGameServerHost(controlHost);

  return publishedControlHost ? `${protocol}://${publishedControlHost}:${controlPort}` : undefined;
}

function resolvePublishedGameServerHost(host?: string) {
  if (!host) {
    return undefined;
  }

  const fallbackHost = getRemoteMatchmakingHost();

  if (!fallbackHost || !isPrivatePublishedHost(host)) {
    return host;
  }

  return fallbackHost;
}

function resolvePublishedGameServerBaseUrl(baseUrl: string) {
  try {
    const parsedUrl = new URL(baseUrl);
    const fallbackHost = getRemoteMatchmakingHost();

    if (fallbackHost && isPrivatePublishedHost(parsedUrl.hostname)) {
      parsedUrl.hostname = fallbackHost;
    }

    return parsedUrl.toString().replace(/\/$/, "");
  } catch {
    return baseUrl;
  }
}

function getRemoteMatchmakingHost() {
  try {
    const host = new URL(MATCHMAKING_API_BASE_URL).hostname;
    return isPrivatePublishedHost(host) ? undefined : host;
  } catch {
    return undefined;
  }
}

function isPrivatePublishedHost(host: string) {
  const normalizedHost = host.trim().toLowerCase().replace(/^\[|\]$/g, "");

  return (
    normalizedHost === "localhost" ||
    normalizedHost === "0.0.0.0" ||
    normalizedHost === "::" ||
    normalizedHost === "::1" ||
    normalizedHost === "0:0:0:0:0:0:0:1" ||
    normalizedHost.startsWith("127.")
  );
}

function hashString(value: string) {
  let hash = 0;

  for (let index = 0; index < value.length; index += 1) {
    hash = Math.imul(31, hash) + value.charCodeAt(index);
    hash |= 0;
  }

  return Math.abs(hash);
}

function getMatchSeed(match: ApiMatchResponse) {
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

function getMatchTeams(match: ApiMatchResponse): MatchLobbyResponse[] {
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

export function getMatchTeamForPlayer(
  match: ApiMatchResponse,
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

export function readStoredGameSession() {
  try {
    const rawSession = window.localStorage.getItem(storedGameSessionKey);

    if (!rawSession) {
      return undefined;
    }

    const session = JSON.parse(rawSession) as StoredGameSession;

    return session?.parameters?.matchId ? session : undefined;
  } catch {
    return undefined;
  }
}

export function writeStoredGameSession(session: StoredGameSession) {
  window.localStorage.setItem(storedGameSessionKey, JSON.stringify(session));
}

export function clearStoredGameSession() {
  window.localStorage.removeItem(storedGameSessionKey);
}

export function sendCancelChampionPhaseKeepalive(matchId: string) {
  const accessToken = readTokens()?.accessToken;

  void fetch(`${MATCHMAKING_API_BASE_URL}/api/matches/${matchId}/champion-phase`, {
    headers: {
      ...(accessToken ? { Authorization: `Bearer ${accessToken}` } : {}),
    },
    keepalive: true,
    method: "DELETE",
  }).catch(() => undefined);
}
