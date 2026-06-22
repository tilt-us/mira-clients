use super::{
    CurrentChampionVisual,
    characters::{
        ignara::{IgnaraECastState, IgnaraQCastState, IgnaraWCastState},
        lira::{LiraECastState, LiraQCastState, LiraWCastState},
        sophia::{SophiaECastState, SophiaQCastState, SophiaWCastState},
        yuna::{YunaECastState, YunaQCastState, YunaWCastState},
    },
};
use bevy::prelude::*;
use game_shared::game::player::{Health, PlayerControlled};
use game_shared::network::ChampionId;

#[derive(Resource, Debug, Clone, PartialEq)]
/// Description:
/// Stores gameplay values that the client HUD can render without depending on
/// private gameplay resources.
///
/// Fields:
/// - `health_current`: Current health of the locally controlled player.
/// - `health_max`: Maximum health of the locally controlled player.
/// - `health_percent`: Current health ratio in percent.
/// - `alive`: Whether the locally controlled player can move and cast.
/// - `respawn_seconds`: Remaining server-authoritative respawn time.
/// - `champion_name`: Display name of the locally controlled champion.
/// - `champion_initial`: Portrait initial for the locally controlled champion.
/// - `q_name`: Q ability display name.
/// - `q_cooldown_remaining`: Remaining local Q cooldown in seconds.
/// - `q_cooldown_total`: Total local Q cooldown in seconds.
/// - `q_ready_percent`: Q readiness ratio in percent.
/// - `w_name`: W ability display name.
/// - `w_cooldown_remaining`: Remaining local W cooldown in seconds.
/// - `w_cooldown_total`: Total local W cooldown in seconds.
/// - `w_ready_percent`: W readiness ratio in percent.
/// - `e_name`: E ability display name.
/// - `e_cooldown_remaining`: Remaining local E cooldown in seconds.
/// - `e_cooldown_total`: Total local E cooldown in seconds.
/// - `e_ready_percent`: E readiness ratio in percent.
pub struct MiraHudState {
    /// Current health of the locally controlled player.
    pub health_current: u32,
    /// Maximum health of the locally controlled player.
    pub health_max: u32,
    /// Current health ratio in percent.
    pub health_percent: f32,
    /// Whether the locally controlled player can move and cast.
    pub alive: bool,
    /// Remaining server-authoritative respawn time.
    pub respawn_seconds: f32,
    /// Display name of the locally controlled champion.
    pub champion_name: String,
    /// Portrait initial for the locally controlled champion.
    pub champion_initial: String,
    /// Q ability display name.
    pub q_name: String,
    /// Remaining local Q cooldown in seconds.
    pub q_cooldown_remaining: f32,
    /// Total local Q cooldown in seconds.
    pub q_cooldown_total: f32,
    /// Q readiness ratio in percent.
    pub q_ready_percent: f32,
    /// W ability display name.
    pub w_name: String,
    /// Remaining local W cooldown in seconds.
    pub w_cooldown_remaining: f32,
    /// Total local W cooldown in seconds.
    pub w_cooldown_total: f32,
    /// W readiness ratio in percent.
    pub w_ready_percent: f32,
    /// E ability display name.
    pub e_name: String,
    /// Remaining local E cooldown in seconds.
    pub e_cooldown_remaining: f32,
    /// Total local E cooldown in seconds.
    pub e_cooldown_total: f32,
    /// E readiness ratio in percent.
    pub e_ready_percent: f32,
}

impl Default for MiraHudState {
    fn default() -> Self {
        Self {
            health_current: 100,
            health_max: 100,
            health_percent: 100.0,
            alive: true,
            respawn_seconds: 0.0,
            champion_name: "Lira".to_string(),
            champion_initial: "L".to_string(),
            q_name: "Piercing Bolt".to_string(),
            q_cooldown_remaining: 0.0,
            q_cooldown_total: 1.0,
            q_ready_percent: 100.0,
            w_name: "Arc Burst".to_string(),
            w_cooldown_remaining: 0.0,
            w_cooldown_total: 1.0,
            w_ready_percent: 100.0,
            e_name: "Orbit Missiles".to_string(),
            e_cooldown_remaining: 0.0,
            e_cooldown_total: 1.0,
            e_ready_percent: 100.0,
        }
    }
}

impl MiraHudState {
    /// Description:
    /// Updates the server-provided respawn timer while preserving other HUD values.
    ///
    /// Params:
    /// - `respawn_seconds`: Remaining respawn time reported by the server.
    pub(in crate::systems) fn set_respawn_seconds(&mut self, respawn_seconds: f32) {
        self.respawn_seconds = respawn_seconds.max(0.0);
    }
}

