use mira_downloads::{Artifact, Environment, LatestManifest, RuntimeManifest};
use reqwest::blocking::Client;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{Emitter, LogicalSize, Manager, Size};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const ERROR_CODE_GAME_DATA: &str = "465";
const ERROR_CODE_SERVER_NO_RESPONSE: &str = "19145";
const WINDOWS_PROGRAM_FILES_FALLBACK: &str = r"C:\Program Files";
const WINDOWS_INSTALLER_EXE_WAIT: Duration = Duration::from_secs(30);
const FHD_INSTALLER_SIZE: InstallerWindowSize = InstallerWindowSize {
    width: 450.0,
    height: 575.0,
};
const WQHD_INSTALLER_SIZE: InstallerWindowSize = InstallerWindowSize {
    width: 600.0,
    height: 750.0,
};
const FOUR_K_INSTALLER_SIZE: InstallerWindowSize = InstallerWindowSize {
    width: 675.0,
    height: 825.0,
};

/// Stores Installer Window Size data used by the installer Tauri backend system.
#[derive(Clone, Copy)]
struct InstallerWindowSize {
    width: f64,
    height: f64,
}

/// Stores Install Progress data used by the installer Tauri backend system.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallProgress {
    label_key: String,
    progress: f32,
}

/// Stores Platform Info data used by the installer Tauri backend system.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlatformInfo {
    os: String,
    linux_family: Option<String>,
    package_extension: String,
}

/// Stores Install Result data used by the installer Tauri backend system.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallResult {
    launcher_path: String,
}

/// Runs the detect platform step for the installer Tauri backend system.
#[tauri::command]
fn detect_platform() -> PlatformInfo {
    detect_platform_info()
}

/// Runs the default install path step for the installer Tauri backend system.
#[tauri::command]
fn default_install_path() -> String {
    default_install_path_buf().to_string_lossy().into_owned()
}

/// Runs the path has content step for the installer Tauri backend system.
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

/// Runs the install game step for the installer Tauri backend system.
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

/// Runs the launch installed launcher step for the installer Tauri backend system.
#[tauri::command]
fn launch_installed_launcher(launcher_path: String) -> Result<(), String> {
    launch_path(PathBuf::from(launcher_path))
}

/// Runs the run step for the installer Tauri backend system.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                disable_webview_hardware_acceleration(&window);
            }
            configure_main_window_size(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            detect_platform,
            default_install_path,
            install_game,
            launch_installed_launcher,
            path_has_content
        ])
        .run(tauri::generate_context!())
        .expect("error while running Mira Installer");
}

/// Runs the disable webview hardware acceleration step for the installer Tauri backend system.
#[cfg(target_os = "linux")]
fn disable_webview_hardware_acceleration(window: &tauri::WebviewWindow) {
    let _ = window.with_webview(|webview| {
        use webkit2gtk::{HardwareAccelerationPolicy, SettingsExt, WebViewExt};

        if let Some(settings) = webview.inner().settings() {
            settings.set_hardware_acceleration_policy(HardwareAccelerationPolicy::Never);
        }
    });
}

/// Runs the disable webview hardware acceleration step for the installer Tauri backend system.
#[cfg(not(target_os = "linux"))]
fn disable_webview_hardware_acceleration(_window: &tauri::WebviewWindow) {}

/// Runs the configure main window size step for the installer Tauri backend system.
fn configure_main_window_size(app: &mut tauri::App) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    let monitor_size = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
        .map(|monitor| monitor.size().to_owned());
    let installer_size = monitor_size
        .map(|size| installer_window_size_for_monitor(size.width, size.height))
        .unwrap_or(FHD_INSTALLER_SIZE);
    let logical_size = Size::Logical(LogicalSize {
        width: installer_size.width,
        height: installer_size.height,
    });

    let _ = window.set_max_size(Some(logical_size));
    let _ = window.set_min_size(Some(logical_size));
    let _ = window.set_size(logical_size);
    let _ = window.center();
}

