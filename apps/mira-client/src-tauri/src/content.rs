use crate::config;
use mira_downloads::{
    ContentArtifact, ContentManifest, DownloadProgress, LatestManifest, download_artifact,
    download_client,
};
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{Emitter, Manager};
use zip::ZipArchive;

const GAME_STATE_FILENAME: &str = "game-state.json";
const GAME_DIRECTORY: &str = "game";
const GAME_DOWNLOAD_FILENAME: &str = "game.zip";

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum GameContentStatusKind {
    Checking,
    Installing,
    Updating,
    Ready,
    Error,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameContentStatus {
    pub state: GameContentStatusKind,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub progress_percent: u8,
    pub error: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct GameContentState {
    schema_version: u32,
    environment: mira_downloads::Environment,
    content_id: String,
    sha256: String,
    size: u64,
    installed_at: String,
}

#[derive(Default)]
pub(crate) struct GameContentManager {
    status: std::sync::Mutex<GameContentStatus>,
}

impl Default for GameContentStatus {
    fn default() -> Self {
        Self {
            state: GameContentStatusKind::Checking,
            downloaded_bytes: 0,
            total_bytes: 0,
            progress_percent: 0,
            error: None,
        }
    }
}

impl GameContentManager {
    fn set_status(&self, app: &tauri::AppHandle, status: GameContentStatus) {
        if let Ok(mut current) = self.status.lock() {
            *current = status.clone();
        }
        let _ = app.emit("game-content-status", status);
    }

    fn status(&self) -> GameContentStatus {
        self.status
            .lock()
            .map(|status| status.clone())
            .unwrap_or_default()
    }
}

#[tauri::command]
pub(crate) fn game_content_status(
    content_manager: tauri::State<'_, GameContentManager>,
) -> GameContentStatus {
    content_manager.status()
}

/// Checks immediately on client startup and performs missing/stale game content work in the background.
pub(crate) fn start_game_content_sync(app: tauri::AppHandle) {
    tauri::async_runtime::spawn_blocking(move || {
        let manager = app.state::<GameContentManager>();
        manager.set_status(
            &app,
            GameContentStatus {
                state: GameContentStatusKind::Checking,
                ..GameContentStatus::default()
            },
        );
        if let Err(error) = synchronize_game_content(&app, &manager) {
            manager.set_status(
                &app,
                GameContentStatus {
                    state: GameContentStatusKind::Error,
                    error: Some(error),
                    ..GameContentStatus::default()
                },
            );
        }
    });
}

/// Resolves the installed game root only when the startup updater has completed successfully.
pub(crate) fn ready_game_asset_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let manager = app.state::<GameContentManager>();
    let status = manager.status();
    if status.state != GameContentStatusKind::Ready {
        return Err(
            "Game content is not ready. Finish the current installation or update first."
                .to_string(),
        );
    }
    let assets_root = install_root()?.join("assets");
    let game_root = assets_root.join(GAME_DIRECTORY);
    if has_required_game_content(&game_root) && has_required_game_client_runtime_ui(&assets_root) {
        Ok(assets_root)
    } else {
        Err(
            "Required game assets are incomplete. Expected assets/game and assets/ui. Repair the Mira installation before starting a match."
                .to_string(),
        )
    }
}

fn synchronize_game_content(
    app: &tauri::AppHandle,
    manager: &GameContentManager,
) -> Result<(), String> {
    let environment = config::build_environment()?;
    let client = download_client()?;
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

    let root = install_root()?;
    let assets_root = root.join("assets");
    let game_root = assets_root.join(GAME_DIRECTORY);
    let installing = !game_root.is_dir();
    if !game_content_needs_update(&assets_root, &manifest.game, environment) {
        manager.set_status(
            app,
            GameContentStatus {
                state: GameContentStatusKind::Ready,
                downloaded_bytes: manifest.game.size,
                total_bytes: manifest.game.size,
                progress_percent: 100,
                error: None,
            },
        );
        return Ok(());
    }

    let state = if installing {
        GameContentStatusKind::Installing
    } else {
        GameContentStatusKind::Updating
    };
    manager.set_status(
        app,
        GameContentStatus {
            state,
            total_bytes: manifest.game.size,
            ..GameContentStatus::default()
        },
    );
    fs::create_dir_all(&assets_root)
        .map_err(|error| format!("Could not create assets directory: {error}"))?;
    let archive_path = assets_root.join(GAME_DOWNLOAD_FILENAME);
    let artifact = manifest.game.as_artifact();
    download_artifact(&client, &artifact, &archive_path, |progress| {
        emit_download_progress(app, manager, state, progress);
    })?;
    install_game_archive(&archive_path, &assets_root, &manifest.game, environment)?;
    let _ = fs::remove_file(&archive_path);
    manager.set_status(
        app,
        GameContentStatus {
            state: GameContentStatusKind::Ready,
            downloaded_bytes: manifest.game.size,
            total_bytes: manifest.game.size,
            progress_percent: 100,
            error: None,
        },
    );
    Ok(())
}

fn emit_download_progress(
    app: &tauri::AppHandle,
    manager: &GameContentManager,
    state: GameContentStatusKind,
    progress: DownloadProgress,
) {
    manager.set_status(
        app,
        GameContentStatus {
            state,
            downloaded_bytes: progress.downloaded_bytes,
            total_bytes: progress.total_bytes,
            progress_percent: progress.percent(),
            error: None,
        },
    );
}

fn game_content_needs_update(
    assets_root: &Path,
    artifact: &ContentArtifact,
    environment: mira_downloads::Environment,
) -> bool {
    let state_path = assets_root.join(GAME_STATE_FILENAME);
    let Some(state) = fs::read(&state_path)
        .ok()
        .and_then(|value| serde_json::from_slice::<GameContentState>(&value).ok())
    else {
        return true;
    };
    !(state.environment == environment
        && state.content_id == artifact.content_id
        && state.size == artifact.size
        && state.sha256.eq_ignore_ascii_case(&artifact.sha256)
        && has_required_game_content(&assets_root.join(GAME_DIRECTORY)))
}

fn install_game_archive(
    archive_path: &Path,
    assets_root: &Path,
    artifact: &ContentArtifact,
    environment: mira_downloads::Environment,
) -> Result<(), String> {
    let staging_root = assets_root.join(".game-new");
    remove_path_if_exists(&staging_root)?;
    fs::create_dir_all(&staging_root)
        .map_err(|error| format!("Could not create game staging directory: {error}"))?;
    if let Err(error) = unzip_archive(archive_path, &staging_root) {
        let _ = fs::remove_dir_all(&staging_root);
        return Err(error);
    }
    let staged_game = staging_root.join(GAME_DIRECTORY);
    if !has_required_game_content(&staged_game) {
        let _ = fs::remove_dir_all(&staging_root);
        return Err(
            "Game archive does not contain assets/game with expected directories.".to_string(),
        );
    }
    let state = GameContentState {
        schema_version: 1,
        environment,
        content_id: artifact.content_id.clone(),
        sha256: artifact.sha256.clone(),
        size: artifact.size,
        installed_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("Could not create game installation timestamp: {error}"))?
            .as_secs()
            .to_string(),
    };
    let state_temporary = assets_root.join(".game-state-new");
    fs::write(
        &state_temporary,
        serde_json::to_vec_pretty(&state)
            .map_err(|error| format!("Could not serialize game content state: {error}"))?,
    )
    .map_err(|error| format!("Could not write game content state: {error}"))?;

    let active_game = assets_root.join(GAME_DIRECTORY);
    let previous_game = assets_root.join(".game-previous");
    let state_path = assets_root.join(GAME_STATE_FILENAME);
    let previous_state = assets_root.join(".game-state-previous");
    remove_path_if_exists(&previous_game)?;
    remove_path_if_exists(&previous_state)?;
    let had_game = active_game.exists();
    let had_state = state_path.exists();
    if had_game {
        fs::rename(&active_game, &previous_game)
            .map_err(|error| format!("Could not preserve current game content: {error}"))?;
    }
    if had_state {
        fs::rename(&state_path, &previous_state)
            .map_err(|error| format!("Could not preserve current game state: {error}"))?;
    }
    if let Err(error) = fs::rename(&staged_game, &active_game) {
        if had_game {
            let _ = fs::rename(&previous_game, &active_game);
        }
        if had_state {
            let _ = fs::rename(&previous_state, &state_path);
        }
        return Err(format!("Could not activate new game content: {error}"));
    }
    if let Err(error) = fs::rename(&state_temporary, &state_path) {
        let _ = fs::remove_dir_all(&active_game);
        if had_game {
            let _ = fs::rename(&previous_game, &active_game);
        }
        if had_state {
            let _ = fs::rename(&previous_state, &state_path);
        }
        return Err(format!("Could not activate game content state: {error}"));
    }
    let _ = fs::remove_dir_all(&previous_game);
    let _ = fs::remove_file(&previous_state);
    let _ = fs::remove_dir_all(&staging_root);
    Ok(())
}

