import environmentDefinitions from "../../../mira-environments.json";

export type MiraEnvironment = keyof typeof environmentDefinitions;

export type EnvironmentConfig = (typeof environmentDefinitions)[MiraEnvironment] & {
  environment: MiraEnvironment;
};

declare const __MIRA_ENV__: MiraEnvironment;

const environmentNames = Object.keys(environmentDefinitions) as MiraEnvironment[];

export function getEnvironmentConfig(value: string): EnvironmentConfig {
  const environment = value.trim().toLowerCase();

  if (!isMiraEnvironment(environment)) {
    throw new Error(
      `Invalid MIRA_ENV=${JSON.stringify(value)}. Use one of: ${environmentNames.join(", ")}.`,
    );
  }

  return {
    environment,
    ...environmentDefinitions[environment],
  };
}

export function getBuildEnvironmentConfig() {
  return getEnvironmentConfig(__MIRA_ENV__);
}

export function getServiceUrl(config: EnvironmentConfig, service: string) {
  // The public auth API is routed at the services origin (`/api/...`), while
  // the other service names remain path-prefixed behind the shared gateway.
  if (service === "auth") {
    return config.servicesApiUrl.replace(/\/$/, "");
  }

  return new URL(service, `${config.servicesApiUrl}/`).toString().replace(/\/$/, "");
}

function isMiraEnvironment(value: string): value is MiraEnvironment {
  return environmentNames.includes(value as MiraEnvironment);
}
