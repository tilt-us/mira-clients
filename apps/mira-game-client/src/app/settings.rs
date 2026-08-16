use crate::environment::EnvironmentConfig;
use bevy::prelude::{Color, Resource};
use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_ACCENT_COLOR: &str = "#f2c45b";
const AUTH_VALIDATION_TIMEOUT: Duration = Duration::from_secs(5);
const ACCENT_COLOR_PREFIX: char = '#';
const RGB_CHANNEL_COUNT: usize = 3;
const HEX_DIGITS_PER_COLOR_CHANNEL: usize = 2;
const RGB_HEX_COLOR_LENGTH: usize = RGB_CHANNEL_COUNT * HEX_DIGITS_PER_COLOR_CHANNEL;
const HEX_RADIX: u32 = 16;
const SRGB_COLOR_CHANNEL_MAX: f32 = 255.0;
const AUTH_CURRENT_USER_PATH: &str = "/api/me";

/// Selects the window mode for the playable client.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ClientScreenMode {
    Full,
    Window,
    #[default]
    Borderless,
}

/// Records whether the client may continue from startup into gameplay.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub enum ClientLaunchGate {
    Playable,
    Blocked { message: String },
}

/// Stores process-level configuration for the playable client.
#[derive(Resource, Debug, Clone)]
pub struct ClientAppSettings {
    pub asset_root: PathBuf,
    pub ui_enabled: bool,
}

/// Stores launch parameters supplied by the matchmaking client wrapper.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct ClientLaunchSettings {
    pub access_token: Option<String>,
    pub accent_color: Option<String>,
    pub match_id: Option<String>,
    pub player_public_id: Option<String>,
    pub champion: Option<String>,
    pub server_host: Option<String>,
    pub server_port: Option<u16>,
    pub dev_preview: bool,
    pub screen_mode: ClientScreenMode,
}

impl ClientLaunchSettings {
    /// Returns a valid theme accent color inherited from the desktop client.
    pub fn accent_color_css(&self) -> &str {
        self.accent_color
            .as_deref()
            .filter(|color| parse_srgb_hex_color(color).is_some())
            .unwrap_or(DEFAULT_ACCENT_COLOR)
    }

    /// Converts the accent color into Bevy's color type for direct node updates.
    pub fn accent_color_bevy(&self) -> Color {
        let [red, green, blue] = parse_srgb_hex_color(self.accent_color_css())
            .expect("the default accent color must remain valid");

        Color::srgb(red, green, blue)
    }

    /// Returns the production launch validation result for this configuration.
    pub fn release_launch_gate(&self, environment: &EnvironmentConfig) -> ClientLaunchGate {
        if cfg!(debug_assertions) {
            return ClientLaunchGate::Playable;
        }

        if self.dev_preview {
            return blocked_launch_gate();
        }

        if self.access_token.as_deref().is_none_or(str::is_empty)
            || self.match_id.as_deref().is_none_or(str::is_empty)
            || self.player_public_id.as_deref().is_none_or(str::is_empty)
            || self.champion.as_deref().is_none_or(str::is_empty)
            || self.server_host.as_deref().is_none_or(str::is_empty)
            || self.server_port.is_none()
        {
            return blocked_launch_gate();
        }

        if !access_token_is_valid(environment, self.access_token.as_deref().unwrap()) {
            return blocked_launch_gate();
        }

        ClientLaunchGate::Playable
    }
}

impl ClientLaunchGate {
    /// Returns the message that explains why this launch is blocked.
    pub fn blocked_message(&self) -> Option<&str> {
        match self {
            ClientLaunchGate::Playable => None,
            ClientLaunchGate::Blocked { message } => Some(message),
        }
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
/// Checks whether the Extended UI HUD should be enabled for this client process.
fn client_ui_enabled() -> bool {
    std::env::var("MIRA_DISABLE_UI")
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            value != "1" && value != "true" && value != "yes"
        })
        .unwrap_or(true)
}

/// Validates and normalizes a CSS hexadecimal accent color.
pub(crate) fn normalize_accent_color(value: &str) -> Result<String, String> {
    let accent_color = value.trim();

    parse_srgb_hex_color(accent_color)
        .map(|_| accent_color.to_ascii_lowercase())
        .ok_or_else(|| format!("Invalid accent color: {value}"))
}

fn parse_srgb_hex_color(color: &str) -> Option<[f32; RGB_CHANNEL_COUNT]> {
    let rgb_hex = color.strip_prefix(ACCENT_COLOR_PREFIX)?;
    if rgb_hex.len() != RGB_HEX_COLOR_LENGTH
        || !rgb_hex.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return None;
    }

    let mut color_channels = [0.0; RGB_CHANNEL_COUNT];
    for (channel, hex_pair) in color_channels.iter_mut().zip(
        rgb_hex
            .as_bytes()
            .chunks_exact(HEX_DIGITS_PER_COLOR_CHANNEL),
    ) {
        *channel = parse_hex_color_channel(std::str::from_utf8(hex_pair).ok()?)?;
    }

    Some(color_channels)
}

fn parse_hex_color_channel(hex_pair: &str) -> Option<f32> {
    u8::from_str_radix(hex_pair, HEX_RADIX)
        .ok()
        .map(|channel| f32::from(channel) / SRGB_COLOR_CHANNEL_MAX)
}

fn blocked_launch_gate() -> ClientLaunchGate {
    ClientLaunchGate::Blocked {
        message:
            "Something goes wrong! Please close the client and start again via the desktop client!"
                .to_string(),
    }
}

fn access_token_is_valid(environment: &EnvironmentConfig, access_token: &str) -> bool {
    let Ok(url) = environment
        .auth_api_url()
        .join(AUTH_CURRENT_USER_PATH.trim_start_matches('/'))
    else {
        return false;
    };
    let Ok(client) = reqwest::blocking::Client::builder()
        .timeout(AUTH_VALIDATION_TIMEOUT)
        .build()
    else {
        return false;
    };

    client
        .get(url)
        .bearer_auth(access_token)
        .send()
        .map(|response| response.status().is_success())
        .unwrap_or(false)
}

/// Finds the game asset root for development, packaged, and direct binary runs.
fn resolve_asset_root() -> PathBuf {
    asset_root_candidates()
        .into_iter()
        .find(|candidate| has_required_game_content(candidate))
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

    if let Ok(current_exe) = std::env::current_exe()
        && let Some(exe_dir) = current_exe.parent()
    {
        candidates.push(exe_dir.join("assets"));
        candidates.push(exe_dir.join("..").join("assets"));
        candidates.push(exe_dir.join("..").join("..").join("assets"));
    }

    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("assets"),
    );

    candidates
}

fn has_required_game_content(asset_root: &std::path::Path) -> bool {
    ["audio", "champions", "maps", "materials"]
        .iter()
        .all(|directory| asset_root.join("game").join(directory).is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_valid_accent_colors() {
        assert_eq!(
            normalize_accent_color(" #F2C45B "),
            Ok("#f2c45b".to_string())
        );
    }

    #[test]
    fn falls_back_to_the_default_for_invalid_manual_accent_colors() {
        let settings = ClientLaunchSettings {
            accent_color: Some("not-a-color".to_string()),
            ..ClientLaunchSettings::default()
        };

        assert_eq!(settings.accent_color_css(), DEFAULT_ACCENT_COLOR);
    }
}