pub(crate) fn install_root() -> Result<PathBuf, String> {
    if let Some(root) = std::env::var_os(mira_downloads::INSTALL_ROOT_ENV) {
        return Ok(PathBuf::from(root));
    }
    if let Some(root) = mira_downloads::recorded_install_root(config::build_environment()?) {
        return Ok(root);
    }
    std::env::current_exe()
        .map_err(|error| format!("Could not resolve client executable: {error}"))?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Client executable has no installation directory".to_string())
}

pub(crate) fn has_required_game_content(game_root: &Path) -> bool {
    ["audio", "champions", "maps", "materials"]
        .iter()
        .all(|directory| game_root.join(directory).is_dir())
}

pub(crate) fn ui_asset_root() -> Result<PathBuf, String> {
    #[cfg(debug_assertions)]
    if std::env::var_os(mira_downloads::INSTALL_ROOT_ENV).is_none() {
        return repository_ui_asset_root();
    }

    let installed = install_root()?.join("assets").join("ui");
    if has_required_ui_content(&installed) {
        return Ok(installed);
    }

    repository_ui_asset_root()
}

#[cfg(debug_assertions)]
fn repository_ui_asset_root() -> Result<PathBuf, String> {
    canonical_ui_asset_root(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("assets")
            .join("ui"),
    )
}

