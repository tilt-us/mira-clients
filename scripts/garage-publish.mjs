#!/usr/bin/env node
import { createHash } from "node:crypto";
import { createReadStream, promises as fs } from "node:fs";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const PUBLIC_DOWNLOADS_BASE_URL = "https://downloads.tilt-us.com";
const JSON_HEADERS = {
  contentType: "application/json",
  cacheControl: "no-cache",
};
const ARTIFACT_HEADERS = {
  cacheControl: "no-cache",
};

export function garageEnvironment(environment) {
  switch (String(environment).trim().toLowerCase()) {
    case "dev":
      return { environment: "dev", prefix: "dev/" };
    case "staging":
      return { environment: "staging", prefix: "staging/" };
    case "prod":
      return { environment: "prod", prefix: "" };
    default:
      throw new Error(`Invalid MIRA_ENV=${JSON.stringify(environment)}. Use dev, staging, or prod.`);
  }
}

export function garageObjectUrl(environment, objectKey) {
  const { prefix } = garageEnvironment(environment);
  return `${PUBLIC_DOWNLOADS_BASE_URL}/${prefix}${objectKey}`;
}

export function manifestUrls(environment) {
  return {
    installerManifestUrl: garageObjectUrl(environment, "installer/manifest.json"),
    runtimeManifestUrl: garageObjectUrl(environment, "runtime/manifest.json"),
    contentManifestUrl: garageObjectUrl(environment, "content/manifest.json"),
  };
}

export function orderedPublishPlan(entries) {
  const artifactArea = (key) => {
    if (key.startsWith("installer/")) return 0;
    if (key.startsWith("runtime/desktop/")) return 1;
    if (key.startsWith("runtime/game/")) return 2;
    if (key.startsWith("content/")) return 3;
    return 4;
  };
  const artifacts = entries
    .filter((entry) => entry.phase === "artifact")
    .sort((left, right) => artifactArea(left.key) - artifactArea(right.key) || left.key.localeCompare(right.key));
  const manifests = entries.filter((entry) => entry.phase === "manifest");
  const latest = entries.filter((entry) => entry.phase === "latest");
  if (latest.length !== 1) {
    throw new Error("Publish plan must contain exactly one latest.json entry.");
  }
  return [...artifacts, ...manifests, latest[0]];
}

async function walkFiles(directory) {
  const entries = await fs.readdir(directory, { withFileTypes: true });
  const files = await Promise.all(
    entries.map(async (entry) => {
      const path = join(directory, entry.name);
      return entry.isDirectory() ? walkFiles(path) : [path];
    }),
  );
  return files.flat();
}

function extension(path) {
  if (path.endsWith(".AppImage")) return ".AppImage";
  if (path.endsWith(".deb")) return ".deb";
  if (path.endsWith(".rpm")) return ".rpm";
  if (path.endsWith(".dmg")) return ".dmg";
  if (path.endsWith(".exe")) return ".exe";
  return "";
}

function expectedArtifactKey(group, source) {
  const ext = extension(source);
  const normalizedGroup = basename(dirname(source));
  const target = normalizedGroup.split("-").at(-1);
  const artifact = group.replace(/-[a-z]+$/, "");

  if (artifact === "mira-game-client") {
    if (!new Set(["windows", "linux", "macos"]).has(target)) return undefined;
    return `runtime/game/${target}/mira-game-client${target === "windows" ? ".exe" : ""}`;
  }
  if (!new Set(["mira-client", "mira-installer"]).has(artifact)) return undefined;

  const area = artifact === "mira-client" ? "runtime/desktop" : "installer";
  if (target === "windows" && ext === ".exe") return `${area}/windows/${artifact}.exe`;
  if (target === "macos" && ext === ".dmg") return `${area}/macos/${artifact}.dmg`;
  if (target === "linux" && [".AppImage", ".deb", ".rpm"].includes(ext)) {
    return `${area}/linux/${artifact}${ext}`;
  }
  return undefined;
}

async function sha256(path) {
  const hash = createHash("sha256");
  await new Promise((resolvePromise, reject) => {
    const stream = createReadStream(path);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("error", reject);
    stream.on("end", resolvePromise);
  });
  return hash.digest("hex");
}

async function describeArtifact(environment, key, source) {
  const stat = await fs.stat(source);
  if (!stat.isFile() || stat.size === 0) {
    throw new Error(`Artifact is missing or empty: ${source}`);
  }
  return {
    key,
    source,
    url: garageObjectUrl(environment, key),
    sha256: await sha256(source),
    size: stat.size,
  };
}

function byKey(entries, key) {
  const entry = entries.find((candidate) => candidate.key === key);
  if (!entry) throw new Error(`Required release artifact is missing: ${key}`);
  return { url: entry.url, sha256: entry.sha256, size: entry.size };
}

async function writeJson(path, value) {
  await fs.writeFile(path, `${JSON.stringify(value, null, 2)}\n`);
}

