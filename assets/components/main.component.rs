use crate::app::settings::ClientLaunchSettings;
use bevy::prelude::*;
use bevy_extended_ui::{
    BeuStore, framework::UiBindingStore, services::style_service::apply_calc_styles_system,
    styles::CssID,
};
use bevy_extended_ui_macros::*;
use game_logic::MiraHudState;
use serde::Serialize;

const HUD_UPDATE_INTERVAL_SECONDS: f32 = 0.25;
const HUD_HEALTH_FILL_ID: &str = "hud-health-fill";
const HUD_PORTRAIT_RING_ID: &str = "hud-portrait-ring";
const HUD_Q_KEY_ID: &str = "hud-q-key";
const HUD_W_KEY_ID: &str = "hud-w-key";
const HUD_E_KEY_ID: &str = "hud-e-key";
const HUD_Q_FILL_ID: &str = "hud-q-fill";
const HUD_W_FILL_ID: &str = "hud-w-fill";
const HUD_E_FILL_ID: &str = "hud-e-fill";

#[ui_component]
/// Description:
/// Defines the Extended UI main component metadata.
///
/// Fields:
/// - `template_name`: Component tag name registered with the UI framework.
/// - `template_file`: HTML template file loaded for the component.
/// - `styles`: CSS files loaded for the component.
pub struct MainComponent {
    pub template_name: &'static str,
    pub template_file: &'static str,
    pub styles: &'static [&'static str],
}

pub const MAIN_COMPONENT: MainComponent = MainComponent {
    template_name: "app-main",
    template_file: "main.component.html",
    styles: &["main.component.css"],
};

#[derive(Debug, Clone, PartialEq, Serialize, BeuStore)]
/// Description:
/// Stores template-visible HUD values for the main UI component.
///
/// Fields:
/// - `hud_health_text`: Current health text shown in the HUD.
/// - `hud_status_text`: Local player life-state label.
/// - `hud_respawn_text`: Respawn status text shown under the match strip.
/// - `hud_champion_name`: Local champion display name.
/// - `hud_champion_initial`: Local champion portrait initial.
/// - `hud_q_key`: Q ability key label.
/// - `hud_q_name`: Q ability display name.
/// - `hud_q_cooldown`: Q cooldown display text.
/// - `hud_w_key`: W ability key label.
/// - `hud_w_name`: W ability display name.
/// - `hud_w_cooldown`: W cooldown display text.
/// - `hud_e_key`: E ability key label.
/// - `hud_e_name`: E ability display name.
/// - `hud_e_cooldown`: E cooldown display text.
pub struct MainHudStore {
    pub hud_health_text: String,
    pub hud_status_text: String,
    pub hud_respawn_text: String,
    pub hud_champion_name: String,
    pub hud_champion_initial: String,
    pub hud_q_key: String,
    pub hud_q_name: String,
    pub hud_q_cooldown: String,
    pub hud_w_key: String,
    pub hud_w_name: String,
    pub hud_w_cooldown: String,
    pub hud_e_key: String,
    pub hud_e_name: String,
    pub hud_e_cooldown: String,
}

impl Default for MainHudStore {
    fn default() -> Self {
        Self {
            hud_health_text: "100/100".to_string(),
            hud_status_text: "LIVE".to_string(),
            hud_respawn_text: "Respawn ready".to_string(),
            hud_champion_name: "Lira".to_string(),
            hud_champion_initial: "L".to_string(),
            hud_q_key: "Q".to_string(),
            hud_q_name: "Piercing Bolt".to_string(),
            hud_q_cooldown: "READY".to_string(),
            hud_w_key: "W".to_string(),
            hud_w_name: "Arc Burst".to_string(),
            hud_w_cooldown: "READY".to_string(),
            hud_e_key: "E".to_string(),
            hud_e_name: "Orbit Missiles".to_string(),
            hud_e_cooldown: "READY".to_string(),
        }
    }
}

#[derive(Resource, Debug)]
/// Description:
/// Limits main HUD store updates to stable visual ticks instead of every frame.
///
/// Fields:
/// - `0`: Repeating timer used for main HUD store updates.
struct MainHudStoreUpdateTimer(Timer);

#[derive(Resource, Debug, Default)]
/// Description:
/// Caches the last HUD store written to Extended UI.
///
/// Fields:
/// - `0`: Last store value sent to `UiBindingStore`.
struct MainHudStoreCache(Option<MainHudStore>);

impl Default for MainHudStoreUpdateTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(
            HUD_UPDATE_INTERVAL_SECONDS,
            TimerMode::Repeating,
        ))
    }
}

/// Description:
/// Registers runtime systems owned by the main Extended UI component.
pub struct MainComponentPlugin;