#[cfg(not(debug_assertions))]
fn repository_ui_asset_root() -> Result<PathBuf, String> {
    Err(
        "Mira UI assets are missing at <install-root>/assets/ui. Install or repair Mira with the Mira Installer."
            .to_string(),
    )
}

#[cfg(debug_assertions)]
fn canonical_ui_asset_root(path: PathBuf) -> Result<PathBuf, String> {
    path.canonicalize().map_err(|error| {
        format!(
            "Could not resolve UI asset directory {}: {error}",
            path.display()
        )
    })
}

#[tauri::command]
pub(crate) fn ui_asset_root_path() -> Result<String, String> {
    Ok(ui_asset_root()?.to_string_lossy().into_owned())
}

pub(crate) fn has_required_ui_content(ui_root: &Path) -> bool {
    ["characters", "fonts", "wallpapers", "icons"]
        .iter()
        .all(|directory| ui_root.join(directory).is_dir())
}

/// Verifies the exact UI assets loaded by the standalone Bevy game client.
#[cfg(test)]
pub(crate) fn has_required_game_client_ui_content(assets_root: &Path) -> bool {
    [
        "ui/wallpapers/lira-loading.jpg",
        "ui/wallpapers/ignara-loading.jpg",
        "ui/wallpapers/sophia-loading.jpg",
        "ui/wallpapers/yuna-loading.jpg",
        "ui/characters/lira.png",
        "ui/characters/ignara.png",
        "ui/characters/sophia.png",
        "ui/characters/yuna.png",
        "ui/fonts/Roboto-Bold.ttf",
    ]
    .iter()
    .all(|asset| assets_root.join(asset).is_file())
}

