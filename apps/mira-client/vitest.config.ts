import { defineConfig } from "vitest/config";
import { fileURLToPath, URL } from "node:url";
import { getMiraEnvironment } from "./scripts/environment.mjs";

const repoRoot = fileURLToPath(new URL("../..", import.meta.url));
const environment = getMiraEnvironment(process.env.MIRA_ENV, "dev");

export default defineConfig({
  define: {
    __MIRA_ENV__: JSON.stringify(environment),
    __MIRA_UI_ASSET_DEV_ROOT__: JSON.stringify(`/@fs/${repoRoot}/assets/ui`),
  },
  server: {
    fs: {
      allow: [repoRoot],
    },
  },
  test: {
    coverage: {
      clean: true,
      enabled: false,
      include: [
        "src/auth/storage.ts",
        "src/settings.ts",
        "src/utils/profile.ts",
      ],
      provider: "v8",
      reportsDirectory: "../../tests-results/mira-client/coverage",
      reporter: ["text", "lcov", "json", "cobertura"],
      thresholds: {
        branches: 90,
        functions: 90,
        lines: 90,
        statements: 90,
      },
    },
    environment: "jsdom",
    include: ["tests/**/*.test.ts"],
    outputFile: {
      junit: "../../tests-results/mira-client/unit/junit.xml",
    },
    reporters: ["default", "junit"],
  },
});
