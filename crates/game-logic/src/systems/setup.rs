use super::{
    CurrentChampionVisual, HoldMoveDirection, LOCAL_CHAMPION_ID, LocalChampionAnimationState,
    LocalChampionAnimations, MoveTargetMarker, MoveTargetMarkerFx,
    characters::lira::{
        LiraECastState, LiraESettings, LiraQCastState, LiraQIndicatorBody, LiraQIndicatorTip,
        LiraQSettings, LiraWAoeIndicator, LiraWCastState, LiraWRangeIndicator, LiraWSettings,
    },
    characters::{
        ignara::{
            IgnaraECastState, IgnaraESettings, IgnaraQCastState, IgnaraQSettings, IgnaraWCastState,
            IgnaraWSettings,
        },
        sophia::{
            SophiaECastState, SophiaESettings, SophiaQCastState, SophiaQSettings, SophiaWCastState,
            SophiaWSettings,
        },
        yuna::{
            YunaECastState, YunaESettings, YunaQCastState, YunaQSettings, YunaWCastState,
            YunaWSettings,
        },
    },
    healthbar,
};
use bevy::ecs::system::SystemParam;
use bevy::gltf::GltfAssetLabel;
use bevy::math::primitives::{Cuboid, Cylinder};
use bevy::prelude::*;
use bevy_transform_interpolation::prelude::{RotationInterpolation, TranslationInterpolation};
use game_shared::game::{
    camera::TopDownCameraBundle,
    player::{PlayerBundle, PlayerId, PlayerProfile},
    team::TeamSpec,
};
use game_shared::network::{
    ChampionCatalogUpdate, ChampionId, NetworkChampionAbilities, NetworkChampionDefinition,
};
use lightyear::prelude::client::Client;
use lightyear::prelude::*;
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Deserialize)]
/// Description:
/// Represents the champion data loaded from the local champion JSON file.
///
/// Fields:
/// - `localized_name`: Directory slug used for local champion assets.
/// - `model_name`: GLB model filename loaded from the champion model directory.
/// - `animations`: Animation key-to-index mappings used to build the graph.
struct ChampionDataFile {
    localized_name: String,
    model_name: String,
    animations: Vec<ChampionAnimationEntry>,
}

#[derive(Debug, Deserialize)]
/// Description:
/// Represents one animation clip entry from a champion data file.
///
/// Fields:
/// - `key`: Logical animation name such as idle or walk.
/// - `index`: GLTF animation index for the animation clip.
struct ChampionAnimationEntry {
    key: String,
    index: usize,
}

/// Description:
/// Stores champion tuning received from the match server.
///
/// Fields:
/// - `champions`: Champion definitions keyed by their stable content id.
#[derive(Resource, Debug, Default, Clone)]
pub(super) struct ClientChampionCatalog {
    champions: HashMap<ChampionId, NetworkChampionDefinition>,
}

#[derive(SystemParam)]
pub(super) struct ClientChampionTuningParams<'w> {
    q_settings: ResMut<'w, LiraQSettings>,
    w_settings: ResMut<'w, LiraWSettings>,
    e_settings: ResMut<'w, LiraESettings>,
    q_cast_state: ResMut<'w, LiraQCastState>,
    w_cast_state: ResMut<'w, LiraWCastState>,
    e_cast_state: ResMut<'w, LiraECastState>,
    ignara_q_settings: ResMut<'w, IgnaraQSettings>,
    ignara_w_settings: ResMut<'w, IgnaraWSettings>,
    ignara_e_settings: ResMut<'w, IgnaraESettings>,
    ignara_q_cast_state: ResMut<'w, IgnaraQCastState>,
    ignara_w_cast_state: ResMut<'w, IgnaraWCastState>,
    ignara_e_cast_state: ResMut<'w, IgnaraECastState>,
    yuna_q_settings: ResMut<'w, YunaQSettings>,
    yuna_w_settings: ResMut<'w, YunaWSettings>,
    yuna_e_settings: ResMut<'w, YunaESettings>,
    yuna_q_cast_state: ResMut<'w, YunaQCastState>,
    yuna_w_cast_state: ResMut<'w, YunaWCastState>,
    yuna_e_cast_state: ResMut<'w, YunaECastState>,
    sophia_q_settings: ResMut<'w, SophiaQSettings>,
    sophia_w_settings: ResMut<'w, SophiaWSettings>,
    sophia_e_settings: ResMut<'w, SophiaESettings>,
    sophia_q_cast_state: ResMut<'w, SophiaQCastState>,
    sophia_w_cast_state: ResMut<'w, SophiaWCastState>,
    sophia_e_cast_state: ResMut<'w, SophiaECastState>,
}