/// Description:
/// Mirrors local gameplay state into a compact HUD resource.
///
/// Params:
/// - `player_query`: Locally controlled player health used for the HUD health block.
/// - `q_state`: Local Q cooldown state.
/// - `w_state`: Local W cooldown state.
/// - `e_state`: Local E cooldown state.
/// - `hud_state`: Mutable HUD state resource consumed by the client UI layer.
pub(super) fn update_mira_hud_state(
    player_query: Query<(&Health, &CurrentChampionVisual), With<PlayerControlled>>,
    lira_q_state: Res<LiraQCastState>,
    lira_w_state: Res<LiraWCastState>,
    lira_e_state: Res<LiraECastState>,
    ignara_q_state: Res<IgnaraQCastState>,
    ignara_w_state: Res<IgnaraWCastState>,
    ignara_e_state: Res<IgnaraECastState>,
    yuna_q_state: Res<YunaQCastState>,
    yuna_w_state: Res<YunaWCastState>,
    yuna_e_state: Res<YunaECastState>,
    sophia_q_state: Res<SophiaQCastState>,
    sophia_w_state: Res<SophiaWCastState>,
    sophia_e_state: Res<SophiaECastState>,
    mut hud_state: ResMut<MiraHudState>,
) {
    if let Ok((health, visual)) = player_query.single() {
        hud_state.health_current = health.current;
        hud_state.health_max = health.max.max(1);
        hud_state.health_percent = health.current as f32 / hud_state.health_max as f32 * 100.0;
        hud_state.alive = health.current > 0;

        match visual.champion {
            Some(ChampionId(6607)) => {
                hud_state.champion_name = "Ignara".to_string();
                hud_state.champion_initial = "I".to_string();
                hud_state.q_name = "Burning Ground".to_string();
                hud_state.q_cooldown_remaining = ignara_q_state.remaining_seconds();
                hud_state.q_cooldown_total = ignara_q_state.total_seconds();
                hud_state.q_ready_percent = ignara_q_state.ready_percent();

                hud_state.w_name = "Fireball".to_string();
                hud_state.w_cooldown_remaining = ignara_w_state.remaining_seconds();
                hud_state.w_cooldown_total = ignara_w_state.total_seconds();
                hud_state.w_ready_percent = ignara_w_state.ready_percent();

                hud_state.e_name = "Rolling Inferno".to_string();
                hud_state.e_cooldown_remaining = ignara_e_state.remaining_seconds();
                hud_state.e_cooldown_total = ignara_e_state.total_seconds();
                hud_state.e_ready_percent = ignara_e_state.ready_percent();
            }
            Some(ChampionId(6608)) => {
                hud_state.champion_name = "Yuna".to_string();
                hud_state.champion_initial = "Y".to_string();
                hud_state.q_name = "Gravity Orb".to_string();
                hud_state.q_cooldown_remaining = yuna_q_state.remaining_seconds();
                hud_state.q_cooldown_total = yuna_q_state.total_seconds();
                hud_state.q_ready_percent = yuna_q_state.ready_percent();

                hud_state.w_name = "Renewal Field".to_string();
                hud_state.w_cooldown_remaining = yuna_w_state.remaining_seconds();
                hud_state.w_cooldown_total = yuna_w_state.total_seconds();
                hud_state.w_ready_percent = yuna_w_state.ready_percent();

                hud_state.e_name = "Stasis Bolt".to_string();
                hud_state.e_cooldown_remaining = yuna_e_state.remaining_seconds();
                hud_state.e_cooldown_total = yuna_e_state.total_seconds();
                hud_state.e_ready_percent = yuna_e_state.ready_percent();
            }
            Some(ChampionId(6609)) => {
                hud_state.champion_name = "Sophia".to_string();
                hud_state.champion_initial = "S".to_string();
                hud_state.q_name = "Head Star".to_string();
                hud_state.q_cooldown_remaining = sophia_q_state.remaining_seconds();
                hud_state.q_cooldown_total = sophia_q_state.total_seconds();
                hud_state.q_ready_percent = sophia_q_state.ready_percent();

                hud_state.w_name = "Twin Cones".to_string();
                hud_state.w_cooldown_remaining = sophia_w_state.remaining_seconds();
                hud_state.w_cooldown_total = sophia_w_state.total_seconds();
                hud_state.w_ready_percent = sophia_w_state.ready_percent();

                hud_state.e_name = "Quick Spark".to_string();
                hud_state.e_cooldown_remaining = sophia_e_state.remaining_seconds();
                hud_state.e_cooldown_total = sophia_e_state.total_seconds();
                hud_state.e_ready_percent = sophia_e_state.ready_percent();
            }
            _ => {
                hud_state.champion_name = "Lira".to_string();
                hud_state.champion_initial = "L".to_string();
                hud_state.q_name = "Piercing Bolt".to_string();
                hud_state.q_cooldown_remaining = lira_q_state.remaining_seconds();
                hud_state.q_cooldown_total = lira_q_state.total_seconds();
                hud_state.q_ready_percent = lira_q_state.ready_percent();

                hud_state.w_name = "Arc Burst".to_string();
                hud_state.w_cooldown_remaining = lira_w_state.remaining_seconds();
                hud_state.w_cooldown_total = lira_w_state.total_seconds();
                hud_state.w_ready_percent = lira_w_state.ready_percent();

                hud_state.e_name = "Orbit Missiles".to_string();
                hud_state.e_cooldown_remaining = lira_e_state.remaining_seconds();
                hud_state.e_cooldown_total = lira_e_state.total_seconds();
                hud_state.e_ready_percent = lira_e_state.ready_percent();
            }
        }
    }
}
