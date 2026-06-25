import type { Page, Route } from "@playwright/test";
import { createUnsignedJwt, getKeycloakIssuerUrl } from "./auth";

const apiRequestPattern =
  /^(https:\/\/api\.tilt-us\.com|http:\/\/localhost:808[0-3])\//;

const now = new Date("2026-06-25T10:00:00.000Z").toISOString();

export async function mockAuthenticatedClientApi(page: Page) {
  await page.route(apiRequestPattern, async (route) => {
    await fulfillMockApiRequest(route);
  });
}

async function fulfillMockApiRequest(route: Route) {
  const request = route.request();
  const url = new URL(request.url());
  const pathname = stripServicePrefix(url.pathname);

  if (pathname.endsWith("/protocol/openid-connect/token")) {
    await route.fulfill({
      contentType: "application/json",
      json: {
        access_token: createUnsignedJwt({
          email: "e2e-client@mira.de",
          exp: Math.floor(Date.now() / 1000) + 300,
          iat: Math.floor(Date.now() / 1000),
          iss: getKeycloakIssuerUrl(),
          preferred_username: "e2e-client@mira.de",
          sub: "e2e-client",
        }),
        expires_in: 300,
        refresh_token: "e2e-refresh-token",
        token_type: "Bearer",
      },
    });
    return;
  }

  if (pathname === "/api/public/login-options") {
    await route.fulfill({
      contentType: "application/json",
      json: { providers: ["email", "google"] },
    });
    return;
  }

  if (pathname === "/api/me") {
    await route.fulfill({
      contentType: "application/json",
      json: {
        avatarUrl: "",
        displayName: "E2E Client",
        email: "e2e-client@mira.de",
        publicId: 9001,
        subject: "e2e-client",
      },
    });
    return;
  }

  if (pathname === "/api/live/bootstrap") {
    await route.fulfill({
      contentType: "application/json",
      json: {
        friends: {
          friends: [
            {
              displayName: "Lane Partner",
              email: "lane.partner@mira.de",
              publicId: 9101,
            },
            {
              displayName: "Jungle Buddy",
              email: "jungle.buddy@mira.de",
              publicId: 9102,
            },
          ],
        },
        friendRequests: {
          incoming: [],
          outgoing: [],
        },
        friendStatuses: {
          statuses: [
            {
              publicId: 9101,
              status: "ONLINE",
              updatedAt: now,
            },
            {
              publicId: 9102,
              status: "AFK",
              updatedAt: now,
            },
          ],
        },
        lobbyInvitations: [],
        openFriendLobbies: [],
        userStatus: {
          publicId: 9001,
          status: "ONLINE",
          updatedAt: now,
        },
      },
    });
    return;
  }

  if (pathname === "/api/lobbies/invitations") {
    await route.fulfill({
      contentType: "application/json",
      json: [],
    });
    return;
  }

  if (pathname === "/api/user-status/me") {
    await route.fulfill({
      contentType: "application/json",
      json: {
        publicId: 9001,
        status: "ONLINE",
        updatedAt: now,
      },
    });
    return;
  }

  await route.fulfill({
    contentType: "application/json",
    json: {},
  });
}

function stripServicePrefix(pathname: string) {
  return pathname.replace(/^\/(?:auth|live|match)(?=\/api\/)/, "");
}
