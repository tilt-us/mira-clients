use crate::config;
use mira_downloads::{Artifact, ContentArtifact, ContentManifest, LatestManifest};
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::Manager;
use zip::ZipArchive;

const CONTENT_DIRECTORY: &str = "content";
const CONTENT_STATE_FILENAME: &str = "content-state.json";

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ContentState {
    sha256: String,
    size: u64,
    installed_at: String,
}

/// Ensures the current build's content artifact is installed before game launch.
pub(crate) fn ensure_content_current(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let environment = config::build_environment()?;
    let client = Client::builder()
        .user_agent("mira-client")
        .build()
        .map_err(|error| format!("Could not create content HTTP client: {error}"))?;
    let latest: LatestManifest = client
        .get(environment.latest_manifest_url())
        .send()
        .map_err(|error| format!("Could not download latest content pointer: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Latest content pointer request failed: {error}"))?
        .json()
        .map_err(|error| format!("Could not parse latest content pointer: {error}"))?;
    latest.validate_for(environment)?;

    let manifest: ContentManifest = client
        .get(&latest.content_manifest_url)
        .send()
        .map_err(|error| format!("Could not download content manifest: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Content manifest request failed: {error}"))?
        .json()
        .map_err(|error| format!("Could not parse content manifest: {error}"))?;
    manifest.validate_for(environment)?;

    let content_root = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not resolve application data directory: {error}"))?
        .join(CONTENT_DIRECTORY);

    if !content_needs_update(&content_root, &manifest.content) {
        return Ok(content_root.join("assets"));
    }

    fs::create_dir_all(&content_root)
        .map_err(|error| format!("Could not create content directory: {error}"))?;
    let archive_path = content_root.join("assets.zip");
    download_artifact(&client, &manifest.content, &archive_path)?;
    install_content_archive(&archive_path, &content_root, &manifest.content)?;
    let _ = fs::remove_file(archive_path);
    Ok(content_root.join("assets"))
}

fn content_needs_update(content_root: &Path, artifact: &ContentArtifact) -> bool {
    !content_is_current(content_root, artifact)
}

pub(crate) fn content_is_current(content_root: &Path, artifact: &ContentArtifact) -> bool {
    let state_path = content_root.join(CONTENT_STATE_FILENAME);
    let Some(state) = fs::read(&state_path)
        .ok()
        .and_then(|value| serde_json::from_slice::<ContentState>(&value).ok())
    else {
        return false;
    };

    state.size == artifact.size
        && state.sha256.eq_ignore_ascii_case(&artifact.sha256)
        && content_root.join("assets").join("index.html").is_file()
}

fn download_artifact(
    client: &Client,
    content: &ContentArtifact,
    destination: &Path,
) -> Result<(), String> {
    let artifact = Artifact {
        url: content.url.clone(),
        sha256: content.sha256.clone(),
        size: content.size,
    };
    let temporary = destination.with_extension("zip.part");
    let mut response = client
        .get(&artifact.url)
        .send()
        .map_err(|error| format!("Could not download content artifact: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Content artifact request failed: {error}"))?;
    let mut file = File::create(&temporary)
        .map_err(|error| format!("Could not create content download: {error}"))?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let bytes = response
            .read(&mut buffer)
            .map_err(|error| format!("Could not read content download: {error}"))?;
        if bytes == 0 {
            break;
        }
        file.write_all(&buffer[..bytes])
            .map_err(|error| format!("Could not write content download: {error}"))?;
        hasher.update(&buffer[..bytes]);
        downloaded += bytes as u64;
    }

    let checksum = to_hex(&hasher.finalize());
    if let Err(error) = verify_artifact(&artifact, downloaded, &checksum) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    fs::rename(&temporary, destination)
        .map_err(|error| format!("Could not finalize content download: {error}"))
}