/// Runs the installer window size for monitor step for the installer Tauri backend system.
fn installer_window_size_for_monitor(width: u32, height: u32) -> InstallerWindowSize {
    let long_side = width.max(height);
    let short_side = width.min(height);

    if long_side >= 3840 && short_side >= 2160 {
        FOUR_K_INSTALLER_SIZE
    } else if long_side >= 2560 && short_side >= 1440 {
        WQHD_INSTALLER_SIZE
    } else {
        FHD_INSTALLER_SIZE
    }
}

/// Runs the default install path buf step for the installer Tauri backend system.
fn default_install_path_buf() -> PathBuf {
    if cfg!(windows) {
        let mut install_path = std::env::var_os("ProgramFiles")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(WINDOWS_PROGRAM_FILES_FALLBACK));

        install_path.push("Mira Games");
        install_path.push("Mira Moba");
        return install_path;
    }

    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Runs the install game blocking step for the installer Tauri backend system.
fn install_game_blocking(
    app: tauri::AppHandle,
    install_path: String,
) -> Result<InstallResult, String> {
    let requested_root = PathBuf::from(install_path);
    fs::create_dir_all(&requested_root)
        .map_err(|error| format!("failed to create installation folder: {error}"))?;

    emit_progress(&app, "install-status-platform", 0.03);
    let platform = detect_platform_info();
    let environment = build_environment()?;

    emit_progress(&app, "install-status-manifest", 0.08);
    let client = Client::builder()
        .user_agent("mira-installer")
        .build()
        .map_err(|error| format!("failed to create http client: {error}"))?;

    let latest: LatestManifest = client
        .get(environment.latest_manifest_url())
        .send()
        .map_err(|error| format!("failed to download latest manifest: {error}"))?
        .error_for_status()
        .map_err(|error| format!("latest manifest request failed: {error}"))?
        .json()
        .map_err(|error| format!("failed to parse latest manifest: {error}"))?;
    latest.validate_for(environment)?;

    let manifest: RuntimeManifest = client
        .get(&latest.runtime_manifest_url)
        .send()
        .map_err(|error| format!("failed to download runtime manifest: {error}"))?
        .error_for_status()
        .map_err(|error| format!("runtime manifest request failed: {error}"))?
        .json()
        .map_err(|error| format!("failed to parse runtime manifest: {error}"))?;
    manifest.validate_for(environment)?;

    let client_file = select_desktop_artifact(&manifest, &platform)?;
    let game_file = select_game_client_artifact(&manifest, &platform)?;

    let temp_dir = requested_root.join(".mira-installer");
    replace_dir(&temp_dir)?;

    let client_download = temp_dir.join(format!(
        "mira-client-download{}",
        platform.package_extension
    ));
    let game_download = temp_dir.join(game_client_filename(&platform));

    download_file(
        &app,
        &client,
        &client_file,
        &client_download,
        "install-status-download-client",
        0.12,
        0.45,
    )?;
    download_file(
        &app,
        &client,
        &game_file,
        &game_download,
        "install-status-download-game",
        0.45,
        0.72,
    )?;

    emit_progress(&app, "install-status-finalize", 0.75);
    let launcher_path = requested_root.join(launcher_filename(&platform));
    remove_legacy_install_entries(&requested_root)?;

    let launcher_path = if platform.os == "windows" {
        install_windows_launcher(&client_download, &requested_root)?;
        resolve_windows_launcher_path(&requested_root)?
    } else {
        remove_if_exists(&launcher_path)?;
        fs::copy(&client_download, &launcher_path)
            .map_err(|error| format!("failed to install launcher: {error}"))?;
        make_executable(&launcher_path)?;
        launcher_path
    };

    let install_root = launcher_path
        .parent()
        .ok_or_else(|| "installed client folder could not be determined".to_string())?
        .to_path_buf();

    if install_root != requested_root {
        remove_legacy_install_entries(&install_root)?;
    }

    // Content now belongs to the desktop client's application-data directory.
    // Remove only the legacy installer-owned copy so it cannot be selected later.
    remove_if_exists(&install_root.join("assets"))?;

    let game_path = install_root.join(game_client_filename(&platform));
    remove_if_exists(&game_path)?;

    fs::copy(&game_download, &game_path)
        .map_err(|error| format!("failed to install game client: {error}"))?;
    make_executable(&game_path)?;

    let _ = fs::remove_dir_all(&temp_dir);

    emit_progress(&app, "install-status-done", 1.0);
    Ok(InstallResult {
        launcher_path: launcher_path.to_string_lossy().to_string(),
    })
}

