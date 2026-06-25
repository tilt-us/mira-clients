import { expect, test } from "@playwright/test";
import { getCredentials } from "./support/auth";
import { proxyApiRequests } from "./support/apiProxy";

test.beforeEach(async ({ page }) => {
  await proxyApiRequests(page);
  await page.addInitScript(() => {
    localStorage.removeItem("mira.auth.tokens");
    sessionStorage.removeItem("mira.auth.state");
    sessionStorage.removeItem("mira.auth.codeVerifier");
  });
});

test.afterEach(async ({ page }) => {
  await page.unrouteAll({ behavior: "ignoreErrors" });
});

test("logs in with the configured test user", async ({ page }) => {
  const { email, password, target } = getCredentials();

  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Mira Account" })).toBeVisible();
  await page.getByLabel("Email oder Benutzername").fill(email);
  await page.getByLabel("Passwort").fill(password);

  const tokenResponsePromise = page.waitForResponse((response) => {
    return (
      response.request().method() === "POST" &&
      response.url().includes("/protocol/openid-connect/token")
    );
  });

  await page.getByRole("button", { name: /Einloggen/ }).click();

  const tokenResponse = await tokenResponsePromise;
  expect(tokenResponse.ok(), `${target} Keycloak token response`).toBe(true);

  await expect(page.getByLabel("Dashboard")).toBeVisible({ timeout: 20_000 });
  await expect(page.getByRole("button", { name: "Spiel" })).toBeVisible();
  await expect(page.getByLabel("Freunde suchen")).toBeVisible();

  await expect
    .poll(async () => {
      return page.evaluate(() => Boolean(localStorage.getItem("mira.auth.tokens")));
    })
    .toBe(true);
});
