import { existsSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

const root = path.resolve(import.meta.dirname, "..");
const npmCommand = process.platform === "win32" ? "npm.cmd" : "npm";
const binaryName = process.platform === "win32" ? "mira-installer.exe" : "mira-installer";
const binaryPath = path.join(root, "src-tauri", "target", "debug", binaryName);

const build = spawnSync(
  npmCommand,
  ["run", "tauri", "--", "build", "--debug", "--no-bundle"],
  {
    cwd: root,
    stdio: "inherit",
  },
);

if (build.status !== 0) {
  process.exit(build.status ?? 1);
}

if (!existsSync(binaryPath)) {
  console.error(`Installer binary was not found: ${binaryPath}`);
  process.exit(1);
}

const app = spawnSync(binaryPath, {
  cwd: root,
  stdio: "inherit",
});

process.exit(app.status ?? 0);