/// Runs the detect platform info step for the installer Tauri backend system.
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

/// Runs the detect linux family step for the installer Tauri backend system.
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

/// Returns the deployment environment embedded by the installer build script.
fn build_environment() -> Result<Environment, String> {
    option_env!("MIRA_ENV")
        .ok_or_else(|| {
            "MIRA_ENV was not embedded in this installer build. Build with MIRA_ENV=dev, MIRA_ENV=staging, or MIRA_ENV=prod."
                .to_string()
        })?
        .parse()
}

/// Selects the desktop client installer for the current operating system.
fn select_desktop_artifact(
    manifest: &RuntimeManifest,
    platform: &PlatformInfo,
) -> Result<Artifact, String> {
    match platform.os.as_str() {
        "windows" => Ok(manifest.desktop.windows.clone()),
        "macos" => Ok(manifest.desktop.macos.clone()),
        "linux" => match platform.package_extension.as_str() {
            ".deb" => Ok(manifest.desktop.linux.deb.clone()),
            ".rpm" => Ok(manifest.desktop.linux.rpm.clone()),
            ".AppImage" => Ok(manifest.desktop.linux.app_image.clone()),
            extension => Err(format!(
                "Unsupported Linux desktop package extension: {extension}"
            )),
        },
        other => Err(format!("Unsupported installer platform: {other}")),
    }
}

/// Selects the standalone game client executable for the current operating system.
fn select_game_client_artifact(
    manifest: &RuntimeManifest,
    platform: &PlatformInfo,
) -> Result<Artifact, String> {
    match platform.os.as_str() {
        "windows" => Ok(manifest.game_client.windows.clone()),
        "linux" => Ok(manifest.game_client.linux.clone()),
        "macos" => Ok(manifest.game_client.macos.clone()),
        other => Err(format!("Unsupported installer platform: {other}")),
    }
}

/// Runs the launcher filename step for the installer Tauri backend system.
fn launcher_filename(platform: &PlatformInfo) -> &'static str {
    match platform.os.as_str() {
        "windows" => "mira-client.exe",
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

/// Runs the game client filename step for the installer Tauri backend system.
fn game_client_filename(platform: &PlatformInfo) -> &'static str {
    match platform.os.as_str() {
        "windows" => "mira-game-client.exe",
        _ => "mira-game-client",
    }
}

/// Runs the remove legacy install entries step for the installer Tauri backend system.
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

/// Runs the install windows launcher step for the installer Tauri backend system.
fn install_windows_launcher(installer_path: &Path, install_dir: &Path) -> Result<(), String> {
    let install_dir_arg = format!("/D={}", install_dir.to_string_lossy());
    let status = Command::new(installer_path)
        .arg("/S")
        .arg(install_dir_arg)
        .status()
        .map_err(|error| format!("failed to run client installer: {error}"))?;

    if status.success() {
        return Ok(());
    }

    if resolve_windows_launcher_path(install_dir).is_ok() {
        return Ok(());
    }

    Err(format!(
        "client installer failed with exit code {}",
        status
            .code()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    ))
}

