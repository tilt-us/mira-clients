import { readFileSync } from "node:fs";
import { describe, expect, test } from "vitest";

const clientSource = readFileSync(
  "src/pages/Client.tsx",
  "utf8",
);

describe("champion selection API contract", () => {
  test("does not use internal match endpoints or the match-service base", () => {
    expect(clientSource).not.toContain("/internal/matches");
    expect(clientSource).not.toContain("MATCHMAKING_API_BASE_URL");
    expect(clientSource).not.toContain("get as getMatch");
  });

  test("posts a champion selection through the live API base only", () => {
    expect(clientSource).toContain("const result = await selectChampion({");
    expect(clientSource).toContain("baseUrl: LIVE_API_BASE_URL,");
    expect(clientSource).toContain("body: { champion },");
    expect(clientSource).toContain("path: { matchId },");
  });
});