export async function prepareGaragePublish({
  environment,
  artifactsDirectory,
  contentArchive,
  outputDirectory,
  commit,
  tag,
  version,
}) {
  const config = garageEnvironment(environment);
  const files = await walkFiles(artifactsDirectory);
  const releaseArtifacts = [];
  for (const source of files) {
    const group = basename(dirname(source));
    const key = expectedArtifactKey(group, source);
    if (key) releaseArtifacts.push(await describeArtifact(config.environment, key, source));
  }
  const content = await describeArtifact(config.environment, "content/assets.zip", contentArchive);
  const allArtifacts = [...releaseArtifacts, content];

  const installerManifest = {
    schemaVersion: 1,
    environment: config.environment,
    installer: {
      windows: byKey(allArtifacts, "installer/windows/mira-installer.exe"),
      linux: {
        appImage: byKey(allArtifacts, "installer/linux/mira-installer.AppImage"),
        deb: byKey(allArtifacts, "installer/linux/mira-installer.deb"),
        rpm: byKey(allArtifacts, "installer/linux/mira-installer.rpm"),
      },
      macos: byKey(allArtifacts, "installer/macos/mira-installer.dmg"),
    },
  };
  const runtimeManifest = {
    schemaVersion: 1,
    environment: config.environment,
    desktop: {
      windows: byKey(allArtifacts, "runtime/desktop/windows/mira-client.exe"),
      linux: {
        appImage: byKey(allArtifacts, "runtime/desktop/linux/mira-client.AppImage"),
        deb: byKey(allArtifacts, "runtime/desktop/linux/mira-client.deb"),
        rpm: byKey(allArtifacts, "runtime/desktop/linux/mira-client.rpm"),
      },
      macos: byKey(allArtifacts, "runtime/desktop/macos/mira-client.dmg"),
    },
    gameClient: {
      windows: byKey(allArtifacts, "runtime/game/windows/mira-game-client.exe"),
      linux: byKey(allArtifacts, "runtime/game/linux/mira-game-client"),
      macos: byKey(allArtifacts, "runtime/game/macos/mira-game-client"),
    },
  };
  const contentManifest = {
    schemaVersion: 1,
    environment: config.environment,
    content: {
      ...byKey(allArtifacts, "content/assets.zip"),
      contentId: content.sha256,
    },
  };
  const latest = {
    schemaVersion: 1,
    environment: config.environment,
    git: { commit, tag, version },
    publishedAt: new Date().toISOString(),
    ...manifestUrls(config.environment),
  };

  await fs.mkdir(outputDirectory, { recursive: true });
  const installerManifestPath = join(outputDirectory, "installer-manifest.json");
  const runtimeManifestPath = join(outputDirectory, "runtime-manifest.json");
  const contentManifestPath = join(outputDirectory, "content-manifest.json");
  const latestPath = join(outputDirectory, "latest.json");
  await Promise.all([
    writeJson(installerManifestPath, installerManifest),
    writeJson(runtimeManifestPath, runtimeManifest),
    writeJson(contentManifestPath, contentManifest),
    writeJson(latestPath, latest),
  ]);

  const plan = orderedPublishPlan([
    ...allArtifacts.map((entry) => ({ ...entry, phase: "artifact", headers: ARTIFACT_HEADERS })),
    {
      key: "installer/manifest.json",
      source: installerManifestPath,
      phase: "manifest",
      headers: JSON_HEADERS,
    },
    {
      key: "runtime/manifest.json",
      source: runtimeManifestPath,
      phase: "manifest",
      headers: JSON_HEADERS,
    },
    {
      key: "content/manifest.json",
      source: contentManifestPath,
      phase: "manifest",
      headers: JSON_HEADERS,
    },
    { key: "latest.json", source: latestPath, phase: "latest", headers: JSON_HEADERS },
  ]);
  const planPath = join(outputDirectory, "upload-plan.json");
  await writeJson(planPath, { environment: config.environment, plan });
  return { planPath, plan, latest };
}

export async function uploadGaragePlan(planPath) {
  const endpoint = process.env.GARAGE_ENDPOINT;
  const bucket = process.env.GARAGE_BUCKET;
  if (!endpoint || !bucket) {
    throw new Error("GARAGE_ENDPOINT and GARAGE_BUCKET are required for Garage publishing.");
  }
  const { environment, plan } = JSON.parse(await fs.readFile(planPath, "utf8"));
  const { prefix } = garageEnvironment(environment);
  for (const entry of orderedPublishPlan(plan)) {
    const args = [
      "s3",
      "cp",
      entry.source,
      `s3://${bucket}/${prefix}${entry.key}`,
      "--endpoint-url",
      endpoint,
      "--only-show-errors",
      "--content-type",
      entry.headers.contentType ?? "application/octet-stream",
      "--cache-control",
      entry.headers.cacheControl,
    ];
    const result = spawnSync("aws", args, { stdio: "inherit", env: process.env });
    if (result.status !== 0) {
      throw new Error(`Garage upload failed before publishing ${entry.key}. latest.json was not updated.`);
    }
  }
}

function argument(name) {
  const index = process.argv.indexOf(`--${name}`);
  if (index < 0 || !process.argv[index + 1]) throw new Error(`--${name} is required.`);
  return process.argv[index + 1];
}

async function main() {
  const command = process.argv[2];
  if (command === "prepare") {
    const result = await prepareGaragePublish({
      environment: argument("environment"),
      artifactsDirectory: resolve(argument("artifacts")),
      contentArchive: resolve(argument("content")),
      outputDirectory: resolve(argument("output")),
      commit: argument("commit"),
      tag: argument("tag"),
      version: argument("version"),
    });
    process.stdout.write(`${relative(process.cwd(), result.planPath)}\n`);
    return;
  }
  if (command === "upload") {
    await uploadGaragePlan(resolve(argument("plan")));
    return;
  }
  throw new Error("Usage: garage-publish.mjs prepare|upload ...");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
