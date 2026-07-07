import { expect, test, type Page } from "@playwright/test";
import { getCredentials } from "./support/auth";
import { mockAuthenticatedClientApi } from "./support/mockClientApi";

const friendSidebarStorageKey = "mira-client-friend-sidebar-v2";

function folderButtonName(name: string) {
  return new RegExp(`^${name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s+\\d+$`);
}

test.beforeEach(async ({ page }) => {
  await mockAuthenticatedClientApi(page);
  await page.addInitScript(() => {
    localStorage.removeItem("mira.auth.tokens");
    localStorage.removeItem("mira-client-friend-sidebar-v2");
    localStorage.removeItem("mira-client-blocked-public-ids-v1");
    sessionStorage.removeItem("mira.auth.state");
    sessionStorage.removeItem("mira.auth.codeVerifier");
  });
});

test.afterEach(async ({ page }) => {
  await page.unrouteAll({ behavior: "ignoreErrors" });
});

async function loginToClient(page: Page) {
  const { email, password } = getCredentials();

  await page.goto("/");
  await page.getByLabel("Email oder Benutzername").fill(email);
  await page.getByLabel("Passwort").fill(password);
  await page.getByRole("button", { name: /Einloggen/ }).click();

  await expect(page.getByLabel("Dashboard")).toBeVisible();
  await expect(page.getByRole("button", { name: "Spiel" })).toBeVisible();
}