/// Runs the resolve windows launcher path step for the installer Tauri backend system.
fn resolve_windows_launcher_path(install_dir: &Path) -> Result<PathBuf, String> {
    let mut candidates = vec![
        install_dir.join("mira-client.exe"),
        install_dir.join("Mira Client.exe"),
        install_dir.join("Mira Client").join("mira-client.exe"),
        install_dir.join("Mira Client").join("Mira Client.exe"),
    ];

    for env_key in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        if let Some(base_dir) = std::env::var_os(env_key).map(PathBuf::from) {
            candidates.push(base_dir.join("Mira Client").join("mira-client.exe"));
            candidates.push(base_dir.join("Mira Client").join("Mira Client.exe"));
            candidates.push(
                base_dir
                    .join("Mira Games")
                    .join("Mira Moba")
                    .join("mira-client.exe"),
            );
            candidates.push(
                base_dir
                    .join("Mira Games")
                    .join("Mira Moba")
                    .join("Mira Client.exe"),
            );
        }
    }

    let started_at = Instant::now();
    loop {
        if let Some(candidate) = candidates.iter().find(|candidate| candidate.is_file()) {
            return Ok(candidate.to_path_buf());
        }

        if started_at.elapsed() >= WINDOWS_INSTALLER_EXE_WAIT {
            return Err({
                let checked_paths = candidates
                    .iter()
                    .map(|candidate| candidate.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(", ");

                format!("installed client executable was not found. Checked paths: {checked_paths}")
            });
        }

        thread::sleep(Duration::from_millis(500));
    }
}

/// Runs the normalize install error code step for the installer Tauri backend system.
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

/// Runs the launch path step for the installer Tauri backend system.
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

/// Runs the configure linux webkit command step for the installer Tauri backend system.
#[cfg(target_os = "linux")]
fn configure_linux_webkit_command(command: &mut Command) {
    command.env_remove("WEBKIT_DISABLE_DMABUF_RENDERER");
    command.env_remove("WEBKIT_DISABLE_COMPOSITING_MODE");
    command.env_remove("GDK_BACKEND");
    command.env_remove("LIBGL_ALWAYS_SOFTWARE");
}

/// Runs the configure linux webkit command step for the installer Tauri backend system.
#[cfg(not(target_os = "linux"))]
fn configure_linux_webkit_command(_command: &mut Command) {}

/// Runs the download file step for the installer Tauri backend system.
fn download_file(
    app: &tauri::AppHandle,
    client: &Client,
    artifact: &Artifact,
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
        .get(&artifact.url)
        .send()
        .map_err(|error| format!("failed to download artifact: {error}"))?
        .error_for_status()
        .map_err(|error| format!("download request failed: {error}"))?;

    let total = response
        .content_length()
        .filter(|length| *length > 0)
        .unwrap_or(artifact.size);
    let mut file = File::create(&temp_destination)
        .map_err(|error| format!("failed to create download file: {error}"))?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];

    emit_progress(app, label_key, start);
    loop {
        let bytes_read = response
            .read(&mut buffer)
            .map_err(|error| format!("failed while downloading artifact: {error}"))?;

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
    if let Err(error) = verify_download(artifact, downloaded, &actual_sha256) {
        let _ = fs::remove_file(&temp_destination);
        return Err(error);
    }

    fs::rename(&temp_destination, destination)
        .map_err(|error| format!("failed to finalize download file: {error}"))?;
    emit_progress(app, label_key, end);
    Ok(())
}

/// Runs the replace dir step for the installer Tauri backend system.
fn replace_dir(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_dir_all(path)
            .map_err(|error| format!("failed to remove existing folder: {error}"))?;
    }

    fs::create_dir_all(path).map_err(|error| format!("failed to create folder: {error}"))
}

/// Runs the remove if exists step for the installer Tauri backend system.
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

/// Runs the make executable step for the installer Tauri backend system.
#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    let mut permissions = fs::metadata(path)
        .map_err(|error| format!("failed to read file metadata: {error}"))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .map_err(|error| format!("failed to mark file executable: {error}"))
}

