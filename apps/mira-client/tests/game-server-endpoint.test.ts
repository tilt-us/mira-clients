import { afterEach, describe, expect, test, vi } from "vitest";
import type { ApiMatchResponse } from "../src/api/client";
import {
  getMatchGameplayEndpoint,
  requireMatchGameplayEndpoint,
  waitForMatchGameplayEndpoint,
} from "../src/gameSession";
import { isMatchGameStarted } from "../src/pages/Client.helpers";

function readyMatch(gameServerEndpoint?: ApiMatchResponse["gameServerEndpoint"]): ApiMatchResponse {
  return {
    matchId: "match-1",
    status: "READY",
    gameServerEndpoint,
  };
}

afterEach(() => {
  vi.useRealTimers();
});

describe("game server gameplay endpoint", () => {
  test("accepts the dynamic Agones UDP endpoint without a control address", () => {
    const endpoint = requireMatchGameplayEndpoint(
      readyMatch({
        host: "217.160.25.101",
        port: 7949,
        protocol: "UDP",
      }),
    );

    expect(endpoint).toEqual({
      host: "217.160.25.101",
      port: 7949,
      protocol: "UDP",
    });
  });

  test("does not require a legacy localhost control address", () => {
    const match = {
      ...readyMatch({
        host: "217.160.25.101",
        port: 7949,
        protocol: "UDP",
      }),
      gameServer: { controlBaseUrl: "http://127.0.0.1:6000" },
    } as ApiMatchResponse;

    expect(requireMatchGameplayEndpoint(match)).toEqual({
      host: "217.160.25.101",
      port: 7949,
      protocol: "UDP",
    });
  });

  test("does not use legacy control API fields as a gameplay endpoint", () => {
    const legacyControlOnlyMatch = {
      matchId: "match-1",
      status: "READY",
      gameServer: { controlBaseUrl: "http://127.0.0.1:6000" },
    } as ApiMatchResponse;

    expect(
      getMatchGameplayEndpoint(legacyControlOnlyMatch),
    ).toBeUndefined();
  });

  test("rejects a missing gameplay host", () => {
    expect(() =>
      requireMatchGameplayEndpoint(readyMatch({ port: 7949, protocol: "UDP" })),
    ).toThrow("Game server address missing");
  });

  test("does not treat READY as launchable until the external endpoint exists", () => {
    expect(isMatchGameStarted(readyMatch())).toBe(false);
    expect(isMatchGameStarted(readyMatch({
      host: "217.160.25.101",
      port: 7949,
      protocol: "UDP",
    }))).toBe(true);
  });

  test.each([undefined, 0])("rejects a missing or invalid gameplay port: %s", (port) => {
    expect(() =>
      requireMatchGameplayEndpoint(
        readyMatch({ host: "217.160.25.101", port, protocol: "UDP" }),
      ),
    ).toThrow("Game server port missing");
  });

  test("never derives gameplay port 5000 from a legacy control API port", () => {
    const match = {
      ...readyMatch({ host: "217.160.25.101", protocol: "UDP" }),
      gameServer: { controlPort: 6000 },
    } as ApiMatchResponse;

    expect(() =>
      requireMatchGameplayEndpoint(match),
    ).toThrow("Game server port missing");
  });

  test("does not expose a pod-local endpoint as gameplay", () => {
    const match = readyMatch({ host: "10.42.0.9", port: 7949, protocol: "UDP" });

    expect(getMatchGameplayEndpoint(match)).toBeUndefined();
    expect(() => requireMatchGameplayEndpoint(match)).toThrow("Game server address invalid");
  });

  test("waits for a later READY update with a complete endpoint", async () => {
    vi.useFakeTimers();
    let latestMatch = readyMatch({ host: "217.160.25.101", protocol: "UDP" });
    const waitingForEndpoint = waitForMatchGameplayEndpoint("match-1", () => latestMatch);

    await vi.advanceTimersByTimeAsync(250);
    latestMatch = readyMatch({
      host: "217.160.25.101",
      port: 7949,
      protocol: "UDP",
    });
    await vi.advanceTimersByTimeAsync(250);

    await expect(waitingForEndpoint).resolves.toBe(latestMatch);
  });
});
