import { defineConfig } from "vitest/config";

export default defineConfig({
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
