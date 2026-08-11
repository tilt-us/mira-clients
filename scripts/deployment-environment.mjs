#!/usr/bin/env node
import { fileURLToPath } from "node:url";

export function resolveDeploymentEnvironment({ eventName, refName, confirm }) {
  if (eventName === "push") {
    if (refName === "development") return { environment: "dev", production: false };
    if (refName === "master") return { environment: "staging", production: false };
    throw new Error(`Automatic publishing is not configured for branch ${JSON.stringify(refName)}.`);
  }

  if (eventName === "workflow_dispatch") {
    if (refName !== "master") {
      throw new Error("Production publishing must be dispatched from the master branch.");
    }
    if (confirm !== "PROD") {
      throw new Error('Production publishing requires confirm="PROD".');
    }
    return { environment: "prod", production: true };
  }

  throw new Error(`Unsupported deployment event ${JSON.stringify(eventName)}.`);
}

function argument(name) {
  const index = process.argv.indexOf(`--${name}`);
  return index < 0 ? undefined : process.argv[index + 1];
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    const resolution = resolveDeploymentEnvironment({
      eventName: argument("event"),
      refName: argument("branch"),
      confirm: argument("confirm"),
    });
    process.stdout.write(`${JSON.stringify(resolution)}\n`);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
