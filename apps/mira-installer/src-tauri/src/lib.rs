use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::Emitter;
use zip::ZipArchive;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const LATEST_MANIFEST_URL: &str = "https://api.tilt-us.com/downloads/mira/game-sources/latest.json";
const ERROR_CODE_GAME_DATA: &str = "465";
const ERROR_CODE_SERVER_NO_RESPONSE: &str = "19145";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallProgress {
    label_key: String,
    progress: f32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlatformInfo {
    os: String,
    linux_family: Option<String>,
    package_extension: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct LatestManifest {
    version: String,
    tag: String,
    commit: String,
    manifest_url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadManifest {
    version: String,
    tag: String,
    commit: String,
    published_at: String,
    base_url: String,
    files: Vec<ManifestFile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ManifestFile {
    path: String,
    url: String,
    size: u64,
    sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallResult {
    launcher_path: String,
}

#[tauri::command]
fn detect_platform() -> PlatformInfo {
    detect_platform_info()
}

#[tauri::command]
fn path_has_content(install_path: String) -> Result<bool, String> {
    let path = PathBuf::from(install_path);

    if !path.exists() {
        return Ok(false);
    }

    let mut entries = fs::read_dir(path)
        .map_err(|error| format!("failed to read installation folder: {error}"))?;

    Ok(entries
        .next()
        .transpose()
        .map_err(|error| format!("failed to inspect installation folder: {error}"))?
        .is_some())
}

#[tauri::command]
async fn install_game(
    app: tauri::AppHandle,
    install_path: String,
) -> Result<InstallResult, String> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        install_game_blocking(app, install_path)
            .map_err(|error| normalize_install_error_code(&error).to_string())
    })
    .await
    .map_err(|_| ERROR_CODE_GAME_DATA.to_string())?;

    result
}

#[tauri::command]
fn launch_installed_launcher(launcher_path: String) -> Result<(), String> {
    launch_path(PathBuf::from(launcher_path))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            detect_platform,
            install_game,
            launch_installed_launcher,
            path_has_content
        ])
        .run(tauri::generate_context!())
        .expect("error while running Mira Installer");
}

fn install_game_blocking(
    app: tauri::AppHandle,
    install_path: String,
) -> Result<InstallResult, String> {
    let root = PathBuf::from(install_path);
    fs::create_dir_all(&root)
        .map_err(|error| format!("failed to create installation folder: {error}"))?;

    emit_progress(&app, "install-status-platform", 0.03);
    let platform = detect_platform_info();

    emit_progress(&app, "install-status-manifest", 0.08);
    let client = Client::builder()
        .user_agent("mira-installer")
        .build()
        .map_err(|error| format!("failed to create http client: {error}"))?;

    let latest: LatestManifest = client
        .get(LATEST_MANIFEST_URL)
        .send()
        .map_err(|error| format!("failed to download latest manifest: {error}"))?
        .error_for_status()
        .map_err(|error| format!("latest manifest request failed: {error}"))?
        .json()
        .map_err(|error| format!("failed to parse latest manifest: {error}"))?;

    let manifest: DownloadManifest = client
        .get(&latest.manifest_url)
        .send()
        .map_err(|error| format!("failed to download release manifest: {error}"))?
        .error_for_status()
        .map_err(|error| format!("release manifest request failed: {error}"))?
        .json()
        .map_err(|error| format!("failed to parse release manifest: {error}"))?;

    let client_file = find_manifest_file(&manifest, |file| {
        let path = file.path.to_lowercase();
        path.starts_with("mira-client/")
            && path.ends_with(&platform.package_extension.to_lowercase())
    })?;
    let game_file = find_manifest_file(&manifest, |file| {
        let path = file.path.to_lowercase();
        path.starts_with("mira-game-client/") && path.ends_with(game_client_suffix(&platform))
    })?;
    let assets_archive = find_archive(&manifest, "assets", &latest.tag)?;

    let temp_dir = root.join(".mira-installer");
    replace_dir(&temp_dir)?;

    let client_filename = Path::new(&client_file.path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "client download has no filename".to_string())?;
    let client_download = temp_dir.join(client_filename);
    let game_filename = Path::new(&game_file.path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "game client download has no filename".to_string())?;
    let game_download = temp_dir.join(game_filename);
    let assets_download = temp_dir.join(format!("assets-{}.zip", latest.tag));

    download_file(
        &app,
        &client,
        &client_file,
        &client_download,
        "install-status-download-client",
        0.12,
        0.28,
    )?;
    download_file(
        &app,
        &client,
        &game_file,
        &game_download,
        "install-status-download-game",
        0.28,
        0.48,
    )?;
    download_file(
        &app,
        &client,
        &assets_archive,
        &assets_download,
        "install-status-download-assets",
        0.48,
        0.68,
    )?;

    emit_progress(&app, "install-status-finalize", 0.7);
    let assets_dir = root.join("assets");
    replace_dir(&assets_dir)?;

    let launcher_path = root.join(launcher_filename(&platform));
    let game_path = root.join(game_client_filename(&platform));
    remove_legacy_install_entries(&root)?;
    remove_if_exists(&launcher_path)?;
    remove_if_exists(&game_path)?;

    fs::copy(&client_download, &launcher_path)
        .map_err(|error| format!("failed to install launcher: {error}"))?;
    fs::copy(&game_download, &game_path)
        .map_err(|error| format!("failed to install game client: {error}"))?;
    make_executable(&launcher_path)?;
    make_executable(&game_path)?;

    unzip_archive(
        &app,
        &assets_download,
        &assets_dir,
        "install-status-unzip-assets",
        0.72,
        0.98,
    )?;

    write_json(root.join("latest.json"), &latest)?;
    write_json(root.join("manifest.json"), &manifest)?;
    let _ = fs::remove_dir_all(&temp_dir);

    emit_progress(&app, "install-status-done", 1.0);
    Ok(InstallResult {
        launcher_path: launcher_path.to_string_lossy().to_string(),
    })
}

