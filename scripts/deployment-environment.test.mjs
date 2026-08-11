import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { resolveDeploymentEnvironment } from "./deployment-environment.mjs";

test("maps deployment branches without a production fallback", () => {
  assert.deepEqual(resolveDeploymentEnvironment({ eventName: "push", refName: "development" }), {
    environment: "dev",
    production: false,
  });
  assert.deepEqual(resolveDeploymentEnvironment({ eventName: "push", refName: "master" }), {
    environment: "staging",
    production: false,
  });
  assert.throws(() => resolveDeploymentEnvironment({ eventName: "push", refName: "main" }));
  assert.throws(() => resolveDeploymentEnvironment({ eventName: "push", refName: "" }));
});

test("permits production only through confirmed manual master dispatch", () => {
  assert.deepEqual(
    resolveDeploymentEnvironment({ eventName: "workflow_dispatch", refName: "master", confirm: "PROD" }),
    { environment: "prod", production: true },
  );
  assert.throws(() =>
    resolveDeploymentEnvironment({ eventName: "workflow_dispatch", refName: "development", confirm: "PROD" }),
  );
  assert.throws(() =>
    resolveDeploymentEnvironment({ eventName: "workflow_dispatch", refName: "master", confirm: "" }),
  );
});

test("passes the resolved environment to every release build and isolates Garage access", async () => {
  const workflow = await readFile(
    fileURLToPath(new URL("../.github/workflows/release.yml", import.meta.url)),
    "utf8",
  );
  const resolvedEnvironment = "MIRA_ENV: ${{ needs.prepare.outputs.environment }}";

  assert.equal(workflow.split(resolvedEnvironment).length - 1, 4);
  assert.doesNotMatch(workflow, /MIRA_ENV:\s*prod/);
  assert.match(workflow, /runs-on: mira-clients-publisher/);
  assert.match(workflow, /os: windows-latest/);
  assert.match(workflow, /os: macos-15/);
  assert.match(workflow, /os: ubuntu-latest/);
  assert.equal(workflow.split('CI: "true"').length - 1, 2);
  assert.equal(workflow.split("--ci --verbose").length - 1, 3);
  assert.doesNotMatch(workflow, /workflow_run/);
});
