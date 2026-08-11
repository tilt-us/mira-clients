export type MiraEnvironment = "dev" | "staging" | "prod";

export type EnvironmentConfig = {
  websiteUrl: string;
  servicesApiUrl: string;
  authIssuerUrl: string;
  updateManifestUrl: string | null;
  cdnBaseUrl: string | null;
};

export function getMiraEnvironment(
  value?: string,
  defaultEnvironment?: MiraEnvironment,
): MiraEnvironment;
export function getEnvironmentConfig(environment: MiraEnvironment): EnvironmentConfig;
export function getServiceUrl(environmentConfig: EnvironmentConfig, service: string): string;
