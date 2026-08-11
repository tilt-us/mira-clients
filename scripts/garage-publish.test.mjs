import assert from "node:assert/strict";
import test from "node:test";
import { garageEnvironment, garageObjectUrl, manifestUrls, orderedPublishPlan } from "./garage-publish.mjs";

test("maps environments to stable Garage prefixes", () => {
  assert.deepEqual(garageEnvironment("dev"), { environment: "dev", prefix: "dev/" });
  assert.deepEqual(garageEnvironment("staging"), { environment: "staging", prefix: "staging/" });
  assert.deepEqual(garageEnvironment("prod"), { environment: "prod", prefix: "" });
  assert.throws(() => garageEnvironment("preview"));
  assert.throws(() => garageEnvironment(""));
  assert.equal(garageObjectUrl("prod", "latest.json"), "https://downloads.tilt-us.com/latest.json");
});

test("generates fixed manifest URLs", () => {
  assert.deepEqual(manifestUrls("staging"), {
    installerManifestUrl: "https://downloads.tilt-us.com/staging/installer/manifest.json",
    runtimeManifestUrl: "https://downloads.tilt-us.com/staging/runtime/manifest.json",
    contentManifestUrl: "https://downloads.tilt-us.com/staging/content/manifest.json",
  });
});

test("publishes latest.json last", () => {
  const plan = orderedPublishPlan([
    { key: "latest.json", phase: "latest" },
    { key: "runtime/manifest.json", phase: "manifest" },
    { key: "content/assets.zip", phase: "artifact" },
  ]);
  assert.deepEqual(plan.map((entry) => entry.key), [
    "content/assets.zip",
    "runtime/manifest.json",
    "latest.json",
  ]);
});
