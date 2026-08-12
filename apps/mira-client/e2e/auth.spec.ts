import { expect, test, type Page } from "@playwright/test";
import { getEnvironmentConfig, getMiraEnvironment } from "../scripts/environment.mjs";

const keycloakAuthPattern = new RegExp(
  `^${escapeRegExp(
    getEnvironmentConfig(getMiraEnvironment(process.env.MIRA_ENV, "dev")).authIssuerUrl,
  )}/.*$`,
);

async function mockKeycloakAuth(page: Page) {
  await page.route(keycloakAuthPattern, async (route) => {
    await route.fulfill({
      contentType: "text/html",
      body: "<!doctype html><title>Keycloak</title>",
    });
  });
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
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
  await expect(page.getByRole("button", { name: "Passwort vergessen?" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Google" })).toBeEnabled();
  await expect(page.getByRole("button", { name: "GitHub" })).toBeEnabled();
  await expect(page.getByRole("button", { name: "Discord" })).toBeEnabled();
});

test("opens the registered-account password reset flow without OAuth provider hint", async ({
  page,
}) => {
  await mockKeycloakAuth(page);

  await page.goto("/");
  const redirectUri = page.url();
  const expectedResetRedirectUri = new URL(redirectUri);
  expectedResetRedirectUri.searchParams.set("mira_password_reset", "sent");

  await Promise.all([
    page.waitForURL(/reset-credentials/),
    page.getByRole("button", { name: "Passwort vergessen?" }).click(),
  ]);

  const resetUrl = new URL(page.url());

  expect(resetUrl.pathname).toContain("/login-actions/reset-credentials");
  expect(resetUrl.searchParams.get("client_id")).toBe("mira-bevy");
  expect(resetUrl.searchParams.get("redirect_uri")).toBe(
    expectedResetRedirectUri.toString(),
  );
  expect(resetUrl.searchParams.get("accent")).toBe("f2c45b");
  expect(resetUrl.searchParams.get("fontColor")).toBe("black");
  expect(resetUrl.searchParams.get("kc_locale")).toBe("de");
  expect(resetUrl.searchParams.get("lang")).toBe("german");
  expect(resetUrl.searchParams.get("ui_locales")).toBe("de");
  expect(resetUrl.searchParams.has("kc_idp_hint")).toBe(false);
  expect(resetUrl.searchParams.has("prompt")).toBe(false);
});

test("shows a mail-check toast after the password reset callback", async ({ page }) => {
  await page.goto("/?mira_password_reset=sent");

  await expect(
    page.getByText("Bitte prüfe deine Emails, um dein Passwort zurückzusetzen."),
  ).toBeVisible();
  expect(new URL(page.url()).searchParams.has("mira_password_reset")).toBe(false);
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
  expect(authUrl.searchParams.get("client_id")).toBe("mira-bevy");
  expect(authUrl.searchParams.get("code_challenge_method")).toBe("S256");
  expect(authUrl.searchParams.get("accent")).toBe("f2c45b");
  expect(authUrl.searchParams.get("fontColor")).toBe("black");
  expect(authUrl.searchParams.get("kc_locale")).toBe("de");
  expect(authUrl.searchParams.get("lang")).toBe("german");
  expect(authUrl.searchParams.get("ui_locales")).toBe("de");
  expect(authUrl.searchParams.has("prompt")).toBe(false);
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
  expect(authUrl.searchParams.get("client_id")).toBe("mira-bevy");
  expect(authUrl.searchParams.get("code_challenge_method")).toBe("S256");
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
  expect(authUrl.searchParams.get("client_id")).toBe("mira-bevy");
  expect(authUrl.searchParams.get("code_challenge_method")).toBe("S256");
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
