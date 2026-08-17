import { afterEach, describe, expect, test, vi } from "vitest";
import type { ApiMatchResponse } from "../src/api/client";
import {
  canSubmitChampionSelection,
  getWarmupDelayMs,
  scheduleWarmupPick,
} from "../src/pages/championSelectionLifecycle";

function warmupMatch(matchId: string, endsAt = "2026-08-15T12:00:10.000Z") {
  return {
    matchId,
    phase: "WARMUP",
    phaseEndsAt: endsAt,
    serverNow: "2026-08-15T12:00:00.000Z",
    status: "CHAMPION_SELECTION",
  } as ApiMatchResponse;
}

afterEach(() => {
  vi.useRealTimers();
});

describe("champion selection lifecycle", () => {
  test("changes WARMUP to PICK using phaseEndsAt minus serverNow", () => {
    vi.useFakeTimers();
    const onPick = vi.fn();
    const match = warmupMatch("match-one");

    expect(getWarmupDelayMs(match)).toBe(10_000);
    scheduleWarmupPick(match, onPick);

    vi.advanceTimersByTime(9_999);
    expect(onPick).not.toHaveBeenCalled();

    vi.advanceTimersByTime(1);
    expect(onPick).toHaveBeenCalledWith("match-one");
  });

  test("cleans the old timer when a new match event replaces it", () => {
    vi.useFakeTimers();
    const onPick = vi.fn();
    const stopFirstTimer = scheduleWarmupPick(warmupMatch("match-one"), onPick);

    stopFirstTimer();
    scheduleWarmupPick(warmupMatch("match-two"), onPick);
    vi.advanceTimersByTime(10_000);

    expect(onPick).toHaveBeenCalledTimes(1);
    expect(onPick).toHaveBeenCalledWith("match-two");
  });

  test("switches immediately when the server deadline is exactly zero", () => {
    vi.useFakeTimers();
    const onPick = vi.fn();
    const match = warmupMatch("match-one", "2026-08-15T12:00:00.000Z");

    scheduleWarmupPick(match, onPick);

    expect(onPick).toHaveBeenCalledTimes(1);
    expect(onPick).toHaveBeenCalledWith("match-one");
  });

  test("switches once when a delayed timer crosses the deadline", () => {
    vi.useFakeTimers();
    const onPick = vi.fn();

    scheduleWarmupPick(warmupMatch("match-one"), onPick);
    vi.advanceTimersByTime(20_000);
    window.dispatchEvent(new Event("focus"));

    expect(onPick).toHaveBeenCalledTimes(1);
  });

  test("catches up after a paused or inactive client resumes", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-15T12:00:00.000Z"));
    const onPick = vi.fn();
    scheduleWarmupPick(warmupMatch("match-one"), onPick);

    vi.setSystemTime(new Date("2026-08-15T12:00:15.000Z"));
    document.dispatchEvent(new Event("visibilitychange"));

    expect(onPick).toHaveBeenCalledTimes(1);
  });

  test("stopping the warmup lifecycle prevents a stale event from switching", () => {
    vi.useFakeTimers();
    const onPick = vi.fn();
    const stopWarmup = scheduleWarmupPick(warmupMatch("match-one"), onPick);

    stopWarmup();
    vi.runAllTimers();

    expect(onPick).not.toHaveBeenCalled();
  });

  test("allows champion selection only in PICK", () => {
    expect(canSubmitChampionSelection("warmup")).toBe(false);
    expect(canSubmitChampionSelection("pick")).toBe(true);
    expect(canSubmitChampionSelection("ready")).toBe(false);
  });
});
