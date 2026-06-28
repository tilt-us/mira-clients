import { useEffect, useMemo, useState, type CSSProperties } from "react";
import type {
  ApiMatchResponse,
  MatchLobbyResponse,
  MatchPlayerResponse,
} from "../api/client";
import ignaraImage from "../../../../assets/characters/ignara.png";
import liraImage from "../../../../assets/characters/lira.png";
import sophiaImage from "../../../../assets/characters/sophia.png";
import yunaImage from "../../../../assets/characters/yuna.png";
import ignaraWallpaper from "../../../../assets/wallpapers/ignara-wallpaper.png";
import liraWallpaper from "../../../../assets/wallpapers/lira-wallpaper.png";
import sophiaWallpaper from "../../../../assets/wallpapers/sophia-wallpaper.png";
import yunaWallpaper from "../../../../assets/wallpapers/yuna-wallpaper.png";
import {
  LobbyRoleIcon,
  normalizeLobbyRoleId,
  type LobbyRoleId,
} from "../lobbyRoles";
import type { Translate } from "../types/ui";
import { getProfileInitials, getPublicDisplayName } from "../utils/profile";

type ChampionSelectionProps = {
  currentPlayerPublicId?: number;
  match: ApiMatchResponse;
  onChampionHover: (champion?: string, publish?: boolean) => Promise<void>;
  onChampionSelect: (champion: string) => Promise<boolean>;
  onPickTimeout: () => void;
  onReadyPhaseComplete: () => Promise<void> | void;
  t: Translate;
};

const warmupSeconds = 10;
const pickSeconds = 20;
const readySeconds = 20;
const champions = [
  { image: liraImage, name: "Lira", wallpaper: liraWallpaper },
  { image: ignaraImage, name: "Ignara", wallpaper: ignaraWallpaper },
  { image: yunaImage, name: "Yuna", wallpaper: yunaWallpaper },
  { image: sophiaImage, name: "Sophia", wallpaper: sophiaWallpaper },
];
const championImagesByName = new Map(
  champions.map((champion) => [champion.name.toLowerCase(), champion.image]),
);
const championWallpapersByName = new Map(
  champions.map((champion) => [champion.name.toLowerCase(), champion.wallpaper]),
);
const championSelectionRoleLabels: Record<LobbyRoleId, string> = {
  adc: "Adc",
  jungle: "Jng",
  mid: "Mid",
  support: "Sup",
  top: "Top",
};

