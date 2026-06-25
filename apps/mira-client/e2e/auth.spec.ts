import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.route("**/api/public/login-options", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: { providers: ["google"] },
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
  await expect(page.getByRole("button", { name: /Mit Google anmelden/ })).toBeEnabled();
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
