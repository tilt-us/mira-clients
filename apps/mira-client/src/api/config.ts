import {
  getBuildEnvironmentConfig,
  getEnvironmentConfig,
  getServiceUrl,
  type MiraEnvironment,
} from "../environment";

export type EnvironmentRuntimeConfig = {
  environment: MiraEnvironment;
};

const defaultConfig = getBuildEnvironmentConfig();

export let API_ENVIRONMENT = defaultConfig.environment;
export let API_BASE_URL = getServiceUrl(defaultConfig, "auth");
export let LIVE_API_BASE_URL = getServiceUrl(defaultConfig, "live");
export let MATCHMAKING_API_BASE_URL = getServiceUrl(defaultConfig, "match");
export let CHAMPION_API_BASE_URL = getServiceUrl(defaultConfig, "game");
export let CHAT_API_BASE_URL = getServiceUrl(defaultConfig, "chat");

export function applyApiRuntimeConfig(config: EnvironmentRuntimeConfig) {
  const environment = getEnvironmentConfig(config.environment);

  API_ENVIRONMENT = environment.environment;
  API_BASE_URL = getServiceUrl(environment, "auth");
  LIVE_API_BASE_URL = getServiceUrl(environment, "live");
  MATCHMAKING_API_BASE_URL = getServiceUrl(environment, "match");
  CHAMPION_API_BASE_URL = getServiceUrl(environment, "game");
  CHAT_API_BASE_URL = getServiceUrl(environment, "chat");
}
