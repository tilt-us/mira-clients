import { defineConfig, devices } from "@playwright/test";
import { fileURLToPath } from "node:url";

const rootTestResults = fileURLToPath(
  new URL("../../tests-results/mira-client/e2e/", import.meta.url),
);

export default defineConfig({
  testDir: "./e2e",
  outputDir: `${rootTestResults}/test-artifacts`,
  timeout: 60_000,
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  reporter: [
    [process.env.CI ? "github" : "list"],
    ["html", { open: "never", outputFolder: `${rootTestResults}/html-report` }],
    ["junit", { outputFile: `${rootTestResults}/junit/results.xml` }],
  ],
  use: {
    baseURL: "http://127.0.0.1:4173",
    trace: "on-first-retry",
  },
  webServer: {
    command: "npm run dev -- --host 127.0.0.1 --port 4173",
    url: "http://127.0.0.1:4173",
    reuseExistingServer: process.env.E2E_REUSE_SERVER === "1",
    timeout: 120_000,
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});
