import { expect, test, type Page } from "@playwright/test";
import { getCredentials } from "./support/auth";
import { mockAuthenticatedClientApi } from "./support/mockClientApi";

const friendSidebarStorageKey = "mira-client-friend-sidebar-v2";

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
  await expect(page.getByRole("textbox", { name: "Nachricht" })).toBeEnabled();
  await page.getByRole("button", { name: "Chat schliessen" }).click();
  await expect(chatDock).not.toHaveClass(/open/);
});

test("supports the client friend list and folder workflow", async ({ page }) => {
  const folderName = `E2E Squad ${Date.now()}`;

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

  const folderButton = page.getByRole("button", { name: new RegExp(folderName) });
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
    .filter({ has: page.getByRole("button", { name: new RegExp(folderName) }) });
  await expect(folderSection.getByText("Lane Partner")).toBeVisible();

  await expect
    .poll(async () => {
      return page.evaluate(({ storageKey, folderName }) => {
        const storedSidebar = localStorage.getItem(storageKey);

        if (!storedSidebar) {
          return false;
        }

        const parsedSidebar = JSON.parse(storedSidebar) as {
          folders?: Array<{ id: string; name: string }>;
          friendFolders?: Record<string, string | undefined>;
        };
        const folder = parsedSidebar.folders?.find(
          (currentFolder) => currentFolder.name === folderName,
        );

        return Boolean(folder && parsedSidebar.friendFolders?.["9101"] === folder.id);
      }, { folderName, storageKey: friendSidebarStorageKey });
    })
    .toBe(true);
});

test("opens close dialog and logs out", async ({ page }) => {
  await loginToClient(page);

  await page.getByRole("button", { name: "Schliessen" }).click();
  const closeDialog = page.getByRole("dialog", { name: "Mira Client" });
  await expect(closeDialog).toBeVisible();
  await closeDialog.getByRole("button", { name: "Abmelden" }).click();

  await expect(page.getByRole("heading", { name: "Mira Account" })).toBeVisible();
  await expect(page.getByLabel("Dashboard")).toBeHidden();
});

test("opens close dialog and requests quit", async ({ page }) => {
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
});
