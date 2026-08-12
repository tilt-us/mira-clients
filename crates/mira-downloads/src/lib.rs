use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
    str::FromStr,
    time::{Duration, Instant},
};

pub const SCHEMA_VERSION: u32 = 1;
pub const CONTENT_SCHEMA_VERSION: u32 = 2;
pub const DOWNLOADS_BASE_URL: &str = "https://downloads.tilt-us.com";
pub const INSTALL_ROOT_ENV: &str = "MIRA_INSTALL_ROOT";
const INSTALL_ROOT_STATE_ENV: &str = "MIRA_INSTALL_ROOT_STATE";
const INSTALL_ROOT_STATE_PREFIX: &str = "install-root";
const DOWNLOAD_BUFFER_SIZE: usize = 1024 * 1024;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

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
    pub ui: ContentArtifact,
    pub game: ContentArtifact,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentArtifact {
    pub url: String,
    pub sha256: String,
    pub size: u64,
    pub content_id: String,
}

impl ContentManifest {
    pub fn validate_for(&self, expected_environment: Environment) -> Result<(), String> {
        if self.schema_version != CONTENT_SCHEMA_VERSION {
            return Err(format!(
                "Unsupported content manifest schemaVersion={}; expected {CONTENT_SCHEMA_VERSION}.",
                self.schema_version
            ));
        }
        if self.environment != expected_environment {
            return Err(format!(
                "Content manifest environment={} does not match this {} build.",
                self.environment.as_str(),
                expected_environment.as_str()
            ));
        }
        self.ui.as_artifact().validate("content.ui")?;
        self.game.as_artifact().validate("content.game")
    }
}

impl ContentArtifact {
    pub fn as_artifact(&self) -> Artifact {
        Artifact {
            url: self.url.clone(),
            sha256: self.sha256.clone(),
            size: self.size,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}

impl DownloadProgress {
    pub fn percent(self) -> u8 {
        if self.total_bytes == 0 {
            0
        } else {
            ((self.downloaded_bytes.saturating_mul(100) / self.total_bytes).min(100)) as u8
        }
    }
}

/// Streams an artifact to a sibling `.part` file, verifies it, then atomically
/// replaces the requested destination. Callers own extraction and activation.
pub fn download_artifact<F>(
    client: &Client,
    artifact: &Artifact,
    destination: &Path,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(DownloadProgress),
{
    artifact.validate("download artifact")?;
    let parent = destination
        .parent()
        .ok_or_else(|| "Download destination has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create download directory: {error}"))?;
    let temporary = part_path(destination)?;
    let _ = fs::remove_file(&temporary);

    let mut response = client
        .get(&artifact.url)
        .send()
        .map_err(|error| format!("Could not download artifact: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Artifact request failed: {error}"))?;
    let total_bytes = artifact.size;
    let file = File::create(&temporary)
        .map_err(|error| format!("Could not create download file: {error}"))?;
    let mut file = BufWriter::with_capacity(DOWNLOAD_BUFFER_SIZE, file);
    let mut hasher = Sha256::new();
    let mut downloaded_bytes = 0_u64;
    let mut buffer = vec![0_u8; DOWNLOAD_BUFFER_SIZE];
    let mut last_progress = Instant::now() - PROGRESS_INTERVAL;
    on_progress(DownloadProgress {
        downloaded_bytes,
        total_bytes,
    });

    let result = (|| -> Result<(), String> {
        loop {
            let bytes = response
                .read(&mut buffer)
                .map_err(|error| format!("Could not read artifact download: {error}"))?;
            if bytes == 0 {
                break;
            }
            file.write_all(&buffer[..bytes])
                .map_err(|error| format!("Could not write artifact download: {error}"))?;
            hasher.update(&buffer[..bytes]);
            downloaded_bytes += bytes as u64;
            if last_progress.elapsed() >= PROGRESS_INTERVAL {
                on_progress(DownloadProgress {
                    downloaded_bytes,
                    total_bytes,
                });
                last_progress = Instant::now();
            }
        }
        file.flush()
            .map_err(|error| format!("Could not flush artifact download: {error}"))?;
        verify_download(artifact, downloaded_bytes, &to_hex(&hasher.finalize()))?;
        on_progress(DownloadProgress {
            downloaded_bytes,
            total_bytes,
        });
        replace_file(&temporary, destination)
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub fn download_client() -> Result<Client, String> {
    Client::builder()
        .user_agent("mira-downloads")
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(6 * 60 * 60))
        .build()
        .map_err(|error| format!("Could not create download HTTP client: {error}"))
}

/// Returns the installation root saved by the Mira Installer for this environment.
///
/// Packaged clients do not always run next to mutable content. For example,
/// Debian packages place the executable in `/usr/bin` while the installer keeps
/// `assets/ui` in the user-selected installation directory.
pub fn recorded_install_root(environment: Environment) -> Option<PathBuf> {
    let state_path = install_root_state_path(environment).ok()?;
    read_install_root_state(&state_path).ok().flatten()
}

/// Saves the root containing `assets/ui` and `assets/game` for a deployment environment.
pub fn record_install_root(
    environment: Environment,
    install_root: &Path,
) -> Result<PathBuf, String> {
    let install_root = absolute_path(install_root)?;
    if !install_root.is_dir() {
        return Err(format!(
            "Mira installation root does not exist: {}",
            install_root.display()
        ));
    }
    let state_path = install_root_state_path(environment)?;
    write_install_root_state(&state_path, &install_root)?;
    Ok(install_root)
}

fn install_root_state_path(environment: Environment) -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os(INSTALL_ROOT_STATE_ENV) {
        return Ok(PathBuf::from(path));
    }

    Ok(mira_data_dir()?
        .join("Mira Games")
        .join("Mira Moba")
        .join(format!(
            "{INSTALL_ROOT_STATE_PREFIX}-{}",
            environment.as_str()
        )))
}

#[cfg(target_os = "windows")]
fn mira_data_dir() -> Result<PathBuf, String> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join("AppData/Local"))
        })
        .ok_or_else(|| {
            "Could not determine the Windows local application data directory".to_string()
        })
}

