import type { ApiMatchResponse } from "../api/client";

export type ChampionSelectionPhase = "warmup" | "pick" | "ready";

function parseTimestamp(value?: string) {
  const timestamp = value ? Date.parse(value) : Number.NaN;

  return Number.isFinite(timestamp) ? timestamp : undefined;
}

export function getChampionSelectionPhase(
  match: ApiMatchResponse | undefined,
): ChampionSelectionPhase | undefined {
  if (!match) {
    return undefined;
  }

  const phase = match.phase?.trim().toUpperCase();

  if (phase?.includes("READY") || match.status === "READY") {
    return "ready";
  }

  if (phase?.includes("WARMUP") || phase === "PENDING_ACCEPTANCE") {
    return "warmup";
  }

  if (phase?.includes("PICK") || match.status === "CHAMPION_SELECTION") {
    return "pick";
  }

  return undefined;
}

/** The server supplies both timestamps, so the client never guesses a duration. */
export function getWarmupDelayMs(match: ApiMatchResponse) {
  const phaseEndsAt = parseTimestamp(match.phaseEndsAt);
  const serverNow = parseTimestamp(match.serverNow);

  if (phaseEndsAt === undefined || serverNow === undefined) {
    return undefined;
  }

  return Math.max(0, phaseEndsAt - serverNow);
}

export function scheduleWarmupPick(
  match: ApiMatchResponse,
  onPick: (matchId: string) => void,
) {
  const delayMs = getWarmupDelayMs(match);

  if (!match.matchId || delayMs === undefined) {
    return () => undefined;
  }

  let transitioned = false;
  const matchId = match.matchId;
  const deadlineAt = Date.now() + delayMs;
  const transition = () => {
    if (transitioned) {
      return;
    }

    transitioned = true;
    onPick(matchId);
  };

  // Do not wait for another event-loop turn when the deadline has already passed.
  if (delayMs <= 0) {
    transition();
    return () => undefined;
  }

  const catchUpAfterPause = () => {
    if (Date.now() >= deadlineAt) {
      transition();
    }
  };
  const timeoutId = window.setTimeout(transition, delayMs);

  window.addEventListener("focus", catchUpAfterPause);
  document.addEventListener("visibilitychange", catchUpAfterPause);

  return () => {
    window.clearTimeout(timeoutId);
    window.removeEventListener("focus", catchUpAfterPause);
    document.removeEventListener("visibilitychange", catchUpAfterPause);
  };
}

export function canSubmitChampionSelection(phase: ChampionSelectionPhase | undefined) {
  return phase === "pick";
}