/// Description:
/// Spawns the local player, camera, ability indicators, movement marker, and test dummies.
///
/// Params:
/// - `commands`: ECS command buffer used to spawn entities and insert resources.
/// - `asset_server`: Asset server used to load champion scene and animation assets.
/// - `q_settings`: Lira Q settings used for preview and predicted visuals.
/// - `w_settings`: Lira W settings used for preview and predicted visuals.
/// - `e_settings`: Lira E settings used for predicted visuals.
/// - `graphs`: Animation graph assets used to store the local champion graph.
/// - `meshes`: Mesh assets used by indicators, markers, dummies, and health bars.
/// - `materials`: Material assets used by indicators, markers, dummies, and health bars.
pub(super) fn spawn_local_player_and_camera(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    q_settings: Res<LiraQSettings>,
    w_settings: Res<LiraWSettings>,
    e_settings: Res<LiraESettings>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    health_bar_style: Res<super::healthbar::OverheadHealthBarStyle>,
) {
    let champion_data = load_champion_data(LOCAL_CHAMPION_ID).unwrap_or_else(|| {
        warn!(
            "Failed to load champion data for id {}. Falling back to defaults.",
            LOCAL_CHAMPION_ID
        );
        ChampionDataFile {
            localized_name: "lira".to_string(),
            model_name: "model.glb".to_string(),
            animations: vec![
                ChampionAnimationEntry {
                    key: "idle".to_string(),
                    index: 1,
                },
                ChampionAnimationEntry {
                    key: "walk".to_string(),
                    index: 5,
                },
            ],
        }
    });

    let q_settings = *q_settings;
    let w_settings = *w_settings;
    let e_settings = *e_settings;

    let champion_model_asset = format!(
        "game/champions/{}/{}",
        champion_data.localized_name, champion_data.model_name
    );
    let idle_clip_index = champion_data
        .animations
        .iter()
        .find(|entry| entry.key == "idle")
        .map(|entry| entry.index)
        .unwrap_or(0);
    let walk_clip_index = champion_data
        .animations
        .iter()
        .find(|entry| entry.key == "walk")
        .map(|entry| entry.index)
        .unwrap_or(idle_clip_index);

    let (graph, clip_nodes) = AnimationGraph::from_clips([
        asset_server.load(
            GltfAssetLabel::Animation(idle_clip_index).from_asset(champion_model_asset.clone()),
        ),
        asset_server.load(
            GltfAssetLabel::Animation(walk_clip_index).from_asset(champion_model_asset.clone()),
        ),
    ]);
    let graph_handle = graphs.add(graph);

    commands.insert_resource(LocalChampionAnimations {
        graph: graph_handle,
        idle: clip_nodes[0],
        walk: clip_nodes[1],
    });
    commands.insert_resource(LocalChampionAnimationState::default());
    commands.insert_resource(HoldMoveDirection(Vec3::Z));
    commands.insert_resource(LiraQCastState::ready(q_settings.cooldown_seconds));
    commands.insert_resource(LiraWCastState::ready(w_settings.cooldown_seconds));
    commands.insert_resource(LiraECastState::ready(e_settings.cooldown_seconds));

    let local_champion = ChampionId(LOCAL_CHAMPION_ID);
    let local_model_root = commands
        .spawn((
            Name::new("LocalPlayerLiraModel"),
            SceneRoot(
                asset_server
                    .load(GltfAssetLabel::Scene(0).from_asset(champion_model_asset.clone())),
            ),
            Transform::default(),
        ))
        .id();

    let player_entity = commands
        .spawn((
            Name::new("LocalPlayerLira"),
            PlayerBundle::new(PlayerId(1), TeamSpec::Light),
            CurrentChampionVisual {
                champion: Some(local_champion),
                model_root: Some(local_model_root),
            },
            PlayerProfile {
                display_name: "Player".to_string(),
            },
            TranslationInterpolation,
            RotationInterpolation,
            Transform::from_xyz(0.0, 0.0, 0.0),
        ))
        .id();
    commands.entity(player_entity).add_child(local_model_root);

    healthbar::spawn_player_health_bar(
        &mut commands,
        &asset_server,
        &mut meshes,
        &mut materials,
        player_entity,
        health_bar_style.accent_color,
    );

    commands.spawn((
        Name::new("TopDownCamera"),
        TopDownCameraBundle::default(),
        Camera3d::default(),
        Transform::from_xyz(4.8, 6.4, 4.8).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    let marker_mesh = meshes.add(Mesh::from(Cuboid::new(0.5, 0.05, 0.5)));
    let marker_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.85, 0.2),
        emissive: Color::srgb(0.4, 0.35, 0.05).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    commands.spawn((
        Name::new("MoveTargetMarker"),
        MoveTargetMarker,
        Mesh3d(marker_mesh),
        MeshMaterial3d(marker_material),
        MoveTargetMarkerFx::default(),
        Transform::from_xyz(0.0, 0.03, 0.0),
        Visibility::Hidden,
    ));

    let q_material = materials.add(StandardMaterial {
        base_color: q_settings.color(),
        emissive: q_settings.color().with_alpha(0.42).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    commands.spawn((
        Name::new("LiraQIndicatorBody"),
        LiraQIndicatorBody,
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)))),
        MeshMaterial3d(q_material.clone()),
        Transform::from_xyz(0.0, q_settings.elevation, 0.0),
        Visibility::Hidden,
    ));

    commands.spawn((
        Name::new("LiraQIndicatorTip"),
        LiraQIndicatorTip,
        Mesh3d(meshes.add(Mesh::from(Cylinder::new(1.0, 1.0)))),
        MeshMaterial3d(q_material),
        Transform::from_xyz(0.0, q_settings.elevation, 0.0),
        Visibility::Hidden,
    ));

    let w_range_material = materials.add(StandardMaterial {
        base_color: w_settings.color().with_alpha(w_settings.alpha * 0.45),
        emissive: w_settings.color().with_alpha(0.2).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let w_aoe_material = materials.add(StandardMaterial {
        base_color: w_settings.color(),
        emissive: w_settings.color().with_alpha(0.34).into(),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    commands.spawn((
        Name::new("LiraWRangeIndicator"),
        LiraWRangeIndicator,
        Mesh3d(meshes.add(Mesh::from(Cylinder::new(1.0, 1.0)))),
        MeshMaterial3d(w_range_material),
        Transform::from_xyz(0.0, w_settings.elevation, 0.0),
        Visibility::Hidden,
    ));

    commands.spawn((
        Name::new("LiraWAoeIndicator"),
        LiraWAoeIndicator,
        Mesh3d(meshes.add(Mesh::from(Cylinder::new(1.0, 1.0)))),
        MeshMaterial3d(w_aoe_material),
        Transform::from_xyz(0.0, w_settings.elevation + 0.02, 0.0),
        Visibility::Hidden,
    ));
}

/// Description:
/// Receives server-authoritative champion tuning from the match server.
///
/// Params:
/// - `receivers`: Lightyear message receivers containing champion catalog updates.
/// - `catalog`: Local copy of the latest received champion catalog.
/// - `q_settings`: Mutable Lira Q settings used by local prediction and previews.
/// - `w_settings`: Mutable Lira W settings used by local prediction and previews.
/// - `e_settings`: Mutable Lira E settings used by local prediction and previews.
/// - `q_cast_state`: Mutable Q cooldown state whose duration follows received tuning.
/// - `w_cast_state`: Mutable W cooldown state whose duration follows received tuning.
/// - `e_cast_state`: Mutable E cooldown state whose duration follows received tuning.
pub(super) fn receive_champion_catalog_updates(
    mut receivers: Query<&mut MessageReceiver<ChampionCatalogUpdate>, With<Client>>,
    mut catalog: ResMut<ClientChampionCatalog>,
    mut tuning: ClientChampionTuningParams,
) {
    for mut receiver in &mut receivers {
        for update in receiver.receive() {
            catalog.champions = update
                .champions
                .into_iter()
                .map(|champion| (champion.id, champion))
                .collect();

            let Some(lira) = catalog.champions.get(&ChampionId(6606)) else {
                continue;
            };
            apply_lira_prediction_tuning(
                &lira.stats.abilities,
                &mut tuning.q_settings,
                &mut tuning.w_settings,
                &mut tuning.e_settings,
            );
            *tuning.q_cast_state = LiraQCastState::ready(tuning.q_settings.cooldown_seconds);
            *tuning.w_cast_state = LiraWCastState::ready(tuning.w_settings.cooldown_seconds);
            *tuning.e_cast_state = LiraECastState::ready(tuning.e_settings.cooldown_seconds);

            if let Some(ignara) = catalog.champions.get(&ChampionId(6607)) {
                apply_ignara_prediction_tuning(
                    &ignara.stats.abilities,
                    &mut tuning.ignara_q_settings,
                    &mut tuning.ignara_w_settings,
                    &mut tuning.ignara_e_settings,
                );
                *tuning.ignara_q_cast_state =
                    IgnaraQCastState::ready(ignara.stats.abilities.q.cooldown_seconds);
                *tuning.ignara_w_cast_state =
                    IgnaraWCastState::ready(ignara.stats.abilities.w.cooldown_seconds);
                *tuning.ignara_e_cast_state =
                    IgnaraECastState::ready(ignara.stats.abilities.e.cooldown_seconds);
            }

            if let Some(yuna) = catalog.champions.get(&ChampionId(6608)) {
                apply_yuna_prediction_tuning(
                    &yuna.stats.abilities,
                    &mut tuning.yuna_q_settings,
                    &mut tuning.yuna_w_settings,
                    &mut tuning.yuna_e_settings,
                );
                *tuning.yuna_q_cast_state =
                    YunaQCastState::ready(yuna.stats.abilities.q.cooldown_seconds);
                *tuning.yuna_w_cast_state =
                    YunaWCastState::ready(yuna.stats.abilities.w.cooldown_seconds);
                *tuning.yuna_e_cast_state =
                    YunaECastState::ready(yuna.stats.abilities.e.cooldown_seconds);
            }

            if let Some(sophia) = catalog.champions.get(&ChampionId(6609)) {
                apply_sophia_prediction_tuning(
                    &sophia.stats.abilities,
                    &mut tuning.sophia_q_settings,
                    &mut tuning.sophia_w_settings,
                    &mut tuning.sophia_e_settings,
                );
                *tuning.sophia_q_cast_state =
                    SophiaQCastState::ready(sophia.stats.abilities.q.cooldown_seconds);
                *tuning.sophia_w_cast_state =
                    SophiaWCastState::ready(sophia.stats.abilities.w.cooldown_seconds);
                *tuning.sophia_e_cast_state =
                    SophiaECastState::ready(sophia.stats.abilities.e.cooldown_seconds);
            }

            info!(
                "Received server champion catalog with {} champions.",
                catalog.champions.len()
            );
        }
    }
}

/// Description:
/// Loads champion metadata from the local assets directory.
///
/// Params:
/// - `champion_id`: Numeric champion id used as the JSON filename.
///
/// Return:
/// - Parsed champion metadata, or `None` if reading or parsing fails.
fn load_champion_data(champion_id: u32) -> Option<ChampionDataFile> {
    let file_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/game/champions")
        .join(champion_slug(champion_id)?)
        .join("champion.json");

    let raw = match std::fs::read_to_string(&file_path) {
        Ok(raw) => raw,
        Err(error) => {
            warn!(
                "Failed to read champion data file {}: {}",
                file_path.display(),
                error
            );
            return None;
        }
    };

    match serde_json::from_str(&raw) {
        Ok(data) => Some(data),
        Err(error) => {
            warn!(
                "Failed to parse champion data file {}: {}",
                file_path.display(),
                error
            );
            None
        }
    }
}

/// Description:
/// Maps a champion content id to its asset directory slug.
///
/// Params:
/// - `champion_id`: Stable champion content id.
///
/// Return:
/// - Asset directory slug when the champion is known locally.
fn champion_slug(champion_id: u32) -> Option<&'static str> {
    match champion_id {
        6606 => Some("lira"),
        6607 => Some("ignara"),
        6608 => Some("yuna"),
        6609 => Some("sophia"),
        _ => None,
    }
}

/// Description:
/// Applies server-authored champion tuning to local prediction and preview resources.
///
/// Params:
/// - `abilities`: Server-authored ability tuning received from the match server.
/// - `q_settings`: Mutable Q settings used by local prediction and previews.
/// - `w_settings`: Mutable W settings used by local prediction and previews.
/// - `e_settings`: Mutable E settings used by local prediction and previews.
fn apply_lira_prediction_tuning(
    abilities: &NetworkChampionAbilities,
    q_settings: &mut LiraQSettings,
    w_settings: &mut LiraWSettings,
    e_settings: &mut LiraESettings,
) {
    let q = &abilities.q;
    q_settings.cooldown_seconds = positive_or(q.cooldown_seconds, q_settings.cooldown_seconds);
    q_settings.range = positive_or(q.range, q_settings.range);
    q_settings.travel_seconds = positive_or(q.travel_seconds, q_settings.travel_seconds);
    q_settings.projectile_height = positive_or(q.projectile_height, q_settings.projectile_height);
    q_settings.projectile_radius = positive_or(q.projectile_radius, q_settings.projectile_radius);
    q_settings.width = q_settings.projectile_radius * 2.0;
    q_settings.explosion_radius = positive_or(q.explosion_radius, q_settings.explosion_radius);
    q_settings.tip_radius = q_settings.explosion_radius.max(q_settings.tip_radius);
    q_settings.direct_hit_damage = positive_or(q.damage.direct_hit, q_settings.direct_hit_damage);
    q_settings.area_damage = positive_or(q.damage.area, q_settings.area_damage);

    let w = &abilities.w;
    w_settings.cooldown_seconds = positive_or(w.cooldown_seconds, w_settings.cooldown_seconds);
    w_settings.range = positive_or(w.range, w_settings.range);
    w_settings.travel_seconds = positive_or(w.travel_seconds, w_settings.travel_seconds);
    w_settings.projectile_height = positive_or(w.projectile_height, w_settings.projectile_height);
    w_settings.target_height = positive_or(w.target_height, w_settings.target_height);
    w_settings.aoe_radius = positive_or(w.explosion_radius, w_settings.aoe_radius);
    w_settings.area_damage = positive_or(w.damage.area, w_settings.area_damage);

    let e = &abilities.e;
    e_settings.cooldown_seconds = positive_or(e.cooldown_seconds, e_settings.cooldown_seconds);
    if e.missile_count > 0 {
        e_settings.missile_count = e.missile_count;
    }
    e_settings.lifetime_seconds =
        positive_or(e.missile_lifetime_seconds, e_settings.lifetime_seconds);
    e_settings.search_radius = positive_or(e.missile_search_radius, e_settings.search_radius);
    e_settings.orbit_radius = positive_or(e.missile_orbit_radius, e_settings.orbit_radius);
    e_settings.orbit_height = positive_or(e.missile_orbit_height, e_settings.orbit_height);
    e_settings.orbit_speed = positive_or(e.missile_orbit_speed, e_settings.orbit_speed);
    e_settings.chase_speed = positive_or(e.missile_chase_speed, e_settings.chase_speed);
    e_settings.missile_radius = positive_or(e.missile_radius, e_settings.missile_radius);
    e_settings.damage = positive_or(e.damage.missile, e_settings.damage);
}

fn apply_ignara_prediction_tuning(
    abilities: &NetworkChampionAbilities,
    q_settings: &mut IgnaraQSettings,
    w_settings: &mut IgnaraWSettings,
    e_settings: &mut IgnaraESettings,
) {
    let q = &abilities.q;
    q_settings.width = positive_or(q.projectile_radius * 2.0, q_settings.width);
    q_settings.range = positive_or(q.range, q_settings.range);
    q_settings.lifetime_seconds = positive_or(q.lifetime_seconds, q_settings.lifetime_seconds);

    let w = &abilities.w;
    w_settings.range = positive_or(w.range, w_settings.range);
    w_settings.travel_seconds = positive_or(w.travel_seconds, w_settings.travel_seconds);
    w_settings.radius = positive_or(w.projectile_radius, w_settings.radius);

    let e = &abilities.e;
    e_settings.width = positive_or(e.width, e_settings.width);
    e_settings.range = positive_or(e.range, e_settings.range);
    e_settings.travel_seconds = positive_or(e.travel_seconds, e_settings.travel_seconds);
}

fn apply_yuna_prediction_tuning(
    abilities: &NetworkChampionAbilities,
    q_settings: &mut YunaQSettings,
    w_settings: &mut YunaWSettings,
    e_settings: &mut YunaESettings,
) {
    let q = &abilities.q;
    q_settings.range = positive_or(q.range, q_settings.range);
    q_settings.orb_radius = positive_or(q.projectile_radius, q_settings.orb_radius);
    q_settings.aoe_radius = positive_or(q.explosion_radius, q_settings.aoe_radius);
    q_settings.field_seconds = positive_or(q.lifetime_seconds, q_settings.field_seconds);
    q_settings.travel_seconds = positive_or(q.travel_seconds, q_settings.travel_seconds);

    let w = &abilities.w;
    w_settings.radius = positive_or(w.explosion_radius, w_settings.radius);
    w_settings.field_seconds = positive_or(w.lifetime_seconds, w_settings.field_seconds);

    let e = &abilities.e;
    e_settings.range = positive_or(e.range, e_settings.range);
    e_settings.travel_seconds = positive_or(e.travel_seconds, e_settings.travel_seconds);
    e_settings.projectile_radius = positive_or(e.projectile_radius, e_settings.projectile_radius);
}

fn apply_sophia_prediction_tuning(
    abilities: &NetworkChampionAbilities,
    q_settings: &mut SophiaQSettings,
    w_settings: &mut SophiaWSettings,
    e_settings: &mut SophiaESettings,
) {
    let q = &abilities.q;
    q_settings.range = positive_or(q.range, q_settings.range);
    q_settings.target_radius = positive_or(q.target_radius, q_settings.target_radius);
    q_settings.orb_seconds = positive_or(q.lifetime_seconds, q_settings.orb_seconds);
    q_settings.damage_per_second = positive_or(q.damage_per_second, q_settings.damage_per_second);
    q_settings.orb_radius = positive_or(q.projectile_radius, q_settings.orb_radius);

    let w = &abilities.w;
    if w.missile_count > 0 {
        w_settings.minion_count = usize::from(w.missile_count);
    }
    w_settings.lifetime_seconds =
        positive_or(w.missile_lifetime_seconds, w_settings.lifetime_seconds);
    w_settings.search_radius = positive_or(w.missile_search_radius, w_settings.search_radius);
    w_settings.chase_speed = positive_or(w.missile_chase_speed, w_settings.chase_speed);
    w_settings.minion_radius = positive_or(w.missile_radius, w_settings.minion_radius);

    let e = &abilities.e;
    e_settings.buff_seconds = positive_or(e.lifetime_seconds, e_settings.buff_seconds);
    e_settings.speed_seconds = positive_or(e.speed_seconds, e_settings.speed_seconds);
    e_settings.damage_multiplier = positive_or(e.damage_multiplier, e_settings.damage_multiplier);
}

/// Description:
/// Returns a positive candidate value or a fallback value.
///
/// Params:
/// - `candidate`: Candidate numeric value read from champion content.
/// - `fallback`: Fallback value used when the candidate is not positive.
///
/// Returns:
/// - Candidate when finite and positive, otherwise fallback.
fn positive_or(candidate: f32, fallback: f32) -> f32 {
    if candidate.is_finite() && candidate > 0.0 {
        candidate
    } else {
        fallback
    }
}