#[cfg(target_os = "macos")]
fn mira_data_dir() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join("Library/Application Support"))
        .ok_or_else(|| "Could not determine the macOS application support directory".to_string())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn mira_data_dir() -> Result<PathBuf, String> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .ok_or_else(|| "Could not determine the Linux data directory".to_string())
}

#[cfg(not(any(unix, target_os = "windows")))]
fn mira_data_dir() -> Result<PathBuf, String> {
    std::env::current_dir()
        .map_err(|error| format!("Could not determine the Mira data directory: {error}"))
}

fn read_install_root_state(path: &Path) -> Result<Option<PathBuf>, String> {
    let value = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Could not read Mira installation state: {error}")),
    };
    let root = PathBuf::from(value.trim());
    if root.as_os_str().is_empty() || !root.is_absolute() || !root.is_dir() {
        return Ok(None);
    }
    Ok(Some(root))
}

fn write_install_root_state(path: &Path, install_root: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Mira installation state has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create Mira installation state directory: {error}"))?;
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(&temporary, format!("{}\n", install_root.display()))
        .map_err(|error| format!("Could not write Mira installation state: {error}"))?;
    replace_file(&temporary, path)
        .map_err(|error| format!("Could not activate Mira installation state: {error}"))
}

fn absolute_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    std::env::current_dir()
        .map(|directory| directory.join(path))
        .map_err(|error| format!("Could not resolve Mira installation root: {error}"))
}

pub fn verify_download(
    artifact: &Artifact,
    downloaded: u64,
    actual_sha256: &str,
) -> Result<(), String> {
    if downloaded != artifact.size {
        return Err(format!(
            "Download size mismatch: expected {} bytes, got {downloaded}",
            artifact.size
        ));
    }
    if !artifact.sha256.eq_ignore_ascii_case(actual_sha256) {
        return Err("Download checksum mismatch".to_string());
    }
    Ok(())
}

pub fn part_path(destination: &Path) -> Result<PathBuf, String> {
    let filename = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Download destination has no filename".to_string())?;
    Ok(destination.with_file_name(format!("{filename}.part")))
}

fn replace_file(temporary: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        fs::remove_file(destination)
            .map_err(|error| format!("Could not replace previous download: {error}"))?;
    }
    fs::rename(temporary, destination)
        .map_err(|error| format!("Could not finalize artifact download: {error}"))
}

fn to_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
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

    #[test]
    fn requires_independent_v2_ui_and_game_content() {
        let manifest: ContentManifest = serde_json::from_str(
            r#"{"schemaVersion":2,"environment":"dev","ui":{"url":"https://downloads.tilt-us.com/dev/content/ui.zip","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","size":1,"contentId":"ui"},"game":{"url":"https://downloads.tilt-us.com/dev/content/game.zip","sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","size":2,"contentId":"game"}}"#,
        )
        .unwrap();
        manifest.validate_for(Environment::Dev).unwrap();
        assert_eq!(manifest.ui.content_id, "ui");
        assert_eq!(manifest.game.content_id, "game");
        assert!(
            serde_json::from_str::<ContentManifest>(
                r#"{"schemaVersion":1,"environment":"dev","content":{}}"#,
            )
            .is_err()
        );
    }

    #[test]
    fn install_root_state_round_trips_an_existing_absolute_root() {
        let directory = tempfile::tempdir().unwrap();
        let state_path = directory.path().join("state/install-root");
        let install_root = directory.path().join("Mira Moba");
        fs::create_dir_all(&install_root).unwrap();

        write_install_root_state(&state_path, &install_root).unwrap();

        assert_eq!(
            read_install_root_state(&state_path).unwrap(),
            Some(install_root)
        );
    }

    #[test]
    fn install_root_state_ignores_invalid_roots() {
        let directory = tempfile::tempdir().unwrap();
        let state_path = directory.path().join("install-root");

        fs::write(&state_path, "relative/path\n").unwrap();
        assert_eq!(read_install_root_state(&state_path).unwrap(), None);

        fs::write(
            &state_path,
            directory.path().join("missing").display().to_string(),
        )
        .unwrap();
        assert_eq!(read_install_root_state(&state_path).unwrap(), None);
    }
}
