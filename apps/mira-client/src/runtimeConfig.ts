import { invoke, isTauri } from "@tauri-apps/api/core";
import { applyApiRuntimeConfig, type ApiRuntimeConfig } from "./api/config";
import {
  applyKeycloakRuntimeConfig,
  type KeycloakRuntimeConfig,
} from "./auth/config";
import {
  applyAuthStorageRuntimeConfig,
  type AuthStorageRuntimeConfig,
} from "./auth/storage";

export type ClientRuntimeConfig = ApiRuntimeConfig &
  KeycloakRuntimeConfig &
  AuthStorageRuntimeConfig;

export async function loadRuntimeConfig() {
  if (!isTauri()) {
    return;
  }

  const config = await invoke<ClientRuntimeConfig>("client_config");
  applyApiRuntimeConfig(config);
  applyKeycloakRuntimeConfig(config);
  applyAuthStorageRuntimeConfig(config);
}