function getTeamName(index: number) {
  return index === 0 ? "Dark Team" : "Light Team";
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

function getPlayerSelection(match: ApiMatchResponse, publicId?: number) {
  return match.championSelections?.find((selection) => {
    return selection.playerPublicId === publicId;
  });
}

function getPlayerHoveredChampion(match: ApiMatchResponse, publicId?: number) {
  const matchWithHoverState = match as ApiMatchResponse & {
    championHovers?: Array<{ champion?: string; playerPublicId?: number }>;
    championPreviews?: Array<{ champion?: string; playerPublicId?: number }>;
    hoveredChampions?: Array<{ champion?: string; playerPublicId?: number }>;
  };
  const hoverStates =
    matchWithHoverState.championHovers ??
    matchWithHoverState.championPreviews ??
    matchWithHoverState.hoveredChampions ??
    [];

  return hoverStates.find((hoverState) => {
    return hoverState.playerPublicId === publicId;
  })?.champion;
}

function getChampionWallpaper(champion?: string) {
  return champion ? championWallpapersByName.get(champion.toLowerCase()) : undefined;
}

function getChampionImage(champion?: string) {
  return champion ? championImagesByName.get(champion.toLowerCase()) : undefined;
}

function getPlayerName(player: MatchPlayerResponse) {
  const playerName = getPublicDisplayName(player.displayName, "");

  if (!playerName || /^(player|user)(?:\s+\d+)?$/i.test(playerName)) {
    return "User";
  }

  return playerName;
}

function getPlayerAssignedRole(match: ApiMatchResponse, player: MatchPlayerResponse) {
  const assignedRole =
    player.assignedRole ??
    match.roleAssignments?.find((assignment) => {
      return assignment.publicId === player.publicId;
    })?.assignedRole;

  return normalizeLobbyRoleId(assignedRole);
}

function getPickGroups(teams: MatchLobbyResponse[]) {
  const darkPlayers = [...(teams[0]?.players ?? [])];
  const lightPlayers = [...(teams[1]?.players ?? [])];
  const totalPlayers = darkPlayers.length + lightPlayers.length;

  if (totalPlayers <= 2) {
    return [[darkPlayers.shift()], [lightPlayers.shift()]]
      .map((group) =>
        group.filter((player): player is MatchPlayerResponse => Boolean(player)),
      )
      .filter((group) => group.length > 0);
  }

  const groups: MatchPlayerResponse[][] = [];

  for (const step of [
    { count: 1, players: darkPlayers },
    { count: 2, players: lightPlayers },
    { count: 2, players: darkPlayers },
    { count: 2, players: lightPlayers },
    { count: 2, players: darkPlayers },
    { count: Number.POSITIVE_INFINITY, players: lightPlayers },
  ]) {
    const group: MatchPlayerResponse[] = [];

    for (let index = 0; index < step.count; index += 1) {
      const player = step.players.shift();

      if (!player) {
        break;
      }

      group.push(player);
    }

    if (group.length > 0) {
      groups.push(group);
    }
  }

  return groups;
}

function getSelectedPublicIds(match: ApiMatchResponse) {
  return new Set(
    match.championSelections
      ?.map((selection) => selection.playerPublicId)
      .filter((publicId): publicId is number => typeof publicId === "number") ?? [],
  );
}

function ChampionSelection({
  currentPlayerPublicId,
  match,
  onChampionHover,
  onChampionSelect,
  onPickTimeout,
  onReadyPhaseComplete,
  t,
}: ChampionSelectionProps) {
  const [phaseStartedAt, setPhaseStartedAt] = useState(Date.now());
  const [phaseNow, setPhaseNow] = useState(Date.now());
  const [warmupDone, setWarmupDone] = useState(false);
  const [activePickGroupIndex, setActivePickGroupIndex] = useState(0);
  const [preselectedChampionWallpaper, setPreselectedChampionWallpaper] = useState<string>();
  const [localWarmupHoverChampion, setLocalWarmupHoverChampion] = useState<string>();
  const [localPickHoverChampion, setLocalPickHoverChampion] = useState<string>();
  const [preselectedChampion, setPreselectedChampion] = useState<string>();
  const [gameClientStarting, setGameClientStarting] = useState(false);
  const [localSelectedChampion, setLocalSelectedChampion] = useState<string>();
  const [selectingChampion, setSelectingChampion] = useState<string>();
  const teams = useMemo(() => getMatchTeams(match), [match]);
  const pickGroups = useMemo(() => getPickGroups(teams), [teams]);
  const selectedPublicIds = useMemo(() => getSelectedPublicIds(match), [match]);
  const selectedSignature = match.championSelections
    ?.map((selection) => `${selection.playerPublicId}:${selection.champion}`)
    .sort()
    .join("|") ?? "";
  const activePickGroup = pickGroups[activePickGroupIndex] ?? [];
  const activePickPublicIds = new Set(
    activePickGroup
      .map((player) => player.publicId)
      .filter((publicId): publicId is number => typeof publicId === "number"),
  );
  const currentPickTeamIndex = teams[1]?.players?.some((player) => {
    return activePickPublicIds.has(player.publicId ?? -1);
  })
    ? 1
    : 0;
  const currentPlayerTeamIndex = teams[1]?.players?.some((player) => {
    return player.publicId === currentPlayerPublicId;
  })
    ? 1
    : 0;
  const allPlayersSelected =
    pickGroups.length > 0 && activePickGroupIndex >= pickGroups.length;
  const serverSelectedChampion = match.championSelections?.find((selection) => {
    return selection.playerPublicId === currentPlayerPublicId;
  })?.champion;
  const activePhase = !warmupDone ? "warmup" : allPlayersSelected ? "ready" : "pick";
  const selectedChampion = serverSelectedChampion ?? localSelectedChampion;
  const canCurrentPlayerPick =
    activePhase === "pick" &&
    typeof currentPlayerPublicId === "number" &&
    activePickPublicIds.has(currentPlayerPublicId) &&
    !selectedChampion;
  const previewedChampion =
    activePhase === "warmup"
      ? localWarmupHoverChampion ?? preselectedChampion
      : localPickHoverChampion ?? preselectedChampion;
  const selectedChampionWallpaper = getChampionWallpaper(selectedChampion);
  const previewedChampionWallpaper = getChampionWallpaper(previewedChampion);
  function getPlayerTeamIndex(player: MatchPlayerResponse) {
    return teams[1]?.players?.some((teamPlayer) => {
      return teamPlayer.publicId === player.publicId;
    })
      ? 1
      : 0;
  }
  function canShowPlayerPreviewToCurrentClient(player: MatchPlayerResponse) {
    const playerTeamIndex = getPlayerTeamIndex(player);
    const sameTeam = playerTeamIndex === currentPlayerTeamIndex;
    const isActivePicker =
      activePhase === "pick" && activePickPublicIds.has(player.publicId ?? -1);

    return sameTeam || isActivePicker;
  }
  const activePickerPreviewChampions = activePickGroup
    .map((player) => {
      if (!canShowPlayerPreviewToCurrentClient(player)) {
        return undefined;
      }

      if (
        player.publicId === currentPlayerPublicId &&
        (activePhase === "warmup" || canCurrentPlayerPick)
      ) {
        return (
          previewedChampion ??
          getPlayerHoveredChampion(match, player.publicId) ??
          getPlayerSelection(match, player.publicId)?.champion
        );
      }

      return (
        getPlayerHoveredChampion(match, player.publicId) ??
        getPlayerSelection(match, player.publicId)?.champion
      );
    })
    .filter((champion): champion is string => Boolean(champion));
  const activePickerPreviewWallpapers = activePickerPreviewChampions
    .map(getChampionWallpaper)
    .filter((wallpaper): wallpaper is string => Boolean(wallpaper));
  const bubbleChampionWallpapers =
    activePhase === "ready"
      ? [selectedChampionWallpaper].filter((wallpaper): wallpaper is string => Boolean(wallpaper))
      : Array.from(
          new Set(
            [previewedChampionWallpaper, ...activePickerPreviewWallpapers].filter(
              (wallpaper): wallpaper is string => Boolean(wallpaper),
            ),
          ),
        ).slice(0, 2);
  const bubbleChampionNames =
    activePhase === "ready"
      ? [selectedChampion].filter((champion): champion is string => Boolean(champion))
      : Array.from(
          new Set(
            [previewedChampion, ...activePickerPreviewChampions].filter(
              (champion): champion is string => Boolean(champion),
            ),
          ),
        ).slice(0, 2);
  const centerChampionName =
    activePhase === "ready" ? selectedChampion : bubbleChampionNames[0];
  const isCurrentPlayerPicking =
    activePhase === "pick" &&
    typeof currentPlayerPublicId === "number" &&
    activePickPublicIds.has(currentPlayerPublicId);
  const showSelectedChampionBubble = activePhase === "ready" || !isCurrentPlayerPicking;
  const activePickCardIndexes =
    teams[currentPickTeamIndex]?.players
      ?.map((player, playerIndex) => {
        return activePickPublicIds.has(player.publicId ?? -1) ? playerIndex : -1;
      })
      .filter((playerIndex) => playerIndex >= 0) ?? [];
  const activePickCardIndex =
    activePickCardIndexes.length > 0
      ? activePickCardIndexes.reduce((sum, playerIndex) => sum + playerIndex, 0) /
        activePickCardIndexes.length
      : 2;
  const pickIndicatorOffset = `calc(${(activePickCardIndex - 2) * 106}px + var(--pick-indicator-list-offset, 0px))`;
  const pickIndicatorSide = currentPickTeamIndex === 1 ? "light" : "dark";
  const phaseDurationSeconds =
    activePhase === "warmup"
      ? warmupSeconds
      : activePhase === "ready"
        ? readySeconds
        : pickSeconds;
  const phaseElapsedMs = Math.max(0, phaseNow - phaseStartedAt);
  const phaseElapsedSeconds = Math.floor(phaseElapsedMs / 1_000);
  const phaseSeconds = Math.max(0, phaseDurationSeconds - phaseElapsedSeconds);
  const phaseProgress = Math.min(1, phaseElapsedMs / (phaseDurationSeconds * 1_000));
  const canCurrentPlayerPreselect = activePhase !== "ready" && !selectedChampion;
  const confirmableChampion = canCurrentPlayerPick ? preselectedChampion : undefined;

  async function handleChampionSelect(champion: string) {
    if (!canCurrentPlayerPick || selectingChampion) {
      return;
    }

    setSelectingChampion(champion);
    const selected = await onChampionSelect(champion);
    setSelectingChampion(undefined);

    if (selected) {
      setLocalSelectedChampion(champion);
      setLocalPickHoverChampion(undefined);
      setPreselectedChampion(undefined);
      void onChampionHover(undefined, true);
      if (
        activePickGroup.length > 0 &&
        activePickGroup.every((player) => {
          return (
            player.publicId === currentPlayerPublicId ||
            selectedPublicIds.has(player.publicId ?? -1)
          );
        })
      ) {
        setActivePickGroupIndex((currentIndex) => currentIndex + 1);
      }
    }
  }

  function handleChampionHover(champion: string) {
    if (activePhase === "warmup") {
      setLocalWarmupHoverChampion(champion);
      return;
    }

    if (!canCurrentPlayerPick) {
      return;
    }

    setLocalPickHoverChampion(champion);
    void onChampionHover(champion, true);
  }

  function handleChampionHoverClear() {
    if (activePhase === "warmup") {
      setLocalWarmupHoverChampion(preselectedChampion);
      return;
    }

    if (canCurrentPlayerPick) {
      setLocalPickHoverChampion(undefined);
      void onChampionHover(preselectedChampion, true);
    }
  }

  function handleChampionPreselect(champion: string) {
    if (!canCurrentPlayerPreselect || selectingChampion) {
      return;
    }

    setPreselectedChampion(champion);
    setLocalPickHoverChampion(undefined);
    setPreselectedChampionWallpaper(getChampionWallpaper(champion));

    if (activePhase === "warmup") {
      setLocalWarmupHoverChampion(champion);
      void onChampionHover(champion, true);
      return;
    }

    void onChampionHover(champion, true);
  }

  function handleChampionConfirm() {
    if (!confirmableChampion) {
      return;
    }

    void handleChampionSelect(confirmableChampion);
  }

  useEffect(() => {
    const intervalId = window.setInterval(() => {
      setPhaseNow(Date.now());
    }, 100);

    return () => {
      window.clearInterval(intervalId);
    };
  }, []);

  useEffect(() => {
    setPhaseStartedAt(Date.now());
    setPhaseNow(Date.now());
  }, [activePhase, activePickGroupIndex]);

  useEffect(() => {
    if (
      activePhase === "pick" &&
      activePickGroup.length > 0 &&
      activePickGroup.every((player) => selectedPublicIds.has(player.publicId ?? -1))
    ) {
      setActivePickGroupIndex((currentIndex) => currentIndex + 1);
    }
  }, [activePhase, activePickGroup, selectedPublicIds, selectedSignature]);

  useEffect(() => {
    if (activePhase === "warmup" && phaseElapsedSeconds >= warmupSeconds) {
      setWarmupDone(true);
      setLocalWarmupHoverChampion(undefined);
    }

    if (activePhase === "pick" && phaseElapsedSeconds >= pickSeconds) {
      onPickTimeout();
    }

    if (
      activePhase === "ready" &&
      phaseElapsedSeconds >= readySeconds &&
      !gameClientStarting
    ) {
      setGameClientStarting(true);
      void onReadyPhaseComplete();
    }
  }, [
    activePhase,
    gameClientStarting,
    onPickTimeout,
    onReadyPhaseComplete,
    phaseElapsedSeconds,
    preselectedChampion,
  ]);

  return (
    <main
      className={[
        "champion-selection-page",
        preselectedChampionWallpaper || selectedChampionWallpaper
          ? "champion-selection-page-wallpaper"
          : "",
        selectedChampionWallpaper ? "champion-selection-page-selected-wallpaper" : "",
      ]
        .filter(Boolean)
        .join(" ")}
      style={
        {
          "--champion-selection-wallpaper": preselectedChampionWallpaper || selectedChampionWallpaper
            ? `url(${preselectedChampionWallpaper ?? selectedChampionWallpaper})`
            : undefined,
        } as CSSProperties
      }
      aria-label={t("champion-select-title")}
    >
      <section className="champion-selection-timer" aria-live="polite">
        <span>
          {gameClientStarting
            ? t("champion-select-game-starting")
            : activePhase === "warmup"
              ? t("champion-select-warmup")
              : activePhase === "ready"
                ? t("champion-select-ready")
                : getTeamName(currentPickTeamIndex)}
        </span>
        <strong>{String(phaseSeconds).padStart(2, "0")}</strong>
        <div
          className={[
            "champion-selection-timeline",
            activePhase !== "warmup" ? "champion-selection-timeline-active" : "",
            activePhase === "ready" ? "champion-selection-timeline-ready" : "",
            activePhase === "pick" && currentPickTeamIndex === 1
              ? "champion-selection-timeline-light"
              : "",
          ]
            .filter(Boolean)
            .join(" ")}
        >
          <span style={{ transform: `scaleX(${phaseProgress})` }} />
        </div>
      </section>

      <section className="champion-selection-layout">
        {teams.map((team, teamIndex) => {
          const isActivePickTeam =
            activePhase === "pick" &&
            (team.players ?? []).some((player) => {
              return activePickPublicIds.has(player.publicId ?? -1);
            });
          const isOpponentTeam = teamIndex !== currentPlayerTeamIndex;

          return (
            <aside
              className={[
                "champion-selection-team",
                teamIndex === 0
                  ? "champion-selection-team-dark"
                  : "champion-selection-team-light",
                isActivePickTeam ? "champion-selection-team-active" : "",
              ]
                .filter(Boolean)
                .join(" ")}
              key={team.lobbyId ?? teamIndex}
            >
              <h2>{getTeamName(teamIndex)}</h2>
              <div className="champion-selection-team-list">
                {(team.players ?? []).map((player) => {
                  const playerSelection = getPlayerSelection(match, player.publicId);
                  const playerHoveredChampion =
                    player.publicId === currentPlayerPublicId &&
                    (activePhase === "warmup" || canCurrentPlayerPick)
                      ? previewedChampion
                      : getPlayerHoveredChampion(match, player.publicId);
                  const visiblePlayerHoveredChampion = canShowPlayerPreviewToCurrentClient(player)
                    ? playerHoveredChampion
                    : undefined;
                  const previewChampion =
                    playerSelection?.champion ?? visiblePlayerHoveredChampion;
                  const playerChampionImage = getChampionImage(previewChampion);
                  const isCurrentPick = activePickPublicIds.has(player.publicId ?? -1);
                  const isCurrentPlayer = player.publicId === currentPlayerPublicId;
                  const playerName = isOpponentTeam
                    ? t("champion-select-opponent")
                    : isCurrentPlayer
                      ? t("champion-select-self")
                      : getPlayerName(player);
                  const assignedRole = !isOpponentTeam
                    ? getPlayerAssignedRole(match, player)
                    : undefined;

                  return (
                    <article
                      className={
                        [
                          "champion-selection-player",
                          isCurrentPick ? "current" : "",
                          isOpponentTeam ? "opponent" : "ally",
                        ]
                          .filter(Boolean)
                          .join(" ")
                      }
                      key={player.publicId}
                    >
                      <div className="champion-selection-player-avatar">
                        {playerChampionImage ? (
                          <img alt="" src={playerChampionImage} />
                        ) : !isOpponentTeam && player.avatarUrl ? (
                          <img alt="" src={player.avatarUrl} />
                        ) : !isOpponentTeam ? (
                          getProfileInitials(player.displayName ?? "?")
                        ) : null}
                      </div>
                      <div className="champion-selection-player-meta">
                        <span>{playerName}</span>
                        {previewChampion ? (
                          <small>{previewChampion}</small>
                        ) : null}
                        {assignedRole ? (
                          <small className="champion-selection-player-role">
                            <LobbyRoleIcon role={assignedRole} />
                            <span aria-hidden="true">&bull;</span>
                            <strong>{championSelectionRoleLabels[assignedRole]}</strong>
                          </small>
                        ) : null}
                      </div>
                    </article>
                  );
                })}
              </div>
            </aside>
          );
        })}

        {activePhase === "pick" && activePickGroup.length > 0 ? (
          <div
            className={[
              "champion-selection-pick-indicator",
              `champion-selection-pick-indicator-${pickIndicatorSide}`,
            ].join(" ")}
            style={{ "--pick-indicator-offset": pickIndicatorOffset } as CSSProperties}
            aria-live="polite"
          >
            <span aria-hidden="true" />
            <strong>{String(phaseSeconds).padStart(2, "0")}</strong>
          </div>
        ) : null}

        <section className="champion-selection-center">
          {selectedChampion ? (
            <div className="champion-selection-picked">
              <span>
                {activePhase === "ready"
                  ? t("champion-select-own-champion")
                  : getTeamName(currentPickTeamIndex)}
              </span>
              {showSelectedChampionBubble ? (
                <div className="champion-selection-opponent-bubble">
                  {bubbleChampionWallpapers.length > 0 ? (
                    <div
                      className={
                        bubbleChampionWallpapers.length > 1
                          ? "champion-selection-bubble-wallpapers split"
                          : "champion-selection-bubble-wallpapers"
                      }
                      aria-hidden="true"
                    >
                      {bubbleChampionWallpapers.map((wallpaper, index) => (
                        <span
                          key={`${wallpaper}-${index}`}
                          style={
                            {
                              "--bubble-wallpaper": `url(${wallpaper})`,
                            } as CSSProperties
                          }
                        />
                      ))}
                    </div>
                  ) : null}
                </div>
              ) : null}
              {centerChampionName ? <strong>{centerChampionName}</strong> : null}
            </div>
          ) : (
            <>
              <span>{match.mode ?? "Ranked"}</span>
              <h1>{t("champion-select-title")}</h1>
              <div className="champion-selection-champions">
                {champions.map((champion) => (
                  <button
                    className={
                      preselectedChampion === champion.name
                        ? "champion-selection-champion selected"
                        : "champion-selection-champion"
                    }
                    disabled={!canCurrentPlayerPreselect || Boolean(selectingChampion)}
                    key={champion.name}
                    type="button"
                    onClick={() => handleChampionPreselect(champion.name)}
                    onMouseEnter={() => handleChampionHover(champion.name)}
                    onMouseLeave={handleChampionHoverClear}
                    onFocus={() => handleChampionHover(champion.name)}
                    onBlur={handleChampionHoverClear}
                  >
                    <span>
                      <img alt="" src={champion.image} />
                    </span>
                    <strong>{champion.name}</strong>
                  </button>
                ))}
              </div>
              {confirmableChampion ? (
                <button
                  className="champion-selection-confirm-button"
                  disabled={Boolean(selectingChampion)}
                  type="button"
                  onClick={handleChampionConfirm}
                >
                  <span>
                    {selectingChampion ? selectingChampion : t("champion-select-confirm")}
                  </span>
                </button>
              ) : null}
            </>
          )}
        </section>
      </section>
    </main>
  );
}

export default ChampionSelection;