pub(crate) fn has_required_game_client_runtime_ui(assets_root: &Path) -> bool {
    ["characters", "fonts", "wallpapers"]
        .iter()
        .all(|directory| assets_root.join("ui").join(directory).is_dir())
        && assets_root.join("ui/fonts/Roboto-Bold.ttf").is_file()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::{ZipWriter, write::SimpleFileOptions};

    fn artifact() -> ContentArtifact {
        ContentArtifact {
            url: "https://downloads.tilt-us.com/content/game.zip".to_string(),
            sha256: "a".repeat(64),
            size: 42,
            content_id: "game-v1".to_string(),
        }
    }

    fn write_archive(path: &Path, valid: bool) {
        let file = File::create(path).unwrap();
        let mut archive = ZipWriter::new(file);
        let directories = if valid {
            [
                "game/audio/a",
                "game/champions/a",
                "game/maps/a",
                "game/materials/a",
            ]
        } else {
            [
                "ui/characters/a",
                "ui/fonts/a",
                "ui/wallpapers/a",
                "ui/icons/a",
            ]
        };
        for entry in directories {
            archive
                .start_file(entry, SimpleFileOptions::default())
                .unwrap();
            archive.write_all(b"x").unwrap();
        }
        archive.finish().unwrap();
    }

    #[test]
    fn missing_game_is_an_installation_and_current_game_is_not_updated() {
        let directory = tempfile::tempdir().unwrap();
        assert!(game_content_needs_update(
            directory.path(),
            &artifact(),
            mira_downloads::Environment::Dev,
        ));
        let game = directory.path().join("game");
        for child in ["audio", "champions", "maps", "materials"] {
            fs::create_dir_all(game.join(child)).unwrap();
        }
        fs::write(
            directory.path().join(GAME_STATE_FILENAME),
            serde_json::to_vec(&GameContentState {
                schema_version: 1,
                environment: mira_downloads::Environment::Dev,
                content_id: "game-v1".to_string(),
                sha256: "a".repeat(64),
                size: 42,
                installed_at: "1".to_string(),
            })
            .unwrap(),
        )
        .unwrap();
        let state: GameContentState =
            serde_json::from_slice(&fs::read(directory.path().join(GAME_STATE_FILENAME)).unwrap())
                .unwrap();
        assert_eq!(state.content_id, artifact().content_id);
        assert_eq!(state.sha256, artifact().sha256);
        assert!(has_required_game_content(
            &directory.path().join(GAME_DIRECTORY)
        ));
        assert!(!game_content_needs_update(
            directory.path(),
            &artifact(),
            mira_downloads::Environment::Dev,
        ));
    }

    #[cfg(debug_assertions)]
    #[test]
    fn repository_ui_assets_use_the_external_ui_root() {
        let ui_root = ui_asset_root().unwrap();
        assert!(ui_root.is_absolute());
        assert!(
            !ui_root
                .components()
                .any(|component| component == std::path::Component::ParentDir)
        );

        assert!(has_required_ui_content(&ui_root));
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn release_builds_do_not_fall_back_to_the_repository_ui_assets() {
        let error = repository_ui_asset_root().unwrap_err();

        assert!(error.contains("<install-root>/assets/ui"));
    }

    #[test]
    fn ui_asset_validation_requires_all_external_ui_directories() {
        let directory = tempfile::tempdir().unwrap();
        let ui_root = directory.path().join("ui");
        fs::create_dir_all(&ui_root).unwrap();
        assert!(!has_required_ui_content(&ui_root));

        for child in ["characters", "fonts", "wallpapers", "icons"] {
            fs::create_dir_all(ui_root.join(child)).unwrap();
        }
        assert!(has_required_ui_content(&ui_root));
    }

    #[test]
    fn game_client_ui_validation_requires_all_assets_loaded_by_bevy() {
        let directory = tempfile::tempdir().unwrap();
        let assets_root = directory.path().join("assets");
        assert!(!has_required_game_client_ui_content(&assets_root));

        for asset in [
            "ui/wallpapers/lira-loading.jpg",
            "ui/wallpapers/ignara-loading.jpg",
            "ui/wallpapers/sophia-loading.jpg",
            "ui/wallpapers/yuna-loading.jpg",
            "ui/characters/lira.png",
            "ui/characters/ignara.png",
            "ui/characters/sophia.png",
            "ui/characters/yuna.png",
            "ui/fonts/Roboto-Bold.ttf",
        ] {
            let path = assets_root.join(asset);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, []).unwrap();
        }

        assert!(has_required_game_client_ui_content(&assets_root));
    }

    #[test]
    fn failed_update_preserves_previous_game() {
        let directory = tempfile::tempdir().unwrap();
        let assets = directory.path();
        let active = assets.join("game");
        for child in ["audio", "champions", "maps", "materials"] {
            fs::create_dir_all(active.join(child)).unwrap();
        }
        fs::write(active.join("champions/old.txt"), "old").unwrap();
        let archive = assets.join("game.zip");
        write_archive(&archive, false);
        assert!(
            install_game_archive(
                &archive,
                assets,
                &artifact(),
                mira_downloads::Environment::Dev
            )
            .is_err()
        );
        assert!(active.join("champions/old.txt").is_file());
    }

    #[test]
    fn game_archive_requires_only_game_directories() {
        let directory = tempfile::tempdir().unwrap();
        let archive = directory.path().join("game.zip");
        write_archive(&archive, true);
        install_game_archive(
            &archive,
            directory.path(),
            &artifact(),
            mira_downloads::Environment::Dev,
        )
        .unwrap();
        assert!(has_required_game_content(&directory.path().join("game")));
        assert!(!directory.path().join("ui").exists());
    }
}