fn detect_platform_info() -> PlatformInfo {
    match std::env::consts::OS {
        "windows" => PlatformInfo {
            os: "windows".to_string(),
            linux_family: None,
            package_extension: ".exe".to_string(),
        },
        "macos" => PlatformInfo {
            os: "macos".to_string(),
            linux_family: None,
            package_extension: ".dmg".to_string(),
        },
        "linux" => {
            let family = detect_linux_family();
            let package_extension = match family.as_deref() {
                Some("debian") => ".deb",
                Some("fedora") => ".rpm",
                Some("arch") => ".AppImage",
                _ => ".AppImage",
            };

            PlatformInfo {
                os: "linux".to_string(),
                linux_family: family,
                package_extension: package_extension.to_string(),
            }
        }
        other => PlatformInfo {
            os: other.to_string(),
            linux_family: None,
            package_extension: ".AppImage".to_string(),
        },
    }
}

fn detect_linux_family() -> Option<String> {
    let release = fs::read_to_string("/etc/os-release").ok()?.to_lowercase();
    let values = release
        .lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(key, _)| *key == "id" || *key == "id_like")
        .flat_map(|(_, value)| {
            value
                .trim_matches('"')
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    if values.iter().any(|value| {
        matches!(
            value.as_str(),
            "debian" | "ubuntu" | "linuxmint" | "pop" | "elementary"
        )
    }) {
        return Some("debian".to_string());
    }

    if values.iter().any(|value| {
        matches!(
            value.as_str(),
            "fedora" | "rhel" | "centos" | "rocky" | "almalinux" | "suse" | "opensuse"
        )
    }) {
        return Some("fedora".to_string());
    }

    if values.iter().any(|value| {
        matches!(
            value.as_str(),
            "arch" | "manjaro" | "endeavouros" | "garuda"
        )
    }) {
        return Some("arch".to_string());
    }

    None
}

fn game_client_suffix(platform: &PlatformInfo) -> &'static str {
    match platform.os.as_str() {
        "windows" => "-windows.exe",
        "macos" => "-macos",
        _ => "-linux",
    }
}

fn launcher_filename(platform: &PlatformInfo) -> &'static str {
    match platform.os.as_str() {
        "windows" => "mira-launcher.exe",
        "macos" => "mira-launcher.dmg",
        "linux" => match platform.package_extension.as_str() {
            ".deb" => "mira-launcher.deb",
            ".rpm" => "mira-launcher.rpm",
            ".AppImage" => "mira-launcher.AppImage",
            _ => "mira-launcher",
        },
        _ => "mira-launcher",
    }
}

