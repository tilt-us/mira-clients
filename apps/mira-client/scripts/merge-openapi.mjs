import { access, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const DEFAULT_INPUTS = [
  "https://api.tilt-us.com/auth/v3/api-docs",
  "https://api.tilt-us.com/live/v3/api-docs",
  "https://api.tilt-us.com/match/v3/api-docs",
];
const DEFAULT_OUTPUT = ".openapi/api-docs.json";
const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

if (process.env.OPENAPI_INPUT) {
  console.log(
    `Using OPENAPI_INPUT=${process.env.OPENAPI_INPUT}; skipping OpenAPI merge.`,
  );
  process.exit(0);
}

const configuredInputs = parseInputs(process.env.OPENAPI_INPUTS);
const inputs = configuredInputs ?? DEFAULT_INPUTS;
const output = process.env.OPENAPI_MERGED_OUTPUT ?? DEFAULT_OUTPUT;
const outputPath = path.resolve(repoRoot, output);
const specs = await readSpecs(inputs, outputPath, output, {
  allowCachedFallback: !configuredInputs,
});

if (!specs) {
  process.exit(0);
}

const mergedSpec = mergeSpecs(specs, inputs);

await mkdir(path.dirname(outputPath), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(mergedSpec, null, 2)}\n`, "utf8");

console.log(`Merged ${inputs.length} OpenAPI specs into ${output}`);

function parseInputs(value) {
  if (!value) {
    return undefined;
  }

  const inputs = value
    .split(",")
    .map((input) => input.trim())
    .filter(Boolean);

  if (inputs.length === 0) {
    throw new Error("OPENAPI_INPUTS was set, but did not contain any inputs.");
  }

  return inputs;
}

async function readSpecs(inputs, outputPath, output, { allowCachedFallback }) {
  try {
    return await Promise.all(inputs.map(readSpec));
  } catch (error) {
    if (allowCachedFallback && (await fileExists(outputPath))) {
      console.warn(
        `Could not fetch default OpenAPI inputs: ${formatError(error)}`,
      );
      console.warn(
        `Using existing ${output}. Make the API services reachable or set OPENAPI_INPUTS to refresh it.`,
      );

      return undefined;
    }

    const hint = allowCachedFallback
      ? ` No cached ${output} exists. Make the API services reachable or set OPENAPI_INPUTS.`
      : "";

    throw new Error(`${formatError(error)}${hint}`);
  }
}

async function readSpec(input) {
  if (isHttpUrl(input)) {
    let response;

    try {
      response = await fetch(input);
    } catch (error) {
      throw new Error(`Failed to fetch ${input}: ${formatError(error)}`, {
        cause: error,
      });
    }

    if (!response.ok) {
      throw new Error(
        `Failed to fetch ${input}: ${response.status} ${response.statusText}`,
      );
    }

    return response.json();
  }

  const filePath = path.resolve(repoRoot, input);
  return JSON.parse(await readFile(filePath, "utf8"));
}

async function fileExists(filePath) {
  try {
    await access(filePath);
    return true;
  } catch {
    return false;
  }
}

function mergeSpecs(specs, inputs) {
  if (specs.length === 0) {
    throw new Error("No OpenAPI specs were provided.");
  }

  const [baseSpec, ...restSpecs] = specs;
  const mergedSpec = {
    ...baseSpec,
    info: {
      ...baseSpec.info,
      title: baseSpec.info?.title ?? "Merged OpenAPI definition",
      description: mergeDescription(baseSpec.info?.description, inputs),
    },
    servers: uniqueByJson(specs.flatMap((spec) => spec.servers ?? [])),
    paths: { ...(baseSpec.paths ?? {}) },
    components: { ...(baseSpec.components ?? {}) },
    tags: uniqueTags(specs.flatMap((spec) => spec.tags ?? [])),
  };

  for (const [index, spec] of restSpecs.entries()) {
    renameConflictingComponents(mergedSpec.components, spec, inputs[index + 1]);
    mergePaths(mergedSpec.paths, spec.paths ?? {});
    mergeComponents(mergedSpec.components, spec.components ?? {});
  }

  dedupeOperationIds(mergedSpec.paths);

  return mergedSpec;
}

function mergeDescription(description, inputs) {
  const mergedFrom = `Merged from: ${inputs.join(", ")}`;

  if (!description) {
    return mergedFrom;
  }

  return `${description}\n\n${mergedFrom}`;
}

function mergePaths(targetPaths, sourcePaths) {
  for (const [route, sourcePathItem] of Object.entries(sourcePaths)) {
    const targetPathItem = targetPaths[route];

    if (!targetPathItem) {
      targetPaths[route] = sourcePathItem;
      continue;
    }

    for (const [methodOrField, value] of Object.entries(sourcePathItem)) {
      if (!(methodOrField in targetPathItem)) {
        targetPathItem[methodOrField] = value;
        continue;
      }

      if (!isSameJson(targetPathItem[methodOrField], value)) {
        throw new Error(
          `OpenAPI path conflict at ${route}.${methodOrField}. Rename one operation or use a single OPENAPI_INPUT.`,
        );
      }
    }
  }
}

function renameConflictingComponents(targetComponents, sourceSpec, input) {
  const sourceComponents = sourceSpec.components ?? {};
  const refReplacements = new Map();

  for (const [section, sourceSection] of Object.entries(sourceComponents)) {
    if (!isPlainObject(sourceSection)) {
      continue;
    }

    const targetSection = targetComponents[section];

    if (!isPlainObject(targetSection)) {
      continue;
    }

    for (const [name, value] of Object.entries(sourceSection)) {
      if (!(name in targetSection) || isSameJson(targetSection[name], value)) {
        continue;
      }

      const nextName = createUniqueComponentName(
        createComponentPrefix(input),
        name,
        targetSection,
        sourceSection,
      );
      const previousRef = `#/components/${section}/${name}`;
      const nextRef = `#/components/${section}/${nextName}`;

      sourceSection[nextName] = value;
      delete sourceSection[name];
      refReplacements.set(previousRef, nextRef);

      console.warn(
        `Renamed conflicting OpenAPI component "${previousRef}" to "${nextRef}" from ${input}.`,
      );
    }
  }

  if (refReplacements.size > 0) {
    replaceRefs(sourceSpec, refReplacements);
  }
}

