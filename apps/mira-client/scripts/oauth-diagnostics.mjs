import { getEnvironmentConfig, getMiraEnvironment, getServiceUrl } from "./environment.mjs";

const environment = getMiraEnvironment(process.env.MIRA_ENV, "dev");
const config = getEnvironmentConfig(environment);

console.log("[mira-client][oauth]");
console.log(`environment=${environment}`);
console.log("clientId=mira-bevy");
console.log(`issuer=${config.authIssuerUrl}`);
console.log(`authApiBaseUrl=${getServiceUrl(config, "auth")}`);
console.log("keycloakRegisteredRedirectUri=http://127.0.0.1");
console.log("nativeRedirectUri=http://127.0.0.1:<ephemeral-port>");
