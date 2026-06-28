import { expect, test, type Page } from "@playwright/test";

const keycloakAuthPattern =
  /^(?:https:\/\/api\.tilt-us\.com\/keycloak|http:\/\/localhost:8081)\/.*$/;

async function mockKeycloakAuth(page: Page) {
  await page.route(keycloakAuthPattern, async (route) => {
    await route.fulfill({
      contentType: "text/html",
      body: "<!doctype html><title>Keycloak</title>",
    });
  });
}

test.beforeEach(async ({ page }) => {
  await page.route("**/api/public/login-options", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: { providers: ["google", "github", "discord"] },
    });
  });
});

test("renders the authentication screen", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Mira Account" })).toBeVisible();
  await expect(page.getByText("Anmelden oder registrieren")).toBeVisible();
  await expect(page.getByRole("tab", { name: "Anmelden" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expect(page.getByLabel("Email oder Benutzername")).toBeVisible();
  await expect(page.getByLabel("Passwort")).toBeVisible();
  await expect(page.getByRole("button", { name: /Einloggen/ })).toBeVisible();
  await expect(page.getByRole("button", { name: "Google" })).toBeEnabled();
  await expect(page.getByRole("button", { name: "GitHub" })).toBeEnabled();
  await expect(page.getByRole("button", { name: "Discord" })).toBeEnabled();
});

test("starts GitHub login with the GitHub identity provider hint", async ({ page }) => {
  await mockKeycloakAuth(page);

  await page.goto("/");

  await Promise.all([
    page.waitForURL(/kc_idp_hint=github/),
    page.getByRole("button", { name: "GitHub" }).click(),
  ]);

  const authUrl = new URL(page.url());

  expect(authUrl.searchParams.get("kc_idp_hint")).toBe("github");
  expect(authUrl.searchParams.get("accent")).toBe("f2c45b");
  expect(authUrl.searchParams.get("fontColor")).toBe("black");
  expect(authUrl.searchParams.get("kc_locale")).toBe("de");
  expect(authUrl.searchParams.get("lang")).toBe("german");
  expect(authUrl.searchParams.get("ui_locales")).toBe("de");
  expect(authUrl.searchParams.get("prompt")).toBe("select_account");
});

test("starts Google login with account selection and Google language hint", async ({ page }) => {
  await mockKeycloakAuth(page);

  await page.goto("/");

  await Promise.all([
    page.waitForURL(/kc_idp_hint=google/),
    page.getByRole("button", { name: "Google" }).click(),
  ]);

  const authUrl = new URL(page.url());

  expect(authUrl.searchParams.get("kc_idp_hint")).toBe("google");
  expect(authUrl.searchParams.get("accent")).toBe("f2c45b");
  expect(authUrl.searchParams.get("fontColor")).toBe("black");
  expect(authUrl.searchParams.get("kc_locale")).toBe("de");
  expect(authUrl.searchParams.get("lang")).toBe("german");
  expect(authUrl.searchParams.get("hl")).toBe("de");
  expect(authUrl.searchParams.get("ui_locales")).toBe("de");
  expect(authUrl.searchParams.get("prompt")).toBe("select_account");
});

test("starts Discord login with the Discord identity provider hint", async ({ page }) => {
  await mockKeycloakAuth(page);

  await page.goto("/");

  await Promise.all([
    page.waitForURL(/kc_idp_hint=discord/),
    page.getByRole("button", { name: "Discord" }).click(),
  ]);

  const authUrl = new URL(page.url());

  expect(authUrl.searchParams.get("kc_idp_hint")).toBe("discord");
  expect(authUrl.searchParams.get("accent")).toBe("f2c45b");
  expect(authUrl.searchParams.get("fontColor")).toBe("black");
  expect(authUrl.searchParams.get("kc_locale")).toBe("de");
  expect(authUrl.searchParams.get("lang")).toBe("german");
  expect(authUrl.searchParams.get("ui_locales")).toBe("de");
  expect(authUrl.searchParams.has("prompt")).toBe(false);
});

test("switches to the registration form", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("tab", { name: "Registrieren" }).click();

  await expect(page.getByRole("tab", { name: "Registrieren" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expect(page.getByLabel("Anzeigename")).toBeVisible();
  await expect(page.getByLabel("Email")).toBeVisible();
  await expect(page.getByLabel("Passwort")).toBeVisible();
  await expect(page.getByRole("button", { name: /Account erstellen/ })).toBeVisible();
});
