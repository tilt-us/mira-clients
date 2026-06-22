use bevy::prelude::{Color, Resource};
use std::path::PathBuf;

const DEFAULT_ACCENT_COLOR: &str = "#f2c45b";

/// Description:
/// Stores process-level settings for the playable client app.
///
/// Fields:
/// - `ui_enabled`: Whether the Extended UI HUD should be registered.
#[derive(Resource, Debug, Clone)]
pub struct ClientAppSettings {
    pub asset_root: PathBuf,
    pub ui_enabled: bool,
}

/// Description:
/// Stores launch parameters supplied by the matchmaking client wrapper.
///
/// Fields:
/// - `access_token`: Bearer token used for authenticated matchmaking requests.
/// - `accent_color`: Hex color inherited from the desktop client theme.
/// - `match_id`: Match identifier assigned by matchmaking.
/// - `player_public_id`: Public player id assigned by the platform.
/// - `champion`: Requested champion slug or id.
/// - `matchmaking_api_base_url`: Base URL of the matchmaking API.
/// - `server_control_base_url`: Base URL of the dedicated server control API.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct ClientLaunchSettings {
    pub access_token: Option<String>,
    pub accent_color: Option<String>,
    pub match_id: Option<String>,
    pub player_public_id: Option<String>,
    pub champion: Option<String>,
    pub matchmaking_api_base_url: Option<String>,
    pub server_control_base_url: Option<String>,
}

impl ClientLaunchSettings {
    /// Description:
    /// Returns the validated theme accent color inherited from the desktop client.
    pub fn accent_color_css(&self) -> &str {
        self.accent_color.as_deref().unwrap_or(DEFAULT_ACCENT_COLOR)
    }

    /// Description:
    /// Converts the accent color into Bevy's color type for direct node updates.
    pub fn accent_color_bevy(&self) -> Color {
        let color = self.accent_color_css();
        let red = parse_hex_pair(&color[1..3]);
        let green = parse_hex_pair(&color[3..5]);
        let blue = parse_hex_pair(&color[5..7]);

        Color::srgb(red, green, blue)
    }
}

impl Default for ClientAppSettings {
    fn default() -> Self {
        Self {
            asset_root: resolve_asset_root(),
            ui_enabled: client_ui_enabled(),
        }
    }
}

/// Description:
/// Checks whether the Extended UI HUD should be enabled for this client process.
///
/// Returns:
/// - `true` unless `MIRA_DISABLE_UI` is set to `1`, `true`, or `yes`.
fn client_ui_enabled() -> bool {
    std::env::var("MIRA_DISABLE_UI")
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            value != "1" && value != "true" && value != "yes"
        })
        .unwrap_or(true)
}

fn parse_hex_pair(value: &str) -> f32 {
    u8::from_str_radix(value, 16).unwrap_or(0) as f32 / 255.0
}

/// Description:
/// Finds the game asset root for dev runs, packaged desktop runs, and direct binary runs.
///
/// Return:
/// - Absolute path containing `index.html` and `components/`.
fn resolve_asset_root() -> PathBuf {
    asset_root_candidates()
        .into_iter()
        .find(|candidate| candidate.join("index.html").is_file())
        .and_then(|candidate| candidate.canonicalize().ok())
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("assets")
        })
}

fn asset_root_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(path) = std::env::var_os("MIRA_GAME_ASSET_ROOT") {
        candidates.push(PathBuf::from(path));
    }

    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("assets"));
        candidates.push(current_dir.join("..").join("assets"));
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            candidates.push(exe_dir.join("assets"));
            candidates.push(exe_dir.join("..").join("assets"));
            candidates.push(exe_dir.join("..").join("..").join("assets"));
        }
    }

    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("assets"),
    );

    candidates
}