fn install_content_archive(
    archive_path: &Path,
    content_root: &Path,
    artifact: &ContentArtifact,
) -> Result<(), String> {
    let staging_root = content_root.join(".assets-new");
    remove_path_if_exists(&staging_root)?;
    fs::create_dir_all(&staging_root)
        .map_err(|error| format!("Could not create content staging directory: {error}"))?;

    if let Err(error) = unzip_archive(archive_path, &staging_root) {
        let _ = fs::remove_dir_all(&staging_root);
        return Err(error);
    }
    let staged_assets = staging_root.join("assets");
    if !staged_assets.join("index.html").is_file() {
        let _ = fs::remove_dir_all(&staging_root);
        return Err("Content archive does not contain assets/index.html".to_string());
    }

    let state = ContentState {
        sha256: artifact.sha256.clone(),
        size: artifact.size,
        installed_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("Could not create content installation timestamp: {error}"))?
            .as_secs()
            .to_string(),
    };
    let state_temporary = content_root.join("content-state.json.new");
    fs::write(
        &state_temporary,
        serde_json::to_vec_pretty(&state)
            .map_err(|error| format!("Could not serialize content state: {error}"))?,
    )
    .map_err(|error| format!("Could not write new content state: {error}"))?;

    let active_assets = content_root.join("assets");
    let backup_assets = content_root.join(".assets-previous");
    let state_path = content_root.join(CONTENT_STATE_FILENAME);
    let backup_state = content_root.join(".content-state-previous");
    remove_path_if_exists(&backup_assets)?;
    remove_path_if_exists(&backup_state)?;
    let had_active_assets = active_assets.exists();
    let had_state = state_path.exists();
    if had_state {
        fs::rename(&state_path, &backup_state)
            .map_err(|error| format!("Could not preserve current content state: {error}"))?;
    }
    if had_active_assets {
        if let Err(error) = fs::rename(&active_assets, &backup_assets) {
            if had_state {
                let _ = fs::rename(&backup_state, &state_path);
            }
            return Err(format!("Could not preserve current content: {error}"));
        }
    }
    if let Err(error) = fs::rename(&staged_assets, &active_assets) {
        if had_active_assets {
            let _ = fs::rename(&backup_assets, &active_assets);
        }
        if had_state {
            let _ = fs::rename(&backup_state, &state_path);
        }
        let _ = fs::remove_file(&state_temporary);
        return Err(format!("Could not activate new content: {error}"));
    }

    if let Err(error) = fs::rename(&state_temporary, &state_path) {
        let _ = fs::remove_dir_all(&active_assets);
        if had_active_assets {
            let _ = fs::rename(&backup_assets, &active_assets);
        }
        if had_state {
            let _ = fs::rename(&backup_state, &state_path);
        }
        return Err(format!("Could not activate new content state: {error}"));
    }

    let _ = fs::remove_dir_all(&backup_assets);
    let _ = fs::remove_file(&backup_state);
    let _ = fs::remove_dir_all(&staging_root);
    Ok(())
}

fn unzip_archive(archive_path: &Path, destination: &Path) -> Result<(), String> {
    let file = File::open(archive_path)
        .map_err(|error| format!("Could not open content archive: {error}"))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| format!("Could not read content archive: {error}"))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Could not read content archive entry: {error}"))?;
        let enclosed_name = entry
            .enclosed_name()
            .ok_or_else(|| "Content archive contains an unsafe path".to_string())?
            .to_path_buf();
        let output = destination.join(enclosed_name);
        if entry.is_dir() {
            fs::create_dir_all(output)
                .map_err(|error| format!("Could not create content directory: {error}"))?;
        } else {
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("Could not create content directory: {error}"))?;
            }
            let mut output_file = File::create(output)
                .map_err(|error| format!("Could not create content file: {error}"))?;
            std::io::copy(&mut entry, &mut output_file)
                .map_err(|error| format!("Could not extract content file: {error}"))?;
        }
    }
    Ok(())
}

fn remove_path_if_exists(path: &Path) -> Result<(), String> {
    if path.exists() {
        if path.is_dir() {
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        }
        .map_err(|error| format!("Could not remove stale content path: {error}"))?;
    }
    Ok(())
}

