import assert from "node:assert/strict";
import { readFile, stat } from "node:fs/promises";
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
    { key: "content/game.zip", phase: "artifact" },
    { key: "content/ui.zip", phase: "artifact" },
  ]);
  assert.deepEqual(plan.map((entry) => entry.key), [
    "content/ui.zip",
    "content/game.zip",
    "runtime/manifest.json",
    "latest.json",
  ]);
});

test("keeps UI and game source trees and archive inputs separate", async () => {
  for (const directory of [
    "assets/ui/characters",
    "assets/ui/fonts",
    "assets/ui/wallpapers",
    "assets/ui/icons",
    "assets/game/audio",
    "assets/game/champions",
    "assets/game/maps",
    "assets/game/materials",
  ]) {
    assert.equal((await stat(new URL(`../${directory}`, import.meta.url))).isDirectory(), true);
  }

  const workflow = await readFile(new URL("../.github/workflows/release.yml", import.meta.url), "utf8");
  assert.match(workflow, /garage-content\/\{name\}\.zip/);
  assert.match(workflow, /source = Path\("assets"\) \/ name/);
  assert.match(workflow, /--ui garage-content\/ui\.zip/);
  assert.match(workflow, /--game garage-content\/game\.zip/);
  assert.doesNotMatch(workflow, /assets\.zip/);
});
