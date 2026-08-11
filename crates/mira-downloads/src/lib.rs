use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub const SCHEMA_VERSION: u32 = 1;
pub const DOWNLOADS_BASE_URL: &str = "https://downloads.tilt-us.com";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Dev,
    Staging,
    Prod,
}

impl Environment {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dev => "dev",
            Self::Staging => "staging",
            Self::Prod => "prod",
        }
    }

    pub const fn garage_prefix(self) -> &'static str {
        match self {
            Self::Dev => "dev/",
            Self::Staging => "staging/",
            Self::Prod => "",
        }
    }

    pub fn latest_manifest_url(self) -> String {
        format!("{}/{}latest.json", DOWNLOADS_BASE_URL, self.garage_prefix())
    }
}

impl FromStr for Environment {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "dev" => Ok(Self::Dev),
            "staging" => Ok(Self::Staging),
            "prod" => Ok(Self::Prod),
            _ => Err(format!(
                "Invalid MIRA_ENV={value:?}. Use one of: dev, staging, prod."
            )),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitMetadata {
    pub commit: String,
    pub tag: String,
    pub version: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestManifest {
    pub schema_version: u32,
    pub environment: Environment,
    pub git: GitMetadata,
    pub published_at: String,
    pub installer_manifest_url: String,
    pub runtime_manifest_url: String,
    pub content_manifest_url: String,
}

impl LatestManifest {
    pub fn validate_for(&self, expected_environment: Environment) -> Result<(), String> {
        validate_manifest_header(self.schema_version, self.environment, expected_environment)?;
        validate_url("installerManifestUrl", &self.installer_manifest_url)?;
        validate_url("runtimeManifestUrl", &self.runtime_manifest_url)?;
        validate_url("contentManifestUrl", &self.content_manifest_url)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Artifact {
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

impl Artifact {
    pub fn validate(&self, name: &str) -> Result<(), String> {
        validate_url(name, &self.url)?;
        if self.sha256.len() != 64 || !self.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!("{name} has an invalid SHA-256 checksum"));
        }
        if self.size == 0 {
            return Err(format!("{name} has an invalid size"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinuxDesktopArtifacts {
    pub app_image: Artifact,
    pub deb: Artifact,
    pub rpm: Artifact,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DesktopArtifacts {
    pub windows: Artifact,
    pub linux: LinuxDesktopArtifacts,
    pub macos: Artifact,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PlatformArtifacts {
    pub windows: Artifact,
    pub linux: Artifact,
    pub macos: Artifact,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeManifest {
    pub schema_version: u32,
    pub environment: Environment,
    pub desktop: DesktopArtifacts,
    pub game_client: PlatformArtifacts,
}

impl RuntimeManifest {
    pub fn validate_for(&self, expected_environment: Environment) -> Result<(), String> {
        validate_manifest_header(self.schema_version, self.environment, expected_environment)?;
        self.desktop.windows.validate("desktop.windows")?;
        self.desktop
            .linux
            .app_image
            .validate("desktop.linux.appImage")?;
        self.desktop.linux.deb.validate("desktop.linux.deb")?;
        self.desktop.linux.rpm.validate("desktop.linux.rpm")?;
        self.desktop.macos.validate("desktop.macos")?;
        self.game_client.windows.validate("gameClient.windows")?;
        self.game_client.linux.validate("gameClient.linux")?;
        self.game_client.macos.validate("gameClient.macos")
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentManifest {
    pub schema_version: u32,
    pub environment: Environment,
    pub content: ContentArtifact,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentArtifact {
    pub url: String,
    pub sha256: String,
    pub size: u64,
    #[serde(default)]
    pub content_id: Option<String>,
}

impl ContentManifest {
    pub fn validate_for(&self, expected_environment: Environment) -> Result<(), String> {
        validate_manifest_header(self.schema_version, self.environment, expected_environment)?;
        Artifact {
            url: self.content.url.clone(),
            sha256: self.content.sha256.clone(),
            size: self.content.size,
        }
        .validate("content")
    }
}

fn validate_manifest_header(
    schema_version: u32,
    environment: Environment,
    expected_environment: Environment,
) -> Result<(), String> {
    if schema_version != SCHEMA_VERSION {
        return Err(format!(
            "Unsupported download manifest schemaVersion={schema_version}; expected {SCHEMA_VERSION}."
        ));
    }
    if environment != expected_environment {
        return Err(format!(
            "Download manifest environment={} does not match this {} build.",
            environment.as_str(),
            expected_environment.as_str()
        ));
    }
    Ok(())
}

fn validate_url(name: &str, value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.starts_with("https://") || value.starts_with("http://") {
        return Ok(());
    }
    Err(format!("{name} must be an absolute HTTP(S) URL"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_latest_urls_without_a_production_fallback() {
        assert_eq!(
            Environment::Dev.latest_manifest_url(),
            "https://downloads.tilt-us.com/dev/latest.json"
        );
        assert_eq!(
            Environment::Staging.latest_manifest_url(),
            "https://downloads.tilt-us.com/staging/latest.json"
        );
        assert_eq!(
            Environment::Prod.latest_manifest_url(),
            "https://downloads.tilt-us.com/latest.json"
        );
        assert!("preview".parse::<Environment>().is_err());
        assert!("".parse::<Environment>().is_err());
    }
}
