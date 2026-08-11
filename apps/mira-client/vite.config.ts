import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";
import { resolve } from "node:path";
import packageJson from "./package.json" with { type: "json" };
import { getMiraEnvironment } from "./scripts/environment.mjs";

const host = process.env.TAURI_DEV_HOST;
const repoRoot = fileURLToPath(new URL("../..", import.meta.url));

// https://vite.dev/config/
export default defineConfig(({ command, mode }) => {
  const env = loadEnv(mode, process.cwd(), "");
  const environment = getMiraEnvironment(
    process.env.MIRA_ENV ?? env.MIRA_ENV,
    command === "serve" ? "dev" : undefined,
  );

  return {
    plugins: [react()],
    base: "./",
    define: {
      __CLIENT_VERSION__: JSON.stringify(packageJson.version),
      __MIRA_ENV__: JSON.stringify(environment),
      __MIRA_UI_ASSET_DEV_ROOT__: JSON.stringify(`/@fs/${resolve(repoRoot, "assets/ui")}`),
    },

    // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
    //
    // 1. prevent Vite from obscuring rust errors
    clearScreen: false,
    // 2. tauri expects a fixed port, fail if that port is not available
    server: {
      port: 1420,
      strictPort: true,
      host: host || false,
      fs: {
        allow: [repoRoot],
      },
      hmr: host
        ? {
            protocol: "ws",
            host,
            port: 1421,
          }
        : undefined,
      watch: {
        // 3. tell Vite to ignore watching `src-tauri`
        ignored: ["**/src-tauri/**"],
      },
    },
  };
});
