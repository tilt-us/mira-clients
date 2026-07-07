import type { Page, Route } from "@playwright/test";
import { createUnsignedJwt, getKeycloakIssuerUrl } from "./auth";

const apiRequestPattern =
  /^(https:\/\/api\.tilt-us\.com|http:\/\/localhost:808[0-5])\//;

const now = new Date("2026-06-25T10:00:00.000Z").toISOString();

type MockClientSettingsFolder = {
  friendPublicIds: number[];
  moveHereWhen?: string;
  name: string;
};

let mockClientSettingsFolders: MockClientSettingsFolder[] = [];

export async function mockAuthenticatedClientApi(page: Page) {
  mockClientSettingsFolders = [];

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

  if (pathname.endsWith("/protocol/openid-connect/logout")) {
    const referer = request.headers().referer;
    const appOrigin = referer ? new URL(referer).origin : "http://127.0.0.1:4173";
    const redirectScript = `window.location.replace(${JSON.stringify(`${appOrigin}/`)})`;

    await route.fulfill({
      contentType: "text/html",
      body: `<!doctype html><script>${redirectScript}</script>`,
    });
    return;
  }

  if (pathname === "/api/public/login-options") {
    await route.fulfill({
      contentType: "application/json",
      json: { providers: ["email", "google", "github", "discord"] },
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

  if (pathname === "/api/me/settings") {
    await route.fulfill({
      contentType: "application/json",
      json: {
        accent_color: "#f2c45b",
        allow_friend_request: "allow",
        background: "yuna",
        chat_position: "right",
        client_animation: "all",
        folders: [],
        language: "de",
        resolution: "1600x900",
        screen_mode: "borderless",
        show_email_public: false,
        ui_scale: 0.9,
        use_friend_colors: false,
      },
    });
    return;
  }

  if (pathname === "/api/me/settings/folders") {
    if (request.method() === "PUT") {
      mockClientSettingsFolders = normalizeMockClientSettingsFolders(
        request.postDataJSON() as MockClientSettingsFolder[] | null,
      );
    }

    await route.fulfill({
      contentType: "application/json",
      json: mockClientSettingsFolders,
    });
    return;
  }

  if (pathname === "/api/me/settings/folders/order") {
    const body = request.postDataJSON() as { folderNames?: unknown } | null;
    const reorderedFolders = reorderMockClientSettingsFolders(body?.folderNames);

    if (!reorderedFolders) {
      await route.fulfill({
        contentType: "application/json",
        json: { message: "Invalid folder order." },
        status: 400,
      });
      return;
    }

    mockClientSettingsFolders = reorderedFolders;

    await route.fulfill({
      contentType: "application/json",
      json: mockClientSettingsFolders,
    });
    return;
  }

  const folderFriendMatch = pathname.match(
    /^\/api\/me\/settings\/folders\/([^/]+)\/friends\/(\d+)$/,
  );

  if (folderFriendMatch) {
    const folderName = decodeURIComponent(folderFriendMatch[1]);
    const friendPublicId = Number(folderFriendMatch[2]);
    const folder = upsertMockClientSettingsFolder(folderName, {});

    if (request.method() === "DELETE") {
      folder.friendPublicIds = folder.friendPublicIds.filter(
        (currentFriendPublicId) => currentFriendPublicId !== friendPublicId,
      );
    } else {
      folder.friendPublicIds = [...new Set([...folder.friendPublicIds, friendPublicId])];
    }

    await route.fulfill({
      contentType: "application/json",
      json: folder,
    });
    return;
  }

  const folderRenameMatch = pathname.match(
    /^\/api\/me\/settings\/folders\/([^/]+)\/rename$/,
  );

  if (folderRenameMatch) {
    const folderName = decodeURIComponent(folderRenameMatch[1]);
    const body = request.postDataJSON() as { name?: unknown } | null;
    const renamedFolder = renameMockClientSettingsFolder(folderName, body?.name);

    if (!renamedFolder) {
      await route.fulfill({
        contentType: "application/json",
        json: { message: "Invalid folder rename." },
        status: 400,
      });
      return;
    }

    if (renamedFolder === "conflict") {
      await route.fulfill({
        contentType: "application/json",
        json: { message: "A folder with this name already exists." },
        status: 409,
      });
      return;
    }

    await route.fulfill({
      contentType: "application/json",
      json: renamedFolder,
    });
    return;
  }

  const folderMatch = pathname.match(/^\/api\/me\/settings\/folders\/([^/]+)$/);

  if (folderMatch) {
    const folderName = decodeURIComponent(folderMatch[1]);

    if (request.method() === "DELETE") {
      mockClientSettingsFolders = mockClientSettingsFolders.filter(
        (folder) => folder.name.toLocaleLowerCase() !== folderName.toLocaleLowerCase(),
      );

      await route.fulfill({ status: 204 });
      return;
    }

    const body = request.postDataJSON() as Partial<MockClientSettingsFolder> | null;
    const folder = upsertMockClientSettingsFolder(folderName, body ?? {});

    await route.fulfill({
      contentType: "application/json",
      json: folder,
    });
    return;
  }

  const settingsSummaryMatch = pathname.match(
    /^\/api\/users\/(\d+)\/settings-summary$/,
  );

  if (settingsSummaryMatch) {
    await route.fulfill({
      contentType: "application/json",
      json: {
        accentColor: "#f2c45b",
        publicId: Number(settingsSummaryMatch[1]),
        showEmailPublic: true,
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

  if (pathname === "/api/users/online") {
    const page = Math.max(0, Number(url.searchParams.get("page") ?? 0));
    const limit = Math.max(1, Number(url.searchParams.get("limit") ?? 50));
    const users = [
      {
        publicId: 9001,
        displayName: "E2E Client",
        status: "ONLINE",
        updatedAt: now,
      },
      {
        publicId: 9101,
        displayName: "Lane Partner",
        status: "ONLINE",
        updatedAt: now,
      },
      {
        publicId: 9102,
        displayName: "Jungle Buddy",
        status: "AFK",
        updatedAt: now,
      },
    ];
    const offset = page * limit;

    await route.fulfill({
      contentType: "application/json",
      json: {
        limit,
        page,
        total: users.length,
        totalPages: Math.ceil(users.length / limit),
        users: users.slice(offset, offset + limit),
      },
    });
    return;
  }

  if (pathname === "/api/chats") {
    await route.fulfill({
      contentType: "application/json",
      json: [
        {
          roomId: "private-9001-9101",
          type: "PRIVATE",
          participantPublicIds: [9001, 9101],
          createdAt: now,
          lastReadAt: now,
          unreadCount: 0,
          updatedAt: now,
        },
      ],
    });
    return;
  }

  if (pathname === "/api/chats/private/9101/messages" && request.method() === "POST") {
    const body = request.postDataJSON() as { content?: string } | null;

    await route.fulfill({
      contentType: "application/json",
      json: {
        messageId: "message-e2e-private",
        roomId: "private-9001-9101",
        senderPublicId: 9001,
        content: body?.content ?? "",
        createdAt: now,
      },
    });
    return;
  }

  if (pathname === "/api/chats/private/9102/messages" && request.method() === "POST") {
    const body = request.postDataJSON() as { content?: string } | null;

    await route.fulfill({
      contentType: "application/json",
      json: {
        messageId: "message-e2e-private-2",
        roomId: "private-9001-9102",
        senderPublicId: 9001,
        content: body?.content ?? "",
        createdAt: now,
      },
    });
    return;
  }

  if (pathname === "/api/chats/private-9001-9101/messages") {
    await route.fulfill({
      contentType: "application/json",
      json: {
        messages: [
          {
            messageId: "message-e2e-existing",
            roomId: "private-9001-9101",
            senderPublicId: 9101,
            content: "Ready for duo?",
            createdAt: now,
          },
        ],
      },
    });
    return;
  }

  if (pathname === "/api/chats/private-9001-9101/read") {
    await route.fulfill({
      contentType: "application/json",
      json: {
        roomId: "private-9001-9101",
        type: "PRIVATE",
        participantPublicIds: [9001, 9101],
        createdAt: now,
        lastReadAt: now,
        unreadCount: 0,
        updatedAt: now,
      },
    });
    return;
  }

  if (pathname === "/api/champions") {
    const weekly = url.searchParams.get("weekly") === "true";
    const owned = url.searchParams.get("owned") === "true";
    const champions = weekly
      ? [{ name: "Sophia" }, { name: "Yuna" }]
      : owned
        ? [{ name: "Lira" }]
        : [{ name: "Ignara" }, { name: "Lira" }, { name: "Sophia" }, { name: "Yuna" }];

    await route.fulfill({
      contentType: "application/json",
      json: champions,
    });
    return;
  }

  await route.fulfill({
    contentType: "application/json",
    json: {},
  });
}

function stripServicePrefix(pathname: string) {
  return pathname.replace(/^\/(?:auth|live|match|game|champions|chat)(?=\/api\/)/, "");
}

function normalizeMockClientSettingsFolders(value: MockClientSettingsFolder[] | null) {
  const foldersByName = new Map<string, MockClientSettingsFolder>();

  for (const folder of Array.isArray(value) ? value : []) {
    if (!folder?.name?.trim()) {
      continue;
    }

    const name = folder.name.trim();
    const nameKey = name.toLocaleLowerCase();
    const existingFolder = foldersByName.get(nameKey);
    const friendPublicIds = normalizeMockFriendPublicIds(folder.friendPublicIds);
    const moveHereWhen =
      typeof folder.moveHereWhen === "string" && folder.moveHereWhen.trim()
        ? folder.moveHereWhen.trim().slice(0, 30)
        : undefined;

    foldersByName.set(nameKey, {
      friendPublicIds: [
        ...new Set([...(existingFolder?.friendPublicIds ?? []), ...friendPublicIds]),
      ],
      moveHereWhen: existingFolder?.moveHereWhen ?? moveHereWhen,
      name: existingFolder?.name ?? name,
    });
  }

  return [...foldersByName.values()];
}

function normalizeMockFriendPublicIds(value: unknown) {
  return Array.isArray(value)
    ? [
        ...new Set(
          value.filter(
            (friendPublicId): friendPublicId is number =>
              Number.isInteger(friendPublicId) && friendPublicId > 0,
          ),
        ),
      ]
    : [];
}

function upsertMockClientSettingsFolder(
  folderName: string,
  body: Partial<MockClientSettingsFolder>,
) {
  const name = folderName.trim();
  const nameKey = name.toLocaleLowerCase();
  const existingFolder = mockClientSettingsFolders.find(
    (folder) => folder.name.toLocaleLowerCase() === nameKey,
  );

  if (existingFolder) {
    existingFolder.friendPublicIds =
      body.friendPublicIds === undefined
        ? existingFolder.friendPublicIds
        : normalizeMockFriendPublicIds(body.friendPublicIds);
    existingFolder.moveHereWhen =
      body.moveHereWhen === undefined
        ? existingFolder.moveHereWhen
        : typeof body.moveHereWhen === "string" && body.moveHereWhen.trim()
          ? body.moveHereWhen.trim().slice(0, 30)
          : undefined;

    return existingFolder;
  }

  const folder: MockClientSettingsFolder = {
    friendPublicIds: normalizeMockFriendPublicIds(body.friendPublicIds),
    moveHereWhen:
      typeof body.moveHereWhen === "string" && body.moveHereWhen.trim()
        ? body.moveHereWhen.trim().slice(0, 30)
        : undefined,
    name,
  };

  mockClientSettingsFolders = [...mockClientSettingsFolders, folder];

  return folder;
}

function renameMockClientSettingsFolder(folderName: string, value: unknown) {
  if (typeof value !== "string" || !value.trim()) {
    return undefined;
  }

  const currentNameKey = folderName.trim().toLocaleLowerCase();
  const nextName = value.trim();
  const nextNameKey = nextName.toLocaleLowerCase();
  const folder = mockClientSettingsFolders.find(
    (currentFolder) => currentFolder.name.toLocaleLowerCase() === currentNameKey,
  );

  if (!folder) {
    return undefined;
  }

  const duplicateFolder = mockClientSettingsFolders.find(
    (currentFolder) =>
      currentFolder.name.toLocaleLowerCase() === nextNameKey &&
      currentFolder !== folder,
  );

  if (duplicateFolder) {
    return "conflict" as const;
  }

  folder.name = nextName;

  return folder;
}

function reorderMockClientSettingsFolders(value: unknown) {
  if (!Array.isArray(value) || value.length !== mockClientSettingsFolders.length) {
    return undefined;
  }

  const foldersByName = new Map(
    mockClientSettingsFolders.map((folder) => [
      folder.name.toLocaleLowerCase(),
      folder,
    ]),
  );
  const seenNames = new Set<string>();
  const reorderedFolders: MockClientSettingsFolder[] = [];

  for (const folderName of value) {
    if (typeof folderName !== "string" || !folderName.trim()) {
      return undefined;
    }

    const folderNameKey = folderName.trim().toLocaleLowerCase();
    const folder = foldersByName.get(folderNameKey);

    if (!folder || seenNames.has(folderNameKey)) {
      return undefined;
    }

    seenNames.add(folderNameKey);
    reorderedFolders.push(folder);
  }

  return reorderedFolders;
}