fn game_client_filename(platform: &PlatformInfo) -> &'static str {
    match platform.os.as_str() {
        "windows" => "mira-game-client.exe",
        _ => "mira-game-client",
    }
}

fn remove_legacy_install_entries(root: &Path) -> Result<(), String> {
    for entry in [
        "mira-client",
        "mira-launcher",
        "mira-launcher.exe",
        "mira-launcher.dmg",
        "mira-launcher.deb",
        "mira-launcher.rpm",
        "mira-launcher.AppImage",
        "mira-game-client.exe",
    ] {
        remove_if_exists(&root.join(entry))?;
    }

    Ok(())
}

fn find_archive(
    manifest: &DownloadManifest,
    archive_name: &str,
    tag: &str,
) -> Result<ManifestFile, String> {
    let exact_path = format!("{archive_name}-{tag}.zip");
    find_manifest_file(manifest, |file| file.path == exact_path).or_else(|_| {
        find_manifest_file(manifest, |file| {
            file.path.starts_with(archive_name) && file.path.ends_with(".zip")
        })
    })
}

fn find_manifest_file<F>(manifest: &DownloadManifest, predicate: F) -> Result<ManifestFile, String>
where
    F: Fn(&ManifestFile) -> bool,
{
    manifest
        .files
        .iter()
        .find(|file| predicate(file))
        .cloned()
        .ok_or_else(|| "required download file is missing in manifest".to_string())
}

fn normalize_install_error_code(error: &str) -> &'static str {
    let normalized = error.to_lowercase();

    if normalized.contains("failed to download")
        || normalized.contains("request failed")
        || normalized.contains("while downloading")
        || normalized.contains("http")
        || normalized.contains("timed out")
        || normalized.contains("connection")
    {
        return ERROR_CODE_SERVER_NO_RESPONSE;
    }

    ERROR_CODE_GAME_DATA
}

fn launch_path(path: PathBuf) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("launcher does not exist: {}", path.display()));
    }

    let launcher_dir = path
        .parent()
        .ok_or_else(|| "465".to_string())?
        .to_path_buf();
    let path_text = path.to_string_lossy().to_string();
    let result = match std::env::consts::OS {
        "windows" => Command::new("cmd")
            .current_dir(&launcher_dir)
            .args(["/C", "start", "", &path_text])
            .spawn(),
        "macos" => Command::new("open")
            .current_dir(&launcher_dir)
            .arg(&path)
            .spawn(),
        "linux" => {
            if path.extension().and_then(|extension| extension.to_str()) == Some("AppImage") {
                let mut command = Command::new(&path);
                command.current_dir(&launcher_dir);
                configure_linux_webkit_command(&mut command);
                command.spawn()
            } else {
                let mut command = Command::new("xdg-open");
                command.current_dir(&launcher_dir);
                configure_linux_webkit_command(&mut command);
                command.arg(&path).spawn()
            }
        }
        _ => Command::new(&path).current_dir(&launcher_dir).spawn(),
    };

    result
        .map(|_| ())
        .map_err(|error| format!("failed to launch mira-launcher: {error}"))
}

