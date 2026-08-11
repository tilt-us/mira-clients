use reqwest::Url;
use serde::Deserialize;

const ENVIRONMENT_DEFINITIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../mira-environments.json"
));

/// Identifies a supported Mira deployment environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Dev,
    Staging,
    Prod,
}

/// Holds validated public endpoints for a single deployment environment.
#[derive(Debug, Clone)]
pub struct EnvironmentConfig {
    pub environment: Environment,
    pub website_url: Url,
    pub services_api_url: Url,
    pub auth_issuer_url: Url,
    pub update_manifest_url: Option<Url>,
    pub cdn_base_url: Option<Url>,
    auth_api_url: Url,
}

#[derive(Debug, Deserialize)]
struct EnvironmentDefinitions {
    dev: RawEnvironmentConfig,
    staging: RawEnvironmentConfig,
    prod: RawEnvironmentConfig,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawEnvironmentConfig {
    website_url: String,
    services_api_url: String,
    auth_issuer_url: String,
    update_manifest_url: Option<String>,
    cdn_base_url: Option<String>,
}

impl Environment {
    /// Returns the environment embedded by the Cargo build script.
    pub fn from_build() -> Result<Self, String> {
        Self::from_value(option_env!("MIRA_ENV"))
    }

    fn from_value(value: Option<&str>) -> Result<Self, String> {
        match value {
            Some("dev") => Ok(Self::Dev),
            Some("staging") => Ok(Self::Staging),
            Some("prod") => Ok(Self::Prod),
            Some(value) => Err(format!(
                "Invalid embedded MIRA_ENV={value:?}. Use one of: dev, staging, prod."
            )),
            None => Err(
                "MIRA_ENV was not embedded in this build. Build with MIRA_ENV=dev, MIRA_ENV=staging, or MIRA_ENV=prod."
                    .to_string(),
            ),
        }
    }
}

impl EnvironmentConfig {
    /// Parses the central environment definition selected for this build.
    pub fn from_build() -> Result<Self, String> {
        let definitions = serde_json::from_str::<EnvironmentDefinitions>(ENVIRONMENT_DEFINITIONS)
            .map_err(|error| {
            format!("Could not parse central environment configuration: {error}")
        })?;
        let environment = Environment::from_build()?;
        let definition = match environment {
            Environment::Dev => definitions.dev,
            Environment::Staging => definitions.staging,
            Environment::Prod => definitions.prod,
        };

        Self::from_raw(environment, definition)
    }

    /// Returns the auth service URL used to validate launch access tokens.
    pub fn auth_api_url(&self) -> &Url {
        &self.auth_api_url
    }

