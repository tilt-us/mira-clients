import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const separatorIndex = process.argv.indexOf("--");

if (separatorIndex === -1 || separatorIndex === process.argv.length - 1) {
  console.error("Usage: node scripts/with-env.mjs KEY=value [...] -- command [args...]");
  process.exit(1);
}

const env = { ...process.env };
const pathKey = Object.keys(env).find((key) => key.toLowerCase() === "path") ?? "PATH";
const rustBinPath = path.join(process.env.USERPROFILE ?? "", ".cargo", "bin");

if (process.platform === "win32" && fs.existsSync(path.join(rustBinPath, "cargo.exe"))) {
  env[pathKey] = `${rustBinPath}${path.delimiter}${env[pathKey] ?? ""}`;
}

for (const assignment of process.argv.slice(2, separatorIndex)) {
  const equalsIndex = assignment.indexOf("=");

  if (equalsIndex <= 0) {
    console.error(`Invalid environment assignment: ${assignment}`);
    process.exit(1);
  }

  const key = assignment.slice(0, equalsIndex);
  const value = assignment.slice(equalsIndex + 1);

  const isPathLike =
    path.isAbsolute(value) ||
    value.startsWith("./") ||
    value.startsWith("../") ||
    value.startsWith(".\\") ||
    value.startsWith("..\\");

  env[key] = isPathLike ? path.resolve(value) : value;
}

const [command, ...args] = process.argv.slice(separatorIndex + 1);
const commandPath = resolveCommand(command, env);
const shimTarget = getNpmCmdShimTarget(commandPath);
const child = spawn(shimTarget ? process.execPath : commandPath, shimTarget ? [shimTarget, ...args] : args, {
  env,
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

function resolveCommand(command, env) {
  if (process.platform !== "win32" || command.includes("/") || command.includes("\\")) {
    return command;
  }

  const extensions = (env.PATHEXT ?? ".COM;.EXE;.BAT;.CMD")
    .split(";")
    .filter(Boolean);
  const hasExtension = extensions.some((extension) =>
    command.toLowerCase().endsWith(extension.toLowerCase()),
  );
  const candidates = hasExtension
    ? [command]
    : [...extensions.map((extension) => `${command}${extension.toLowerCase()}`), command];

  for (const directory of (env[pathKey] ?? "").split(path.delimiter)) {
    for (const candidate of candidates) {
      const fullPath = path.join(directory, candidate);

      if (fs.existsSync(fullPath)) {
        return fullPath;
      }
    }
  }

  return command;
}

function getNpmCmdShimTarget(commandPath) {
  if (process.platform !== "win32" || !commandPath.toLowerCase().endsWith(".cmd")) {
    return undefined;
  }

  const contents = fs.readFileSync(commandPath, "utf8");
  const match = contents.match(/"%dp0%\\\.\.\\([^"]+?\.js)"/i);

  if (!match) {
    return undefined;
  }

  return path.resolve(path.dirname(commandPath), "..", match[1]);
}