/// Runs the make executable step for the installer Tauri backend system.
#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

/// Runs the part path step for the installer Tauri backend system.
fn part_path(destination: &Path) -> Result<PathBuf, String> {
    let filename = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "download destination has no filename".to_string())?;
    Ok(destination.with_file_name(format!("{filename}.part")))
}

/// Runs the emit progress step for the installer Tauri backend system.
fn emit_progress(app: &tauri::AppHandle, label_key: &'static str, progress: f32) {
    let _ = app.emit(
        "installer:progress",
        InstallProgress {
            label_key: label_key.to_string(),
            progress: progress.clamp(0.0, 1.0),
        },
    );
}

/// Runs the to hex step for the installer Tauri backend system.
fn to_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn verify_download(
    artifact: &Artifact,
    downloaded: u64,
    actual_sha256: &str,
) -> Result<(), String> {
    if downloaded != artifact.size {
        return Err(format!(
            "download size mismatch: expected {} bytes, got {downloaded}",
            artifact.size
        ));
    }
    if !artifact.sha256.eq_ignore_ascii_case(actual_sha256) {
        return Err("download checksum mismatch".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(name: &str) -> Artifact {
        Artifact {
            url: format!("https://downloads.tilt-us.com/{name}"),
            sha256: "a".repeat(64),
            size: 42,
        }
    }

    fn runtime_manifest() -> RuntimeManifest {
        RuntimeManifest {
            schema_version: 1,
            environment: Environment::Dev,
            desktop: mira_downloads::DesktopArtifacts {
                windows: artifact("desktop/windows/mira-client.exe"),
                linux: mira_downloads::LinuxDesktopArtifacts {
                    app_image: artifact("desktop/linux/mira-client.AppImage"),
                    deb: artifact("desktop/linux/mira-client.deb"),
                    rpm: artifact("desktop/linux/mira-client.rpm"),
                },
                macos: artifact("desktop/macos/mira-client.dmg"),
            },
            game_client: mira_downloads::PlatformArtifacts {
                windows: artifact("game/windows/mira-game-client.exe"),
                linux: artifact("game/linux/mira-game-client"),
                macos: artifact("game/macos/mira-game-client"),
            },
        }
    }

    #[test]
    fn maps_all_latest_manifest_urls() {
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
    }

    #[test]
    fn selects_runtime_artifacts_for_the_current_platform() {
        let manifest = runtime_manifest();
        let linux = PlatformInfo {
            os: "linux".to_string(),
            linux_family: Some("debian".to_string()),
            package_extension: ".deb".to_string(),
        };

        assert!(
            select_desktop_artifact(&manifest, &linux)
                .unwrap()
                .url
                .ends_with("mira-client.deb")
        );
        assert!(
            select_game_client_artifact(&manifest, &linux)
                .unwrap()
                .url
                .ends_with("mira-game-client")
        );
    }

    #[test]
    fn parses_and_validates_a_runtime_manifest() {
        let encoded = serde_json::to_string(&runtime_manifest()).unwrap();
        let parsed: RuntimeManifest = serde_json::from_str(&encoded).unwrap();
        parsed.validate_for(Environment::Dev).unwrap();
    }

    #[test]
    fn verifies_size_and_checksum() {
        let expected = Artifact {
            url: "https://downloads.tilt-us.com/runtime/game/linux/mira-game-client".to_string(),
            sha256: "abc".to_string(),
            size: 3,
        };
        assert!(verify_download(&expected, 3, "abc").is_ok());
        assert!(verify_download(&expected, 2, "abc").is_err());
        assert!(verify_download(&expected, 3, "def").is_err());
    }

    #[test]
    fn network_failures_are_reported_as_server_errors() {
        assert_eq!(
            normalize_install_error_code("failed to download artifact: connection reset"),
            ERROR_CODE_SERVER_NO_RESPONSE
        );
    }
}
