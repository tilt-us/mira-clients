import { describe, expect, test } from "vitest";
import {
  canReconnectGameClient,
  didGameClientExit,
  type GameLaunchParameters,
} from "../src/gameSession";

const launchParameters: GameLaunchParameters = {
  champion: "lira",
  matchId: "match-1",
  matchManifestJson: "{}",
  playerPublicId: 42,
  protocol: "UDP",
  screen: "window",
  serverHost: "217.160.25.101",
  serverPort: 7949,
  team: "light",
};

describe("game-session reconnect", () => {
  test("does not offer reconnect after an unexpected game-client exit", () => {
    expect(canReconnectGameClient(false, false, launchParameters)).toBe(false);
  });

  test("offers reconnect only after the player intentionally closed the client", () => {
    expect(canReconnectGameClient(false, true, launchParameters)).toBe(true);
  });

  test("does not offer reconnect while the game is running or without launch data", () => {
    expect(canReconnectGameClient(true, true, launchParameters)).toBe(false);
    expect(canReconnectGameClient(false, true)).toBe(false);
  });

  test("ends the session when an observed game client exits unexpectedly", () => {
    expect(didGameClientExit(true, false)).toBe(true);
    expect(didGameClientExit(false, false)).toBe(false);
    expect(didGameClientExit(true, true)).toBe(false);
  });
});
