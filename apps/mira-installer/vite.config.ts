import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";
import packageJson from "./package.json" with { type: "json" };

const host = process.env.TAURI_DEV_HOST;
const repoRoot = fileURLToPath(new URL("../..", import.meta.url));

export default defineConfig(() => ({
  plugins: [react()],
  base: "./",
  define: {
    __INSTALLER_VERSION__: JSON.stringify(packageJson.version),
  },
  clearScreen: false,
  server: {
    port: 1430,
    strictPort: true,
    host: host || false,
    fs: {
      allow: [repoRoot],
    },
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1431,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
