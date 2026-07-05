const DEFAULT_API_BASE_URL = "http://localhost:8080";
const DEFAULT_LIVE_API_BASE_URL = "http://localhost:8082";
const DEFAULT_MATCHMAKING_API_BASE_URL = "http://localhost:8083";
const DEFAULT_CHAMPION_API_BASE_URL = "http://localhost:8084";
const DEFAULT_CHAT_API_BASE_URL = "http://localhost:8085";

export type ApiRuntimeConfig = {
  apiBaseUrl?: string;
  liveApiBaseUrl?: string;
  matchmakingApiBaseUrl?: string;
  championApiBaseUrl?: string;
  chatApiBaseUrl?: string;
};

export let API_BASE_URL = normalizeBaseUrl(
  import.meta.env.VITE_API_BASE_URL ?? DEFAULT_API_BASE_URL,
);

export let LIVE_API_BASE_URL = normalizeBaseUrl(
  import.meta.env.VITE_LIVE_API_BASE_URL ?? DEFAULT_LIVE_API_BASE_URL,
);

export let MATCHMAKING_API_BASE_URL = normalizeBaseUrl(
  import.meta.env.VITE_MATCHMAKING_API_BASE_URL ??
    DEFAULT_MATCHMAKING_API_BASE_URL,
);

export let CHAMPION_API_BASE_URL = normalizeBaseUrl(
  import.meta.env.VITE_CHAMPION_API_BASE_URL ?? DEFAULT_CHAMPION_API_BASE_URL,
);

export let CHAT_API_BASE_URL = normalizeBaseUrl(
  import.meta.env.VITE_CHAT_API_BASE_URL ?? DEFAULT_CHAT_API_BASE_URL,
);

export function applyApiRuntimeConfig(config: ApiRuntimeConfig) {
  API_BASE_URL = normalizeBaseUrl(config.apiBaseUrl ?? API_BASE_URL);
  LIVE_API_BASE_URL = normalizeBaseUrl(config.liveApiBaseUrl ?? LIVE_API_BASE_URL);
  MATCHMAKING_API_BASE_URL = normalizeBaseUrl(
    config.matchmakingApiBaseUrl ?? MATCHMAKING_API_BASE_URL,
  );
  CHAMPION_API_BASE_URL = normalizeBaseUrl(
    config.championApiBaseUrl ?? CHAMPION_API_BASE_URL,
  );
  CHAT_API_BASE_URL = normalizeBaseUrl(config.chatApiBaseUrl ?? CHAT_API_BASE_URL);
}

function normalizeBaseUrl(baseUrl: string) {
  return baseUrl.trim().replace(/\/$/, "");
}
