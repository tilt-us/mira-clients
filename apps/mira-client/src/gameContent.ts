export type GameContentState = "checking" | "installing" | "updating" | "ready" | "error";

export type GameContentStatus = {
  state: GameContentState;
  downloadedBytes: number;
  totalBytes: number;
  progressPercent: number;
  error?: string;
};

export const initialGameContentStatus: GameContentStatus = {
  state: "checking",
  downloadedBytes: 0,
  totalBytes: 0,
  progressPercent: 0,
};

export function isGameContentBusy(status: GameContentStatus) {
  return status.state === "checking" || status.state === "installing" || status.state === "updating";
}

export function gameContentProgressPercent(status: GameContentStatus) {
  return Math.max(0, Math.min(100, Math.trunc(status.progressPercent)));
}
