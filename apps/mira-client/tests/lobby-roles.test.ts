import { beforeEach, describe, expect, test } from "vitest";
import {
  hasLobbyRoles,
  lobbyRolesStorageKey,
  normalizeLobbyRoleSelection,
  readStoredLobbyRoles,
  writeStoredLobbyRoles,
} from "../src/lobbyRoles";

describe("lobby role persistence", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  test("returns empty selection when nothing is stored", () => {
    expect(readStoredLobbyRoles()).toEqual([undefined, undefined]);
  });

  test("remembers the selected role after closing the lobby", () => {
    // Simulate selecting a primary and secondary role in the lobby.
    writeStoredLobbyRoles(normalizeLobbyRoleSelection(["mid", "jungle"]));

    // Reopening the client / creating a new lobby restores the preference.
    expect(readStoredLobbyRoles()).toEqual(["mid", "jungle"]);
  });

  test("normalizes stored aliases and drops duplicate secondary roles", () => {
    localStorage.setItem(
      lobbyRolesStorageKey,
      JSON.stringify(["JNG", "jungle"]),
    );

    expect(readStoredLobbyRoles()).toEqual(["jungle", undefined]);
  });

  test("keeps a single remembered role when no secondary was chosen", () => {
    writeStoredLobbyRoles(["support", undefined]);

    const storedRoles = readStoredLobbyRoles();

    expect(storedRoles).toEqual(["support", undefined]);
    expect(hasLobbyRoles(storedRoles)).toBe(true);
  });

  test("ignores malformed stored values", () => {
    localStorage.setItem(lobbyRolesStorageKey, "not-json");
    expect(readStoredLobbyRoles()).toEqual([undefined, undefined]);

    localStorage.setItem(lobbyRolesStorageKey, JSON.stringify({ role: "mid" }));
    expect(readStoredLobbyRoles()).toEqual([undefined, undefined]);

    localStorage.setItem(lobbyRolesStorageKey, JSON.stringify(["banana", 42]));
    expect(readStoredLobbyRoles()).toEqual([undefined, undefined]);
  });
});
