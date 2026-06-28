import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const noSharedAuth = args.some((arg) => arg === "-no-shared" || arg === "--no-shared");
const passthroughArgs = args.filter((arg) => arg !== "-no-shared" && arg !== "--no-shared");

const envAssignments = ["MIRA_CLIENT_CONFIG=./mira-client.toml"];

if (noSharedAuth) {
  envAssignments.push("MIRA_CLIENT_NO_SHARED_AUTH=1");
}

const commandArgs = [
  path.join(scriptDir, "with-env.mjs"),
  ...envAssignments,
  "--",
  "tauri",
  "dev",
  "--config",
  "src-tauri/tauri.desktop-dev.conf.json",
  ...passthroughArgs,
];

const child = spawn(process.execPath, commandArgs, {
  cwd: path.resolve(scriptDir, ".."),
  stdio: "inherit",
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
  }

  process.exit(code ?? 1);
});

child.on("error", (error) => {
  console.error(error.message);
  process.exit(1);
});
