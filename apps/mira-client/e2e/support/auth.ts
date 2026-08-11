import { getEnvironmentConfig, getMiraEnvironment } from "../../scripts/environment.mjs";

type LoginTarget = "dev";

export type TestCredentials = {
  email: string;
  password: string;
  target: LoginTarget;
};

const defaultCredentialsByTarget: Record<
  LoginTarget,
  { email: string; password: string }
> = {
  dev: {
    email: "test2@mira.de",
    password: "geebeeteeklee",
  },
};

export function getLoginTarget(): LoginTarget {
  return "dev";
}

export function getCredentials(): TestCredentials {
  const target = getLoginTarget();

  return {
    target,
    email: process.env.E2E_LOGIN_EMAIL ?? defaultCredentialsByTarget[target].email,
    password:
      process.env.E2E_LOGIN_PASSWORD ?? defaultCredentialsByTarget[target].password,
  };
}

export function shouldUseRealKeycloakLogin() {
  return process.env.E2E_REAL_LOGIN === "1";
}

export function getKeycloakIssuerUrl() {
  return getEnvironmentConfig(
    getMiraEnvironment(process.env.MIRA_ENV, "dev"),
  ).authIssuerUrl;
}

export function createUnsignedJwt(payload: Record<string, unknown>) {
  return [
    base64UrlEncode({ alg: "none", typ: "JWT" }),
    base64UrlEncode(payload),
    "e2e",
  ].join(".");
}

function base64UrlEncode(value: Record<string, unknown>) {
  return Buffer.from(JSON.stringify(value))
    .toString("base64")
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
}
