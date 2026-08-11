import { describe, expect, test } from "vitest";
import { getEnvironmentConfig, getServiceUrl } from "../src/environment";

describe("environment configuration", () => {
  test.each([
    [
      "dev",
      "https://dev.tilt-us.com",
      "https://dev-api.tilt-us.com",
      "https://dev-api.tilt-us.com/keycloak/realms/mira",
    ],
    [
      "staging",
      "https://staging.tilt-us.com",
      "https://staging-api.tilt-us.com",
      "https://staging-api.tilt-us.com/keycloak/realms/mira",
    ],
    [
      "prod",
      "https://tilt-us.com",
      "https://api.tilt-us.com",
      "https://api.tilt-us.com/keycloak/realms/mira",
    ],
  ] as const)(
    "maps %s to its public endpoints",
    (environment, websiteUrl, servicesApiUrl, authIssuerUrl) => {
      const config = getEnvironmentConfig(environment);

      expect(config).toMatchObject({
        authIssuerUrl,
        environment,
        servicesApiUrl,
        websiteUrl,
      });
      expect(getServiceUrl(config, "auth")).toBe(servicesApiUrl);
      expect(getServiceUrl(config, "match")).toBe(`${servicesApiUrl}/match`);
    },
  );

  test("rejects an unknown environment", () => {
    expect(() => getEnvironmentConfig("preview")).toThrow(
      "Invalid MIRA_ENV",
    );
  });
});
