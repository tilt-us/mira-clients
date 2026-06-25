type LoginTarget = "dev" | "local";

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
  local: {
    email: "test@mira.de",
    password: "geebeeteeklee",
  },
};

export function getLoginTarget(): LoginTarget {
  return process.env.E2E_TARGET === "local" ? "local" : "dev";
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

export function getKeycloakIssuerUrl() {
  const baseUrl =
    process.env.VITE_KEYCLOAK_BASE_URL ??
    (getLoginTarget() === "dev"
      ? "https://api.tilt-us.com/keycloak"
      : "http://localhost:8081");
  const realm = process.env.VITE_KEYCLOAK_REALM ?? "mira";

  return `${baseUrl.replace(/\/$/, "")}/realms/${realm}`;
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