fn verify_artifact(
    artifact: &Artifact,
    actual_size: u64,
    actual_sha256: &str,
) -> Result<(), String> {
    if artifact.size != actual_size {
        return Err(format!(
            "Content download size mismatch: expected {} bytes, got {actual_size}",
            artifact.size
        ));
    }
    if !artifact.sha256.eq_ignore_ascii_case(actual_sha256) {
        return Err("Content download checksum mismatch".to_string());
    }
    Ok(())
}

fn to_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::{ZipWriter, write::SimpleFileOptions};

    fn artifact(sha256: &str, size: u64) -> ContentArtifact {
        ContentArtifact {
            url: "https://downloads.tilt-us.com/content/assets.zip".to_string(),
            sha256: sha256.to_string(),
            size,
            content_id: Some("test-content".to_string()),
        }
    }

    fn write_archive(path: &Path, valid: bool) {
        let file = File::create(path).unwrap();
        let mut archive = ZipWriter::new(file);
        let entry = if valid {
            "assets/index.html"
        } else {
            "invalid.txt"
        };
        archive
            .start_file(entry, SimpleFileOptions::default())
            .unwrap();
        archive.write_all(b"content").unwrap();
        archive.finish().unwrap();
    }

    #[test]
    fn parses_content_manifest() {
        let manifest: ContentManifest = serde_json::from_str(
            r#"{"schemaVersion":1,"environment":"dev","content":{"url":"https://downloads.tilt-us.com/dev/content/assets.zip","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","size":12}}"#,
        )
        .unwrap();
        assert_eq!(manifest.content.size, 12);
    }

    #[test]
    fn missing_or_mismatched_content_requires_an_update() {
        let directory = tempfile::tempdir().unwrap();
        let expected = artifact("a", 5);
        assert!(content_needs_update(directory.path(), &expected));

        fs::create_dir_all(directory.path().join("assets")).unwrap();
        fs::write(directory.path().join("assets/index.html"), "ok").unwrap();
        fs::write(
            directory.path().join(CONTENT_STATE_FILENAME),
            r#"{"sha256":"different","size":5,"installedAt":"1"}"#,
        )
        .unwrap();
        assert!(content_needs_update(directory.path(), &expected));
    }

    #[test]
    fn current_content_does_not_require_another_download() {
        let directory = tempfile::tempdir().unwrap();
        let expected = artifact("a", 5);
        fs::create_dir_all(directory.path().join("assets")).unwrap();
        fs::write(directory.path().join("assets/index.html"), "ok").unwrap();
        fs::write(
            directory.path().join(CONTENT_STATE_FILENAME),
            r#"{"sha256":"a","size":5,"installedAt":"1"}"#,
        )
        .unwrap();
        assert!(!content_needs_update(directory.path(), &expected));
    }

    #[test]
    fn successful_install_updates_state_without_destroying_current_content_on_failure() {
        let directory = tempfile::tempdir().unwrap();
        let archive = directory.path().join("assets.zip");
        write_archive(&archive, true);
        let size = fs::metadata(&archive).unwrap().len();
        let content = artifact("a", size);
        install_content_archive(&archive, directory.path(), &content).unwrap();
        assert!(content_is_current(directory.path(), &content));

        write_archive(&archive, false);
        assert!(install_content_archive(&archive, directory.path(), &content).is_err());
        assert!(directory.path().join("assets/index.html").is_file());
        assert!(content_is_current(directory.path(), &content));
    }

    #[test]
    fn verifies_download_checksum_and_size() {
        let expected = Artifact {
            url: "https://downloads.tilt-us.com/content/assets.zip".to_string(),
            sha256: "abc".to_string(),
            size: 3,
        };
        assert!(verify_artifact(&expected, 3, "abc").is_ok());
        assert!(verify_artifact(&expected, 2, "abc").is_err());
        assert!(verify_artifact(&expected, 3, "def").is_err());
    }
}
