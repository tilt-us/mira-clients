import environmentDefinitions from "../../../mira-environments.json" with { type: "json" };

const environmentNames = Object.keys(environmentDefinitions);

export function getMiraEnvironment(value, defaultEnvironment) {
  const environment = (value ?? defaultEnvironment)?.trim().toLowerCase();

  if (!environment) {
    throw new Error(
      `MIRA_ENV is required. Use one of: ${environmentNames.join(", ")}.`,
    );
  }

  if (!Object.hasOwn(environmentDefinitions, environment)) {
    throw new Error(
      `Invalid MIRA_ENV=${JSON.stringify(value)}. Use one of: ${environmentNames.join(", ")}.`,
    );
  }

  return environment;
}

export function getEnvironmentConfig(environment) {
  return environmentDefinitions[getMiraEnvironment(environment)];
}

export function getServiceUrl(environmentConfig, service) {
  if (service === "auth") {
    return environmentConfig.servicesApiUrl.replace(/\/$/, "");
  }

  return new URL(service, `${environmentConfig.servicesApiUrl}/`).toString().replace(/\/$/, "");
}