    fn from_raw(
        environment: Environment,
        definition: RawEnvironmentConfig,
    ) -> Result<Self, String> {
        let website_url = parse_url("websiteUrl", &definition.website_url)?;
        let services_api_url = parse_url("servicesApiUrl", &definition.services_api_url)?;
        let auth_issuer_url = parse_url("authIssuerUrl", &definition.auth_issuer_url)?;
        let auth_api_url = services_api_url
            .join("auth")
            .map_err(|error| format!("Could not derive auth API URL: {error}"))?;

        Ok(Self {
            environment,
            website_url,
            services_api_url,
            auth_issuer_url,
            update_manifest_url: parse_optional_url(
                "updateManifestUrl",
                definition.update_manifest_url.as_deref(),
            )?,
            cdn_base_url: parse_optional_url("cdnBaseUrl", definition.cdn_base_url.as_deref())?,
            auth_api_url,
        })
    }
}

fn parse_url(name: &str, value: &str) -> Result<Url, String> {
    Url::parse(value)
        .map_err(|error| format!("Invalid {name} in central environment configuration: {error}"))
}

fn parse_optional_url(name: &str, value: Option<&str>) -> Result<Option<Url>, String> {
    value.map(|value| parse_url(name, value)).transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_config() -> RawEnvironmentConfig {
        RawEnvironmentConfig {
            website_url: "https://tilt-us.com".to_string(),
            services_api_url: "https://api.tilt-us.com".to_string(),
            auth_issuer_url: "https://api.tilt-us.com/keycloak/realms/mira".to_string(),
            update_manifest_url: None,
            cdn_base_url: None,
        }
    }

    fn config(environment: Environment) -> EnvironmentConfig {
        let definitions = serde_json::from_str::<EnvironmentDefinitions>(ENVIRONMENT_DEFINITIONS)
            .expect("the checked-in environment configuration must be valid JSON");
        let definition = match environment {
            Environment::Dev => definitions.dev,
            Environment::Staging => definitions.staging,
            Environment::Prod => definitions.prod,
        };

        EnvironmentConfig::from_raw(environment, definition)
            .expect("the checked-in environment URLs must be valid")
    }

    #[test]
    fn loads_the_environment_embedded_by_the_build_script() {
        let environment = Environment::from_build().expect("the build script must embed MIRA_ENV");
        let config =
            EnvironmentConfig::from_build().expect("the embedded environment must be valid");

        assert_eq!(config.environment, environment);
    }

    #[test]
    fn parses_supported_environment_values() {
        assert_eq!(Environment::from_value(Some("dev")), Ok(Environment::Dev));
        assert_eq!(
            Environment::from_value(Some("staging")),
            Ok(Environment::Staging)
        );
        assert_eq!(Environment::from_value(Some("prod")), Ok(Environment::Prod));
    }

    #[test]
    fn rejects_missing_or_invalid_environment_values() {
        assert!(
            Environment::from_value(None)
                .unwrap_err()
                .contains("MIRA_ENV was not embedded")
        );
        assert!(
            Environment::from_value(Some("preview"))
                .unwrap_err()
                .contains("Invalid embedded MIRA_ENV")
        );
    }

    #[test]
    fn preserves_optional_update_endpoints() {
        let mut definition = raw_config();
        definition.update_manifest_url =
            Some("https://updates.tilt-us.com/manifest.json".to_string());
        definition.cdn_base_url = Some("https://cdn.tilt-us.com".to_string());

        let config = EnvironmentConfig::from_raw(Environment::Dev, definition).unwrap();

        assert_eq!(
            config.update_manifest_url.unwrap().as_str(),
            "https://updates.tilt-us.com/manifest.json"
        );
        assert_eq!(
            config.cdn_base_url.unwrap().as_str(),
            "https://cdn.tilt-us.com/"
        );
    }

    #[test]
    fn rejects_invalid_endpoint_urls() {
        let mut definition = raw_config();
        definition.website_url = "not a URL".to_string();

        let error = EnvironmentConfig::from_raw(Environment::Dev, definition).unwrap_err();

        assert!(error.starts_with("Invalid websiteUrl"));
    }

    #[test]
    fn rejects_services_urls_without_a_path_base() {
        let mut definition = raw_config();
        definition.services_api_url = "mailto:api@tilt-us.com".to_string();

        let error = EnvironmentConfig::from_raw(Environment::Dev, definition).unwrap_err();

        assert!(error.starts_with("Could not derive auth API URL"));
    }

    #[test]
    fn maps_dev_urls() {
        let config = config(Environment::Dev);

        assert_eq!(config.website_url.as_str(), "https://dev.tilt-us.com/");
        assert_eq!(
            config.services_api_url.as_str(),
            "https://dev-api.tilt-us.com/"
        );
        assert_eq!(
            config.auth_issuer_url.as_str(),
            "https://dev-api.tilt-us.com/keycloak/realms/mira"
        );
        assert_eq!(
            config.auth_api_url().as_str(),
            "https://dev-api.tilt-us.com/auth"
        );
    }

    #[test]
    fn maps_staging_urls() {
        let config = config(Environment::Staging);

        assert_eq!(config.website_url.as_str(), "https://staging.tilt-us.com/");
        assert_eq!(
            config.services_api_url.as_str(),
            "https://staging-api.tilt-us.com/"
        );
        assert_eq!(
            config.auth_issuer_url.as_str(),
            "https://staging-api.tilt-us.com/keycloak/realms/mira"
        );
        assert_eq!(
            config.auth_api_url().as_str(),
            "https://staging-api.tilt-us.com/auth"
        );
    }

    #[test]
    fn maps_prod_urls() {
        let config = config(Environment::Prod);

        assert_eq!(config.website_url.as_str(), "https://tilt-us.com/");
        assert_eq!(config.services_api_url.as_str(), "https://api.tilt-us.com/");
        assert_eq!(
            config.auth_issuer_url.as_str(),
            "https://api.tilt-us.com/keycloak/realms/mira"
        );
        assert_eq!(
            config.auth_api_url().as_str(),
            "https://api.tilt-us.com/auth"
        );
    }
}