#[cfg(target_os = "linux")]
fn configure_linux_webkit_command(command: &mut Command) {
    command.env("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

    if std::env::var("XDG_SESSION_TYPE")
        .is_ok_and(|session| session.eq_ignore_ascii_case("wayland"))
    {
        command.env("GDK_BACKEND", "x11");
    }
}

#[cfg(not(target_os = "linux"))]
fn configure_linux_webkit_command(_command: &mut Command) {}

fn download_file(
    app: &tauri::AppHandle,
    client: &Client,
    manifest_file: &ManifestFile,
    destination: &Path,
    label_key: &'static str,
    start: f32,
    end: f32,
) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create download folder: {error}"))?;
    }

    let temp_destination = part_path(destination)?;
    let mut response = client
        .get(&manifest_file.url)
        .send()
        .map_err(|error| format!("failed to download {}: {error}", manifest_file.path))?
        .error_for_status()
        .map_err(|error| {
            format!(
                "download request failed for {}: {error}",
                manifest_file.path
            )
        })?;

    let total = response
        .content_length()
        .filter(|length| *length > 0)
        .unwrap_or(manifest_file.size);
    let mut file = File::create(&temp_destination)
        .map_err(|error| format!("failed to create download file: {error}"))?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];

    emit_progress(app, label_key, start);
    loop {
        let bytes_read = response
            .read(&mut buffer)
            .map_err(|error| format!("failed while downloading {}: {error}", manifest_file.path))?;

        if bytes_read == 0 {
            break;
        }

        file.write_all(&buffer[..bytes_read])
            .map_err(|error| format!("failed to write download file: {error}"))?;
        hasher.update(&buffer[..bytes_read]);
        downloaded += bytes_read as u64;

        if total > 0 {
            let fraction = (downloaded as f32 / total as f32).clamp(0.0, 1.0);
            emit_progress(app, label_key, start + ((end - start) * fraction));
        }
    }

    let actual_sha256 = to_hex(&hasher.finalize());
    if !manifest_file.sha256.eq_ignore_ascii_case(&actual_sha256) {
        return Err(format!("checksum mismatch for {}", manifest_file.path));
    }

    fs::rename(&temp_destination, destination)
        .map_err(|error| format!("failed to finalize download file: {error}"))?;
    emit_progress(app, label_key, end);
    Ok(())
}

fn unzip_archive(
    app: &tauri::AppHandle,
    archive_path: &Path,
    destination: &Path,
    label_key: &'static str,
    start: f32,
    end: f32,
) -> Result<(), String> {
    let archive_file =
        File::open(archive_path).map_err(|error| format!("failed to open archive: {error}"))?;
    let mut archive = ZipArchive::new(archive_file)
        .map_err(|error| format!("failed to read archive: {error}"))?;
    let total_entries = archive.len().max(1);

    emit_progress(app, label_key, start);
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("failed to read archive entry: {error}"))?;
        let Some(enclosed_name) = entry.enclosed_name() else {
            continue;
        };
        let output_path = destination.join(enclosed_name);

        if entry.is_dir() {
            fs::create_dir_all(&output_path)
                .map_err(|error| format!("failed to create extracted folder: {error}"))?;
        } else {
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("failed to create extracted folder: {error}"))?;
            }

            let mut output_file = File::create(&output_path)
                .map_err(|error| format!("failed to create extracted file: {error}"))?;
            std::io::copy(&mut entry, &mut output_file)
                .map_err(|error| format!("failed to extract archive entry: {error}"))?;
        }

        let fraction = (index + 1) as f32 / total_entries as f32;
        emit_progress(app, label_key, start + ((end - start) * fraction));
    }

    emit_progress(app, label_key, end);
    Ok(())
}

fn replace_dir(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_dir_all(path)
            .map_err(|error| format!("failed to remove existing folder: {error}"))?;
    }

    fs::create_dir_all(path).map_err(|error| format!("failed to create folder: {error}"))
}

fn remove_if_exists(path: &Path) -> Result<(), String> {
    if path.exists() {
        if path.is_dir() {
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        }
        .map_err(|error| format!("failed to remove existing file: {error}"))?;
    }

    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    let mut permissions = fs::metadata(path)
        .map_err(|error| format!("failed to read file metadata: {error}"))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .map_err(|error| format!("failed to mark file executable: {error}"))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn write_json<T: Serialize>(path: PathBuf, value: &T) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("failed to serialize json: {error}"))?;
    fs::write(path, json).map_err(|error| format!("failed to write json: {error}"))
}

fn part_path(destination: &Path) -> Result<PathBuf, String> {
    let filename = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "download destination has no filename".to_string())?;
    Ok(destination.with_file_name(format!("{filename}.part")))
}

fn emit_progress(app: &tauri::AppHandle, label_key: &'static str, progress: f32) {
    let _ = app.emit(
        "installer:progress",
        InstallProgress {
            label_key: label_key.to_string(),
            progress: progress.clamp(0.0, 1.0),
        },
    );
}

fn to_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}