test("supports sidebar navigation and collapse", async ({ page }) => {
  await loginToClient(page);

  await expect(page.getByRole("button", { name: "Your Friends" })).toHaveClass(/active/);
  await expect(page.getByRole("button", { name: "Your Teams" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Tournaments" })).toBeVisible();

  await page.getByRole("button", { name: "Your Teams" }).click();
  await expect(page.getByRole("button", { name: "Your Teams" })).toHaveClass(/active/);
  await page.getByRole("button", { name: "Tournaments" }).click();
  await expect(page.getByRole("button", { name: "Tournaments" })).toHaveClass(/active/);
  await page.getByRole("button", { name: "Your Friends" }).click();
  await expect(page.getByRole("button", { name: "Your Friends" })).toHaveClass(/active/);

  await page.getByRole("button", { name: "Sidebar einfahren" }).click();
  await expect(page.getByRole("button", { name: "Sidebar ausfahren" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Profilmenü öffnen" })).toBeVisible();
  await page.getByRole("button", { name: "Sidebar ausfahren" }).click();
  await expect(page.getByRole("button", { name: "Sidebar einfahren" })).toBeVisible();
});

test("supports settings page controls", async ({ page }) => {
  await loginToClient(page);

  await page.getByRole("button", { name: "Settings" }).click();
  const settingsDialog = page.getByRole("dialog", { name: "Einstellungen" });
  await expect(settingsDialog).toBeVisible();
  await expect(settingsDialog.getByRole("button", { name: "Oberfläche" })).toHaveClass(
    /active/,
  );

  await expect(settingsDialog.getByText("Auflösung")).toBeVisible();
  await settingsDialog.getByRole("button", { name: "1600 x 900" }).click();
  await expect(settingsDialog.getByRole("option", { name: "1400 x 800" })).toBeVisible();
  await settingsDialog.getByRole("option", { name: "1400 x 800" }).click();
  await expect(settingsDialog.getByRole("button", { name: "1400 x 800" })).toBeVisible();

  await settingsDialog.getByRole("button", { name: "Alle" }).click();
  await expect(settingsDialog.getByRole("option", { name: "Keine" })).toBeVisible();
  await settingsDialog.getByRole("option", { name: "Keine" }).click();
  await expect(settingsDialog.getByRole("button", { name: "Keine" })).toBeVisible();

  await settingsDialog.getByRole("button", { name: "Spiel" }).click();
  await expect(settingsDialog.getByRole("button", { name: "Rahmenlos" })).toBeVisible();
  await settingsDialog.getByRole("button", { name: "Allgemein" }).click();
  await expect(settingsDialog.getByRole("button", { name: "Erlauben" })).toBeVisible();

  await settingsDialog.getByRole("button", { name: "Schliessen" }).click();
  await expect(settingsDialog).toBeHidden();
});

test("opens and closes chat from the dock and friend list", async ({ page }) => {
  await loginToClient(page);
  const chatDock = page.getByRole("region", { name: "Chat" });

  await page.getByRole("button", { name: "Chat öffnen" }).click();
  await expect(chatDock).toHaveClass(/open/);
  await expect(page.getByText("Öffne einen Freundes-Chat.")).toBeVisible();
  await page.getByRole("button", { name: "Chat schliessen" }).click();
  await expect(chatDock).not.toHaveClass(/open/);

  await page.locator(".friend-card").filter({ hasText: "Lane Partner" }).dblclick();
  await expect(chatDock).toHaveClass(/open/);
  await expect(page.getByText("Lane Partner").last()).toBeVisible();
  await expect(chatDock.getByText("Ready for duo?")).toBeVisible();
  const messageInput = page.getByRole("textbox", { name: "Nachricht" });
  await expect(messageInput).toBeEnabled();
  await messageInput.fill("E2E private ping");
  await page.getByRole("button", { name: "Nachricht senden" }).click();
  await expect(messageInput).toBeEmpty();
  await expect(page.getByText("E2E private ping")).toBeVisible();
  await page.getByRole("button", { name: "Chat schliessen" }).click();
  await expect(chatDock).not.toHaveClass(/open/);
});

test("discovers incoming private chat rooms from the room last message", async ({ page }) => {
  const incomingCreatedAt = new Date("2026-06-25T10:01:00.000Z").toISOString();

  await page.route(
    /^(https:\/\/api\.tilt-us\.com|http:\/\/localhost:8085)\/(?:chat\/)?api\/chats(?:\/private-9001-9101\/messages)?(?:\?.*)?$/,
    async (route) => {
      const pathname = new URL(route.request().url()).pathname.replace(
        /^\/chat(?=\/api\/)/,
        "",
      );

      if (pathname === "/api/chats") {
        await route.fulfill({
          contentType: "application/json",
          json: [
            {
              id: "private-9001-9101",
              type: "PRIVATE",
              participantPublicIds: [9001, 9101],
              createdAt: incomingCreatedAt,
              lastMessage: {
                id: "message-e2e-incoming",
                chatRoomId: "private-9001-9101",
                senderPublicId: 9101,
                content: "Incoming DB ping",
                createdAt: incomingCreatedAt,
              },
              unreadCount: 0,
              updatedAt: incomingCreatedAt,
            },
          ],
        });
        return;
      }

      await route.fulfill({
        contentType: "application/json",
        json: {
          messages: [
            {
              id: "message-e2e-incoming",
              chatRoomId: "private-9001-9101",
              senderPublicId: 9101,
              content: "Incoming DB ping",
              createdAt: incomingCreatedAt,
            },
          ],
        },
      });
    },
  );

  await loginToClient(page);
  const chatDock = page.getByRole("region", { name: "Chat" });

  await expect(chatDock).not.toHaveClass(/open/);
  await page.getByRole("button", { name: "Chat öffnen" }).click();
  await expect(chatDock).toHaveClass(/open/);
  const incomingChat = chatDock.locator(".chat-contact-card").filter({
    hasText: "Lane Partner",
  });
  await expect(incomingChat).toBeVisible();
  await expect(incomingChat.locator(".chat-contact-unread-badge")).toHaveText("1");
  await incomingChat.locator(".chat-contact-button").click();
  await expect(chatDock.getByText("Incoming DB ping")).toBeVisible();
  await incomingChat.getByRole("button", { name: "Chat-Karte schliessen" }).click();
  await expect(incomingChat).toBeHidden();
  await page.waitForTimeout(5_500);
  await expect(incomingChat).toBeHidden();
});

test("supports the client friend list and folder workflow", async ({ page }) => {
  const folderName = `E2E Squad ${Date.now()}`;
  const secondFolderName = `${folderName} Two`;
  const renamedSecondFolderName = `${folderName} Renamed`;
  const folderFriendRequests: Array<{
    folderName: string;
    method: string;
  }> = [];
  const folderRenameRequests: Array<{
    bodyName?: string;
    folderName: string;
    method: string;
  }> = [];

  page.on("request", (request) => {
    const url = new URL(request.url());
    const match = url.pathname.match(
      /\/api\/me\/settings\/folders\/([^/]+)\/friends\/9101$/,
    );

    if (!match) {
      return;
    }

    folderFriendRequests.push({
      folderName: decodeURIComponent(match[1]),
      method: request.method(),
    });
  });

  page.on("request", (request) => {
    const url = new URL(request.url());
    const match = url.pathname.match(
      /\/api\/me\/settings\/folders\/([^/]+)\/rename$/,
    );

    if (!match) {
      return;
    }

    const body = request.postDataJSON() as { name?: string };

    folderRenameRequests.push({
      bodyName: body.name,
      folderName: decodeURIComponent(match[1]),
      method: request.method(),
    });
  });

  await loginToClient(page);

  await expect(page.getByRole("button", { name: "Your Friends" })).toHaveClass(/active/);
  await expect(page.getByLabel("Freunde suchen")).toBeVisible();
  await expect(page.getByText("Lane Partner")).toBeVisible();
  await expect(page.getByText("Jungle Buddy")).toBeVisible();
  await expect(page.getByRole("button", { name: "Ordner erstellen" })).toBeVisible();

  await page.getByRole("button", { name: "Ordner erstellen" }).click();
  const folderDialog = page.getByRole("dialog", { name: "Neuer Ordner" });
  await expect(folderDialog).toBeVisible();
  await page.getByLabel("Ordnername").fill(folderName);
  await folderDialog.getByRole("button", { name: "Erstellen", exact: true }).click();

  const folderButton = page.getByRole("button", { name: folderButtonName(folderName) });
  await expect(folderButton).toBeVisible();
  await expect(folderButton).toContainText("0");

  await page
    .locator(".friend-card")
    .filter({ hasText: "Lane Partner" })
    .getByRole("button", { name: "Freund-Aktionen" })
    .click();
  await page.getByRole("menuitem", { name: "Verschieben nach" }).hover();
  await page.getByRole("menu", { name: "Verschieben nach" })
    .getByRole("menuitem", { name: folderName })
    .click();

  await expect(folderButton).toContainText("1");
  const folderSection = page
    .locator(".friend-folder-section")
    .filter({ has: page.getByRole("button", { name: folderButtonName(folderName) }) });
  await expect(folderSection.getByText("Lane Partner")).toBeVisible();

  await page.getByRole("button", { name: "Ordner erstellen" }).click();
  await expect(folderDialog).toBeVisible();
  await page.getByLabel("Ordnername").fill(secondFolderName);
  await folderDialog.getByRole("button", { name: "Erstellen", exact: true }).click();

  const secondFolderButton = page.getByRole("button", {
    name: folderButtonName(secondFolderName),
  });
  await expect(secondFolderButton).toBeVisible();
  await expect(secondFolderButton).toContainText("0");

  await folderSection
    .locator(".friend-card")
    .filter({ hasText: "Lane Partner" })
    .getByRole("button", { name: "Freund-Aktionen" })
    .click();
  await page.getByRole("menuitem", { name: "Verschieben nach" }).hover();
  await page.getByRole("menu", { name: "Verschieben nach" })
    .getByRole("menuitem", { name: secondFolderName })
    .click();

  await expect(folderButton).toContainText("0");
  await expect(secondFolderButton).toContainText("1");
  await expect(folderSection.getByText("Lane Partner")).toBeHidden();

  const secondFolderSection = page
    .locator(".friend-folder-section")
    .filter({
      has: page.getByRole("button", { name: folderButtonName(secondFolderName) }),
    });
  await expect(secondFolderSection.getByText("Lane Partner")).toBeVisible();

  await expect
    .poll(() => folderFriendRequests)
    .toEqual([
      { folderName, method: "POST" },
      { folderName, method: "DELETE" },
      { folderName: secondFolderName, method: "POST" },
    ]);

  await secondFolderSection.getByRole("button", { name: "Ordner-Aktionen" }).click();
  await page.getByRole("menuitem", { name: "Umbenennen" }).click();
  const folderRenameInput = page.locator(".friend-folder-rename-input");
  await folderRenameInput.fill(renamedSecondFolderName);
  await folderRenameInput.press("Enter");

  const renamedSecondFolderButton = page.getByRole("button", {
    name: folderButtonName(renamedSecondFolderName),
  });
  await expect(renamedSecondFolderButton).toBeVisible();
  await expect(renamedSecondFolderButton).toContainText("1");
  await expect
    .poll(() => folderRenameRequests)
    .toEqual([
      {
        bodyName: renamedSecondFolderName,
        folderName: secondFolderName,
        method: "PUT",
      },
    ]);

  await expect
    .poll(async () => {
      return page.evaluate(({ renamedSecondFolderName, storageKey }) => {
        const storedSidebar = localStorage.getItem(storageKey);

        if (!storedSidebar) {
          return false;
        }

        const parsedSidebar = JSON.parse(storedSidebar) as {
          folders?: Array<{ id: string; name: string }>;
          friendFolders?: Record<string, string | undefined>;
        };
        const folder = parsedSidebar.folders?.find(
          (currentFolder) => currentFolder.name === renamedSecondFolderName,
        );

        return Boolean(folder && parsedSidebar.friendFolders?.["9101"] === folder.id);
      }, { renamedSecondFolderName, storageKey: friendSidebarStorageKey });
    })
    .toBe(true);
});

test("shows incoming lobby invitations with live event field aliases", async ({ page }) => {
  let invitationUpdatedAt = "2026-07-07T10:00:00.000Z";

  await page.route(
    /^(https:\/\/api\.tilt-us\.com|http:\/\/localhost:8082|http:\/\/localhost:8080|http:\/\/localhost:8083)\/(?:live\/|auth\/|match\/)?api\/lobbies\/invitations(?:\?.*)?$/,
    async (route) => {
      await route.fulfill({
        contentType: "application/json",
        json: [
          {
            invitee_public_id: "9001",
            inviters: [{ publicId: "9101", displayName: "Lane Partner" }],
            lobby: {
              id: "invite-lobby-e2e",
              mode: "RANKED",
              members: [{ publicId: "9101", displayName: "Lane Partner" }],
              ownerPublicId: "9101",
            },
            updatedAt: invitationUpdatedAt,
          },
        ],
      });
    },
  );

  await loginToClient(page);

  const inviteCard = page.locator(".lobby-invite-card").filter({ hasText: "Lane Partner" });

  await expect(inviteCard).toBeVisible();
  await expect(page.getByRole("button", { name: "Einladung annehmen" })).toBeVisible();
  await page.getByRole("button", { name: "Einladung ablehnen" }).click();
  await expect(inviteCard).toBeHidden();
  await page.waitForTimeout(3_200);
  await expect(inviteCard).toBeHidden();

  invitationUpdatedAt = "2026-07-07T10:00:01.000Z";

  await expect(inviteCard).toBeVisible({ timeout: 5_000 });
});

test("opens close dialog and logs out", async ({ page }) => {
  const statusUpdates: string[] = [];

  await page.route(
    /^(https:\/\/api\.tilt-us\.com|http:\/\/localhost:8082)\/(?:live\/)?api\/user-status\/me$/,
    async (route) => {
      if (route.request().method() === "PUT") {
        const body = route.request().postDataJSON() as { status?: string } | null;

        if (body?.status) {
          statusUpdates.push(body.status);
        }

        await route.fulfill({
          contentType: "application/json",
          json: {
            publicId: 9001,
            status: body?.status ?? "ONLINE",
            updatedAt: new Date().toISOString(),
          },
        });
        return;
      }

      await route.fulfill({
        contentType: "application/json",
        json: {
          publicId: 9001,
          status: "ONLINE",
          updatedAt: new Date().toISOString(),
        },
      });
    },
  );

  await loginToClient(page);

  await page.getByRole("button", { name: "Schliessen" }).click();
  const closeDialog = page.getByRole("dialog", { name: "Mira Client" });
  await expect(closeDialog).toBeVisible();
  await closeDialog.getByRole("button", { name: "Abmelden" }).click();

  await expect(page.getByRole("heading", { name: "Mira Account" })).toBeVisible();
  await expect(page.getByLabel("Dashboard")).toBeHidden();
  expect(statusUpdates).toContain("OFFLINE");
});

test("opens close dialog and requests quit", async ({ page }) => {
  const statusUpdates: string[] = [];

  await page.route(
    /^(https:\/\/api\.tilt-us\.com|http:\/\/localhost:8082)\/(?:live\/)?api\/user-status\/me$/,
    async (route) => {
      if (route.request().method() === "PUT") {
        const body = route.request().postDataJSON() as { status?: string } | null;

        if (body?.status) {
          statusUpdates.push(body.status);
        }

        await route.fulfill({
          contentType: "application/json",
          json: {
            publicId: 9001,
            status: body?.status ?? "ONLINE",
            updatedAt: new Date().toISOString(),
          },
        });
        return;
      }

      await route.fulfill({
        contentType: "application/json",
        json: {
          publicId: 9001,
          status: "ONLINE",
          updatedAt: new Date().toISOString(),
        },
      });
    },
  );

  await page.addInitScript(() => {
    window.close = () => {
      (window as unknown as { __miraE2eCloseRequested?: boolean })
        .__miraE2eCloseRequested = true;
    };
  });

  await loginToClient(page);

  await page.getByRole("button", { name: "Schliessen" }).click();
  const closeDialog = page.getByRole("dialog", { name: "Mira Client" });
  await expect(closeDialog).toBeVisible();
  await closeDialog.getByRole("button", { name: "Beenden" }).click();

  await expect
    .poll(async () => {
      return page.evaluate(() =>
        Boolean(
          (window as unknown as { __miraE2eCloseRequested?: boolean })
            .__miraE2eCloseRequested,
        ),
      );
    })
    .toBe(true);
  expect(statusUpdates).toContain("OFFLINE");
});
