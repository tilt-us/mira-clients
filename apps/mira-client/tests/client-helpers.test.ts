import { describe, expect, test } from "vitest";
import {
  findLobbyInvitation,
  getInviteCandidateSubtitle,
  mergeLobbyInvitations,
  normalizeLobbyInvitation,
} from "../src/pages/Client.helpers";
import type { LobbyInvitation } from "../src/api/client";

describe("lobby invitation helpers", () => {
  test("normalizes invitation field aliases from live events", () => {
    const invitation = normalizeLobbyInvitation({
      invitee_public_id: "9001",
      lobby_id: "lobby-alias",
      updatedAt: "2026-07-07T10:00:00.000Z",
    } as unknown as LobbyInvitation);

    expect(invitation).toMatchObject({
      inviteePublicId: 9001,
      lobbyId: "lobby-alias",
    });
  });

  test("finds nested lobby invitation events with snake_case fields", () => {
    const invitation = findLobbyInvitation({
      event: "LOBBY_INVITATION",
      payload: {
        invitee_public_id: "9001",
        inviters: [{ publicId: "9101", displayName: "Lane Partner" }],
        lobby: {
          id: "nested-lobby",
          members: [{ publicId: "9101", displayName: "Lane Partner" }],
        },
        updatedAt: "2026-07-07T10:00:00.000Z",
      },
    });

    expect(invitation).toMatchObject({
      inviteePublicId: 9001,
      lobbyId: "nested-lobby",
      inviters: [{ publicId: 9101 }],
    });
  });

  test("keeps visible invitations targeted at the current profile", () => {
    const invitations = mergeLobbyInvitations(
      [],
      [
        {
          inviteePublicId: 9001,
          inviters: [{ publicId: 9101, displayName: "Lane Partner" }],
          lobbyId: "invite-lobby",
          updatedAt: "2026-07-07T10:00:00.000Z",
        },
      ],
      undefined,
      9001,
    );

    expect(invitations).toHaveLength(1);
    expect(invitations[0]?.lobbyId).toBe("invite-lobby");
  });

  test("shows public invite candidate email alongside tag", () => {
    expect(
      getInviteCandidateSubtitle({
        email: "lane.partner@mira.de",
        name: "Lane Partner",
        publicId: 9101,
        source: "user",
        tagId: "LANE",
      }),
    ).toBe("#LANE · lane.partner@mira.de");
  });
});