impl Plugin for MainComponentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MainHudStoreUpdateTimer>()
            .init_resource::<MainHudStoreCache>()
            .add_systems(Update, sync_main_hud_store)
            .add_systems(
                PostUpdate,
                sync_hud_runtime_styles.after(apply_calc_styles_system),
            );
    }
}

/// Description:
/// Initializes the main UI component.
#[component_init]
pub fn constructor() {}

/// Description:
/// Mirrors gameplay HUD state into the main component `BeuStore`.
///
/// Params:
/// - `timer`: Local update timer used to avoid rebuilding UI every frame.
/// - `time`: Frame timing used to advance the update timer.
/// - `hud_state`: Gameplay HUD state provided by the systems crate.
/// - `cache`: Last HUD store value written to Extended UI.
/// - `ui_store`: Extended UI binding store consumed by `{{ main_hud_store.* }}` placeholders.
fn sync_main_hud_store(
    mut timer: ResMut<MainHudStoreUpdateTimer>,
    time: Res<Time>,
    hud_state: Res<MiraHudState>,
    mut cache: ResMut<MainHudStoreCache>,
    mut ui_store: ResMut<UiBindingStore>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    let next_store = MainHudStore {
        hud_health_text: format!("{}/{}", hud_state.health_current, hud_state.health_max),
        hud_status_text: if hud_state.alive {
            "LIVE".to_string()
        } else {
            "DEAD".to_string()
        },
        hud_respawn_text: if hud_state.alive {
            "Respawn ready".to_string()
        } else {
            format!("Respawn in {}s", ceil_seconds(hud_state.respawn_seconds))
        },
        hud_champion_name: hud_state.champion_name.clone(),
        hud_champion_initial: hud_state.champion_initial.clone(),
        hud_q_key: "Q".to_string(),
        hud_q_name: hud_state.q_name.clone(),
        hud_q_cooldown: cooldown_text(hud_state.q_cooldown_remaining),
        hud_w_key: "W".to_string(),
        hud_w_name: hud_state.w_name.clone(),
        hud_w_cooldown: cooldown_text(hud_state.w_cooldown_remaining),
        hud_e_key: "E".to_string(),
        hud_e_name: hud_state.e_name.clone(),
        hud_e_cooldown: cooldown_text(hud_state.e_cooldown_remaining),
    };

    if cache.0.as_ref() == Some(&next_store) {
        return;
    }

    ui_store.set_store(next_store.clone());
    cache.0 = Some(next_store);
}

/// Description:
/// Applies runtime-only HUD styles that depend on gameplay state or launcher theme.
///
/// Params:
/// - `hud_state`: Gameplay HUD state that provides health and ability readiness.
/// - `launch_settings`: Startup settings containing the desktop accent color.
/// - `nodes`: Extended UI nodes addressable by CSS id.
fn sync_hud_runtime_styles(
    hud_state: Res<MiraHudState>,
    launch_settings: Res<ClientLaunchSettings>,
    mut nodes: Query<(
        &CssID,
        &mut Node,
        Option<&mut BackgroundColor>,
        Option<&mut BorderColor>,
    )>,
) {
    let accent_color = launch_settings.accent_color_bevy();

    for (css_id, mut node, background, border) in &mut nodes {
        match css_id.0.as_str() {
            HUD_HEALTH_FILL_ID => node.width = percent_width(hud_state.health_percent),
            HUD_Q_FILL_ID => node.width = percent_width(hud_state.q_ready_percent),
            HUD_W_FILL_ID => node.width = percent_width(hud_state.w_ready_percent),
            HUD_E_FILL_ID => node.width = percent_width(hud_state.e_ready_percent),
            HUD_PORTRAIT_RING_ID | HUD_Q_KEY_ID | HUD_W_KEY_ID | HUD_E_KEY_ID => {
                if let Some(mut background) = background {
                    *background = BackgroundColor(accent_color);
                }

                if let Some(mut border) = border {
                    border.set_all(accent_color);
                }
            }
            _ => {}
        }
    }
}

fn percent_width(percent: f32) -> Val {
    Val::Percent(percent.clamp(0.0, 100.0))
}

/// Description:
/// Formats a cooldown duration for compact HUD display.
///
/// Params:
/// - `remaining_seconds`: Remaining cooldown duration in seconds.
///
/// Returns:
/// - `READY` when available, otherwise a one-decimal cooldown string.
fn cooldown_text(remaining_seconds: f32) -> String {
    if remaining_seconds <= 0.05 {
        return "READY".to_string();
    }

    format!("{}s", ceil_seconds(remaining_seconds))
}

/// Description:
/// Converts a duration into a compact whole-second countdown value.
///
/// Params:
/// - `seconds`: Remaining duration in seconds.
///
/// Returns:
/// - Whole seconds rounded up, clamped to zero.
fn ceil_seconds(seconds: f32) -> u32 {
    seconds.max(0.0).ceil() as u32
}
