import { describe, expect, test } from "vitest";
import {
  gameContentProgressPercent,
  initialGameContentStatus,
  isGameContentBusy,
} from "../src/gameContent";

describe("game content status", () => {
  test.each(["checking", "installing", "updating"] as const)("blocks Play while %s", (state) => {
    expect(isGameContentBusy({ ...initialGameContentStatus, state })).toBe(true);
  });

  test("enables Play only when ready", () => {
    expect(isGameContentBusy({ ...initialGameContentStatus, state: "ready" })).toBe(false);
  });

  test("clamps accent-fill progress to integer percentages", () => {
    expect(gameContentProgressPercent({ ...initialGameContentStatus, progressPercent: -5 })).toBe(0);
    expect(gameContentProgressPercent({ ...initialGameContentStatus, progressPercent: 53.9 })).toBe(53);
    expect(gameContentProgressPercent({ ...initialGameContentStatus, progressPercent: 105 })).toBe(100);
  });
});