function createUniqueComponentName(prefix, name, targetSection, sourceSection) {
  let nextName = `${prefix}${name}`;
  let counter = 2;

  while (nextName in targetSection || nextName in sourceSection) {
    nextName = `${prefix}${name}${counter}`;
    counter += 1;
  }

  return nextName;
}

function createComponentPrefix(input) {
  if (isHttpUrl(input)) {
    return "Api";
  }

  const basename = path.basename(input, path.extname(input));

  return `${toPascalCase(basename)}Api`;
}

function replaceRefs(value, replacements) {
  if (Array.isArray(value)) {
    for (const item of value) {
      replaceRefs(item, replacements);
    }

    return;
  }

  if (!isPlainObject(value)) {
    return;
  }

  if (typeof value.$ref === "string" && replacements.has(value.$ref)) {
    value.$ref = replacements.get(value.$ref);
  }

  for (const nestedValue of Object.values(value)) {
    replaceRefs(nestedValue, replacements);
  }
}

function mergeComponents(targetComponents, sourceComponents) {
  for (const [section, sourceSection] of Object.entries(sourceComponents)) {
    if (!isPlainObject(sourceSection)) {
      if (!(section in targetComponents)) {
        targetComponents[section] = sourceSection;
        continue;
      }

      if (!isSameJson(targetComponents[section], sourceSection)) {
        throw new Error(`OpenAPI component conflict at components.${section}.`);
      }

      continue;
    }

    const targetSection = targetComponents[section] ?? {};
    targetComponents[section] = targetSection;

    for (const [name, value] of Object.entries(sourceSection)) {
      if (!(name in targetSection)) {
        targetSection[name] = value;
        continue;
      }

      if (!isSameJson(targetSection[name], value)) {
        throw new Error(
          `OpenAPI component conflict at components.${section}.${name}.`,
        );
      }
    }
  }
}

function dedupeOperationIds(paths) {
  const operationIds = new Map();

  for (const [route, pathItem] of Object.entries(paths)) {
    for (const [method, operation] of Object.entries(pathItem)) {
      let operationId = operation?.operationId;

      if (!operationId) {
        continue;
      }

      const previousLocation = operationIds.get(operationId);
      const currentLocation = `${method.toUpperCase()} ${route}`;

      if (previousLocation) {
        const previousOperationId = operationId;
        const nextOperationId = createUniqueOperationId(
          operationId,
          route,
          operationIds,
        );

        operation.operationId = nextOperationId;
        operationId = nextOperationId;

        console.warn(
          `Renamed duplicate OpenAPI operationId "${previousOperationId}" to "${nextOperationId}" for ${currentLocation}; original was already used at ${previousLocation}.`,
        );
      }

      operationIds.set(operationId, currentLocation);
    }
  }
}

function createUniqueOperationId(operationId, route, operationIds) {
  const routePrefix = getRouteOperationPrefix(route);
  const baseOperationId = routePrefix
    ? `${routePrefix}${capitalize(operationId)}`
    : `${operationId}Duplicate`;
  let nextOperationId = baseOperationId;
  let counter = 2;

  while (operationIds.has(nextOperationId)) {
    nextOperationId = `${baseOperationId}${counter}`;
    counter += 1;
  }

  return nextOperationId;
}

function getRouteOperationPrefix(route) {
  const [, apiSegment, firstResource] = route.split("/");

  if (apiSegment !== "api" || !firstResource) {
    return undefined;
  }

  return toCamelCase(firstResource);
}

function toCamelCase(value) {
  const words = value.split(/[^a-zA-Z0-9]+/).filter(Boolean);

  return words
    .map((word, index) => {
      const normalizedWord = word.toLowerCase();
      return index === 0 ? normalizedWord : capitalize(normalizedWord);
    })
    .join("");
}

function capitalize(value) {
  return `${value.charAt(0).toUpperCase()}${value.slice(1)}`;
}

function toPascalCase(value) {
  const camelCase = toCamelCase(value);

  return capitalize(camelCase);
}

function uniqueTags(tags) {
  const byName = new Map();

  for (const tag of tags) {
    const key = tag.name ?? JSON.stringify(tag);

    if (!byName.has(key)) {
      byName.set(key, tag);
    }
  }

  return [...byName.values()];
}

function uniqueByJson(values) {
  const seen = new Set();
  const uniqueValues = [];

  for (const value of values) {
    const key = JSON.stringify(value);

    if (seen.has(key)) {
      continue;
    }

    seen.add(key);
    uniqueValues.push(value);
  }

  return uniqueValues;
}

function isHttpUrl(value) {
  return value.startsWith("http://") || value.startsWith("https://");
}

function isPlainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function isSameJson(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
}

function formatError(error) {
  if (!(error instanceof Error)) {
    return String(error);
  }

  const cause = error.cause;

  if (cause instanceof Error) {
    return `${error.message}; ${cause.message}`;
  }

  return error.message;
}
