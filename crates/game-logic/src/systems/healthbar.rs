use super::TrainingDummy;
use bevy::asset::RenderAssetUsages;
use bevy::math::primitives::Cuboid;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy_fontmesh::{JustifyText, TextAnchor, TextMesh, TextMeshStyle};
use game_shared::game::{
    camera::TopDownCamera,
    player::{Health, Mana, Player, PlayerProfile},
};
use std::collections::HashMap;

const BAR_WIDTH: f32 = 1.75;
const BAR_HEIGHT: f32 = 0.24;
const HEALTH_FILL_HEIGHT: f32 = 0.105;
const MANA_FILL_HEIGHT: f32 = 0.045;
const BAR_DEPTH: f32 = 0.035;
const HEALTH_FILL_Y: f32 = 0.048;
const MANA_FILL_Y: f32 = -0.052;
const BAR_BORDER_THICKNESS: f32 = 0.012;
const NAME_TEXT_Y: f32 = BAR_HEIGHT * 0.5 + 0.18;
const NAME_TEXT_SCALE: f32 = 0.16;
const LEVEL_TEXT_SCALE: f32 = 0.145;
const NAME_MAX_CHARS: usize = 14;
const NAME_SHADOW_OFFSET: Vec3 = Vec3::new(0.014, -0.014, -0.002);
const LEVEL_SHADOW_OFFSET: Vec3 = Vec3::new(0.01, -0.01, -0.002);
const LEVEL_BADGE_WIDTH_TOP: f32 = 0.48;
const LEVEL_BADGE_WIDTH_BOTTOM: f32 = 0.30;
const LEVEL_BADGE_HEIGHT: f32 = 0.22;
const LEVEL_BADGE_LEFT: f32 = -BAR_WIDTH * 0.5;
const LEVEL_BADGE_TOP: f32 = -BAR_HEIGHT * 0.5;
const LEVEL_BADGE_X: f32 = LEVEL_BADGE_LEFT + LEVEL_BADGE_WIDTH_TOP * 0.5;
const LEVEL_BADGE_Y: f32 = LEVEL_BADGE_TOP - LEVEL_BADGE_HEIGHT * 0.5;
const LEVEL_BADGE_BORDER_WIDTH_TOP: f32 = LEVEL_BADGE_WIDTH_TOP + BAR_BORDER_THICKNESS * 2.0;
const LEVEL_BADGE_BORDER_HEIGHT: f32 = LEVEL_BADGE_HEIGHT + BAR_BORDER_THICKNESS * 2.0;
const LEVEL_BADGE_BORDER_X: f32 = LEVEL_BADGE_LEFT + LEVEL_BADGE_BORDER_WIDTH_TOP * 0.5;
const LEVEL_BADGE_BORDER_Y: f32 = LEVEL_BADGE_TOP - LEVEL_BADGE_BORDER_HEIGHT * 0.5;
const LEVEL_TEXT_X: f32 = LEVEL_BADGE_X - (LEVEL_BADGE_WIDTH_TOP - LEVEL_BADGE_WIDTH_BOTTOM) * 0.25;
const MAX_HEALTH_MARKER_THRESHOLD: u32 = 3000;
const OVERHEAD_FONT_PATH: &str = "fonts/Roboto-Bold.ttf";
const LIRA_BAR_OFFSET: f32 = 2.65;
const ALLY_HEALTH_COLOR: Color = Color::srgb_u8(0x23, 0xad, 0xd9);
const ENEMY_HEALTH_COLOR: Color = Color::srgb_u8(0xd9, 0x23, 0x2f);
const LOCAL_HEALTH_COLOR: Color = Color::srgb_u8(0x4a, 0xd1, 0x1b);
const MANA_COLOR: Color = Color::srgb_u8(0x1b, 0x5a, 0xd1);
const DEFAULT_ACCENT_COLOR: Color = Color::srgb_u8(0xf2, 0xc4, 0x5b);
const HEALTH_BAR_PANEL_COLOR: Color = Color::srgb_u8(0x10, 0x12, 0x16);

#[derive(Resource, Debug, Clone, Copy)]
/// Description:
/// Stores visual options used by overhead health bars.
pub struct OverheadHealthBarStyle {
    /// Accent color used for the bottom border under the health bar.
    pub accent_color: Color,
}

impl Default for OverheadHealthBarStyle {
    fn default() -> Self {
        Self {
            accent_color: DEFAULT_ACCENT_COLOR,
        }
    }
}

#[derive(Resource, Debug, Clone, Default)]
/// Description:
/// Stores public player names received before the gameplay snapshot starts.
pub struct OverheadPlayerProfiles {
    display_names: HashMap<u64, String>,
}

impl OverheadPlayerProfiles {
    /// Description:
    /// Stores a public display name for one network player id.
    pub fn set_display_name(&mut self, player_id: u64, display_name: impl Into<String>) {
        let display_name = display_name.into();
        if !display_name.trim().is_empty() {
            self.display_names.insert(player_id, display_name);
        }
    }

    /// Description:
    /// Returns the public display name for one network player id.
    pub fn display_name(&self, player_id: u64) -> Option<&str> {
        self.display_names.get(&player_id).map(String::as_str)
    }
}

#[derive(Component, Debug, Clone, Copy)]
/// Description:
/// Stores the world-space tracking data for an overhead health bar.
///
/// Fields:
/// - `target`: Entity that the health bar follows.
/// - `y_offset`: Vertical world-space offset above the target.
pub(in crate::systems) struct HealthBar {
    target: Entity,
    y_offset: f32,
}

#[derive(Component, Debug, Clone, Copy)]
/// Description:
/// Stores fill-scaling data for a health bar foreground segment.
///
/// Fields:
/// - `target`: Entity whose health controls the fill width.
/// - `max_health`: Fallback maximum health value for sources without a max-health field.
/// - `source`: Health source type used to read the correct component.
pub(in crate::systems) struct HealthBarFill {
    target: Entity,
    max_health: f32,
    source: HealthBarSource,
}

#[derive(Component, Debug, Clone, Copy)]
/// Description:
/// Stores fill-scaling data for a mana bar foreground segment.
pub(in crate::systems) struct HealthBarManaFill {
    target: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
/// Description:
/// Stores one fixed 100-HP marker on the overhead health bar.
pub(in crate::systems) struct HealthBarHpMarker {
    target: Entity,
    fallback_max_health: f32,
    source: HealthBarSource,
    threshold: f32,
}

#[derive(Component, Debug, Clone)]
/// Description:
/// Tracks the player profile shown above the overhead health bar.
pub(in crate::systems) struct HealthBarName {
    target: Entity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Description:
/// Identifies which component type provides health for a health bar fill.
///
/// Fields:
/// - `Player`: Reads health from the shared `Health` component.
/// - `TrainingDummy`: Reads health from an enemy training dummy or remote player stand-in.
pub(in crate::systems) enum HealthBarSource {
    Player,
    TrainingDummy,
}

/// Description:
/// Spawns an overhead health bar for a remote enemy player.
///
/// Params:
/// - `commands`: ECS command buffer used to spawn health bar entities.
/// - `meshes`: Mesh assets used by health bar geometry.
/// - `materials`: Material assets used by health bar visuals.
/// - `target`: Remote player entity followed by the health bar.
/// - `max_health`: Maximum health value used to scale the fill.
///
/// Return:
/// - Spawned health bar entity.
pub(in crate::systems) fn spawn_remote_enemy_player_health_bar(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    target: Entity,
    max_health: f32,
    accent_color: Color,
) -> Entity {
    spawn_health_bar(
        commands,
        asset_server,
        meshes,
        materials,
        target,
        LIRA_BAR_OFFSET,
        max_health,
        HealthBarSource::TrainingDummy,
        ENEMY_HEALTH_COLOR,
        accent_color,
    )
}

/// Description:
/// Spawns an overhead health bar for a remote allied player.
pub(in crate::systems) fn spawn_remote_ally_player_health_bar(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    target: Entity,
    max_health: f32,
    accent_color: Color,
) -> Entity {
    spawn_health_bar(
        commands,
        asset_server,
        meshes,
        materials,
        target,
        LIRA_BAR_OFFSET,
        max_health,
        HealthBarSource::Player,
        ALLY_HEALTH_COLOR,
        accent_color,
    )
}

/// Description:
/// Spawns an overhead health bar for the local player.
///
/// Params:
/// - `commands`: ECS command buffer used to spawn health bar entities.
/// - `meshes`: Mesh assets used by health bar geometry.
/// - `materials`: Material assets used by health bar visuals.
/// - `target`: Player entity followed by the health bar.
///
/// Return:
/// - Spawned health bar entity.
pub(in crate::systems) fn spawn_player_health_bar(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    target: Entity,
    accent_color: Color,
) -> Entity {
    spawn_health_bar(
        commands,
        asset_server,
        meshes,
        materials,
        target,
        LIRA_BAR_OFFSET,
        100.0,
        HealthBarSource::Player,
        LOCAL_HEALTH_COLOR,
        accent_color,
    )
}

/// Description:
/// Updates overhead health bar positions and rotates them to face the top-down camera.
///
/// Params:
/// - `camera_query`: Top-down camera transform used as the billboard orientation source.
/// - `target_query`: Target transforms followed by health bars.
/// - `bar_query`: Health bar tracking components and transforms to update.
pub(in crate::systems) fn update_health_bar_positions(
    camera_query: Query<&GlobalTransform, With<TopDownCamera>>,
    target_query: Query<&GlobalTransform>,
    mut bar_query: Query<(&HealthBar, &mut Transform)>,
) {
    let camera_rotation = camera_query
        .single()
        .ok()
        .map(GlobalTransform::rotation)
        .unwrap_or(Quat::IDENTITY);

    for (bar, mut transform) in &mut bar_query {
        let Ok(target_transform) = target_query.get(bar.target) else {
            continue;
        };

        transform.translation = target_transform.translation() + Vec3::Y * bar.y_offset;
        transform.rotation = camera_rotation;
    }
}

/// Description:
/// Updates health bar fill width and visibility from the tracked target health.
///
/// Params:
/// - `player_query`: Player health components.
/// - `training_dummy_query`: Enemy training dummy health components.
/// - `fill_query`: Health bar fills that should be scaled or hidden.
pub(in crate::systems) fn update_health_bar_fills(
    player_query: Query<&Health>,
    mana_query: Query<&Mana>,
    profile_query: Query<&PlayerProfile>,
    player_id_query: Query<&Player>,
    player_profiles: Res<OverheadPlayerProfiles>,
    training_dummy_query: Query<&TrainingDummy>,
    mut fill_queries: ParamSet<(
        Query<(&HealthBarFill, &mut Transform, &mut Visibility)>,
        Query<(&HealthBarManaFill, &mut Transform, &mut Visibility)>,
        Query<(&HealthBarHpMarker, &mut Transform, &mut Visibility)>,
    )>,
    mut name_query: Query<(&HealthBarName, &mut TextMesh)>,
) {
    for (fill, mut transform, mut visibility) in &mut fill_queries.p0() {
        let Some((health, max_health)) = health_values(
            fill.target,
            fill.source,
            fill.max_health,
            &player_query,
            &training_dummy_query,
        ) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        let ratio = (health / max_health).clamp(0.0, 1.0);
        transform.scale.x = BAR_WIDTH * ratio;
        transform.translation.x = -BAR_WIDTH * (1.0 - ratio) * 0.5;
        *visibility = if ratio > 0.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    for (fill, mut transform, mut visibility) in &mut fill_queries.p1() {
        let Some((mana, max_mana)) = mana_query
            .get(fill.target)
            .ok()
            .map(|mana| (mana.current as f32, mana.max as f32))
        else {
            *visibility = Visibility::Hidden;
            continue;
        };

        let ratio = if max_mana > 0.0 {
            (mana / max_mana).clamp(0.0, 1.0)
        } else {
            0.0
        };
        transform.scale.x = BAR_WIDTH * ratio;
        transform.translation.x = -BAR_WIDTH * (1.0 - ratio) * 0.5;
        *visibility = if ratio > 0.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    for (marker, mut transform, mut visibility) in &mut fill_queries.p2() {
        let Some((_, max_health)) = health_values(
            marker.target,
            marker.source,
            marker.fallback_max_health,
            &player_query,
            &training_dummy_query,
        ) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        if marker.threshold >= max_health {
            *visibility = Visibility::Hidden;
            continue;
        }

        let ratio = (marker.threshold / max_health).clamp(0.0, 1.0);
        transform.translation.x = -BAR_WIDTH * 0.5 + BAR_WIDTH * ratio;
        *visibility = Visibility::Visible;
    }
    for (name, mut text_mesh) in &mut name_query {
        let display_name = profile_query
            .get(name.target)
            .ok()
            .map(|profile| profile.display_name.trim())
            .filter(|display_name| !display_name.is_empty())
            .or_else(|| {
                player_id_query
                    .get(name.target)
                    .ok()
                    .and_then(|player| player_profiles.display_name(player.id.0))
            })
            .unwrap_or("Player");

        let display_name = compact_display_name(display_name);
        if text_mesh.text != display_name {
            text_mesh.text = display_name;
        }
    }
}

/// Description:
/// Spawns the shared child geometry for an overhead health bar.
///
/// Params:
/// - `commands`: ECS command buffer used to spawn the health bar hierarchy.
/// - `meshes`: Mesh assets used by background, fill, and stripe geometry.
/// - `materials`: Material assets used by background, fill, and stripe visuals.
/// - `target`: Entity followed by the health bar.
/// - `y_offset`: Vertical world-space offset above the target.
/// - `max_health`: Maximum health value used by the fill component.
/// - `source`: Health source type read by the fill update system.
/// - `health_color`: Color used by the health fill and level badge.
///
/// Return:
/// - Spawned health bar entity.
fn spawn_health_bar(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    target: Entity,
    y_offset: f32,
    max_health: f32,
    source: HealthBarSource,
    health_color: Color,
    _accent_color: Color,
) -> Entity {
    let background_mesh = meshes.add(Cuboid::new(BAR_WIDTH, BAR_HEIGHT, BAR_DEPTH));
    let health_fill_mesh = meshes.add(Cuboid::new(1.0, HEALTH_FILL_HEIGHT, BAR_DEPTH * 1.25));
    let mana_fill_mesh = meshes.add(Cuboid::new(1.0, MANA_FILL_HEIGHT, BAR_DEPTH * 1.25));
    let horizontal_border_mesh = meshes.add(Cuboid::new(
        BAR_WIDTH,
        BAR_BORDER_THICKNESS,
        BAR_DEPTH * 1.45,
    ));
    let vertical_border_mesh = meshes.add(Cuboid::new(
        BAR_BORDER_THICKNESS,
        BAR_HEIGHT,
        BAR_DEPTH * 1.45,
    ));
    let marker_mesh = meshes.add(Cuboid::new(
        0.012,
        HEALTH_FILL_HEIGHT * 0.5,
        BAR_DEPTH * 1.45,
    ));
    let level_badge_mesh = meshes.add(level_badge_mesh());
    let level_badge_border_mesh = meshes.add(level_badge_border_mesh());
    let overhead_font = asset_server.load(OVERHEAD_FONT_PATH);

    let background_material = materials.add(StandardMaterial {
        base_color: HEALTH_BAR_PANEL_COLOR,
        emissive: Color::BLACK.into(),
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        unlit: true,
        ..default()
    });
    let border_material = materials.add(StandardMaterial {
        base_color: HEALTH_BAR_PANEL_COLOR,
        emissive: HEALTH_BAR_PANEL_COLOR.with_alpha(0.26).into(),
        cull_mode: None,
        unlit: true,
        ..default()
    });
    let health_fill_material = materials.add(StandardMaterial {
        base_color: health_color,
        emissive: health_color.with_alpha(0.24).into(),
        unlit: true,
        ..default()
    });
    let mana_fill_material = materials.add(StandardMaterial {
        base_color: MANA_COLOR,
        emissive: MANA_COLOR.with_alpha(0.24).into(),
        unlit: true,
        ..default()
    });
    let marker_material = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        unlit: true,
        ..default()
    });
    let name_text_material = materials.add(text_mesh_material(Color::WHITE));
    let name_shadow_material = materials.add(text_mesh_material(Color::srgba(0.0, 0.0, 0.0, 0.95)));
    let level_text_material = materials.add(text_mesh_material(Color::WHITE));
    let level_shadow_material = materials.add(text_mesh_material(Color::srgba(0.0, 0.0, 0.0, 0.8)));
    let bar_entity = commands
        .spawn((
            Name::new("HealthBar"),
            HealthBar { target, y_offset },
            Transform::default(),
            Visibility::Visible,
        ))
        .with_children(|bar| {
            bar.spawn((
                Name::new("HealthBarPlayerNameShadow"),
                HealthBarName { target },
                TextMesh {
                    text: "Player".to_string(),
                    font: overhead_font.clone(),
                    style: overhead_text_style(),
                },
                MeshMaterial3d(name_shadow_material),
                Transform::from_translation(
                    Vec3::new(0.0, NAME_TEXT_Y, BAR_DEPTH * 2.18) + NAME_SHADOW_OFFSET,
                )
                .with_scale(Vec3::splat(NAME_TEXT_SCALE)),
            ));
            bar.spawn((
                Name::new("HealthBarPlayerName"),
                HealthBarName { target },
                TextMesh {
                    text: "Player".to_string(),
                    font: overhead_font.clone(),
                    style: overhead_text_style(),
                },
                MeshMaterial3d(name_text_material),
                Transform::from_xyz(0.0, NAME_TEXT_Y, BAR_DEPTH * 2.2)
                    .with_scale(Vec3::splat(NAME_TEXT_SCALE)),
            ));
            bar.spawn((
                Name::new("HealthBarBackground"),
                Mesh3d(background_mesh),
                MeshMaterial3d(background_material.clone()),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ));
            bar.spawn((
                Name::new("HealthBarFill"),
                HealthBarFill {
                    target,
                    max_health,
                    source,
                },
                Mesh3d(health_fill_mesh),
                MeshMaterial3d(health_fill_material),
                Transform::from_xyz(0.0, HEALTH_FILL_Y, BAR_DEPTH * 0.8)
                    .with_scale(Vec3::new(BAR_WIDTH, 1.0, 1.0)),
                Visibility::Visible,
            ));
            bar.spawn((
                Name::new("HealthBarManaFill"),
                HealthBarManaFill { target },
                Mesh3d(mana_fill_mesh),
                MeshMaterial3d(mana_fill_material),
                Transform::from_xyz(0.0, MANA_FILL_Y, BAR_DEPTH * 0.82)
                    .with_scale(Vec3::new(BAR_WIDTH, 1.0, 1.0)),
                Visibility::Visible,
            ));
            bar.spawn((
                Name::new("HealthBarTopBorder"),
                Mesh3d(horizontal_border_mesh.clone()),
                MeshMaterial3d(border_material.clone()),
                Transform::from_xyz(
                    0.0,
                    BAR_HEIGHT * 0.5 - BAR_BORDER_THICKNESS * 0.5,
                    BAR_DEPTH * 1.55,
                ),
            ));
            bar.spawn((
                Name::new("HealthBarBottomBorder"),
                Mesh3d(horizontal_border_mesh),
                MeshMaterial3d(border_material.clone()),
                Transform::from_xyz(
                    0.0,
                    -BAR_HEIGHT * 0.5 + BAR_BORDER_THICKNESS * 0.5,
                    BAR_DEPTH * 1.55,
                ),
            ));
            bar.spawn((
                Name::new("HealthBarLeftBorder"),
                Mesh3d(vertical_border_mesh.clone()),
                MeshMaterial3d(border_material.clone()),
                Transform::from_xyz(
                    -BAR_WIDTH * 0.5 + BAR_BORDER_THICKNESS * 0.5,
                    0.0,
                    BAR_DEPTH * 1.55,
                ),
            ));
            bar.spawn((
                Name::new("HealthBarRightBorder"),
                Mesh3d(vertical_border_mesh),
                MeshMaterial3d(border_material.clone()),
                Transform::from_xyz(
                    BAR_WIDTH * 0.5 - BAR_BORDER_THICKNESS * 0.5,
                    0.0,
                    BAR_DEPTH * 1.55,
                ),
            ));
            spawn_health_markers(
                bar,
                target,
                max_health,
                source,
                marker_mesh,
                marker_material,
            );
            bar.spawn((
                Name::new("HealthBarLevelBadge"),
                Mesh3d(level_badge_mesh),
                MeshMaterial3d(background_material),
                Transform::from_xyz(LEVEL_BADGE_X, LEVEL_BADGE_Y, BAR_DEPTH * 1.95),
            ));
            bar.spawn((
                Name::new("HealthBarLevelBadgeBorder"),
                Mesh3d(level_badge_border_mesh),
                MeshMaterial3d(border_material),
                Transform::from_xyz(LEVEL_BADGE_BORDER_X, LEVEL_BADGE_BORDER_Y, BAR_DEPTH * 1.3),
            ));
            bar.spawn((
                Name::new("HealthBarLevelDigitShadow"),
                TextMesh {
                    text: "1".to_string(),
                    font: overhead_font.clone(),
                    style: overhead_text_style(),
                },
                MeshMaterial3d(level_shadow_material),
                Transform::from_translation(
                    Vec3::new(LEVEL_TEXT_X, LEVEL_BADGE_Y - 0.005, BAR_DEPTH * 2.38)
                        + LEVEL_SHADOW_OFFSET,
                )
                .with_scale(Vec3::splat(LEVEL_TEXT_SCALE)),
            ));
            bar.spawn((
                Name::new("HealthBarLevelDigit"),
                TextMesh {
                    text: "1".to_string(),
                    font: overhead_font.clone(),
                    style: overhead_text_style(),
                },
                MeshMaterial3d(level_text_material),
                Transform::from_xyz(LEVEL_TEXT_X, LEVEL_BADGE_Y - 0.005, BAR_DEPTH * 2.4)
                    .with_scale(Vec3::splat(LEVEL_TEXT_SCALE)),
            ));
        })
        .id();

    bar_entity
}

fn text_mesh_material(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        emissive: color.with_alpha(0.5).into(),
        double_sided: true,
        cull_mode: None,
        unlit: true,
        ..default()
    }
}

fn overhead_text_style() -> TextMeshStyle {
    TextMeshStyle {
        depth: 0.01,
        subdivision: 14,
        anchor: TextAnchor::Center,
        justify: JustifyText::Center,
    }
}

fn spawn_health_markers(
    bar: &mut ChildSpawnerCommands,
    target: Entity,
    max_health: f32,
    source: HealthBarSource,
    marker_mesh: Handle<Mesh>,
    marker_material: Handle<StandardMaterial>,
) {
    let marker_y = HEALTH_FILL_Y + HEALTH_FILL_HEIGHT * 0.25;
    let marker_count = MAX_HEALTH_MARKER_THRESHOLD / 100;
    for marker_index in 1..=marker_count {
        let threshold = marker_index as f32 * 100.0;

        bar.spawn((
            Name::new("HealthBarHpMarker"),
            HealthBarHpMarker {
                target,
                fallback_max_health: max_health,
                source,
                threshold,
            },
            Mesh3d(marker_mesh.clone()),
            MeshMaterial3d(marker_material.clone()),
            Transform::from_xyz(0.0, marker_y, BAR_DEPTH * 1.7),
            Visibility::Hidden,
        ));
    }
}

fn health_values(
    target: Entity,
    source: HealthBarSource,
    fallback_max_health: f32,
    player_query: &Query<&Health>,
    training_dummy_query: &Query<&TrainingDummy>,
) -> Option<(f32, f32)> {
    match source {
        HealthBarSource::Player => player_query
            .get(target)
            .ok()
            .map(|health| (health.current as f32, health.max as f32)),
        HealthBarSource::TrainingDummy => training_dummy_query
            .get(target)
            .ok()
            .map(|dummy| (dummy.health, fallback_max_health)),
    }
}

fn compact_display_name(display_name: &str) -> String {
    let mut compact = display_name
        .split_whitespace()
        .next()
        .unwrap_or("Player")
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
        .take(NAME_MAX_CHARS)
        .collect::<String>();

    if compact.is_empty() {
        compact = "Player".to_string();
    }

    compact
}

fn level_badge_mesh() -> Mesh {
    trapezoid_mesh(
        LEVEL_BADGE_WIDTH_TOP,
        LEVEL_BADGE_WIDTH_BOTTOM,
        LEVEL_BADGE_HEIGHT,
    )
}

fn level_badge_border_mesh() -> Mesh {
    trapezoid_mesh(
        LEVEL_BADGE_BORDER_WIDTH_TOP,
        LEVEL_BADGE_WIDTH_BOTTOM + BAR_BORDER_THICKNESS * 2.0,
        LEVEL_BADGE_BORDER_HEIGHT,
    )
}

fn trapezoid_mesh(top_width: f32, bottom_width: f32, height: f32) -> Mesh {
    let left = -top_width * 0.5;
    let right = top_width * 0.5;
    let bottom_right = left + bottom_width;
    let half_height = height * 0.5;

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [left, half_height, 0.0],
            [right, half_height, 0.0],
            [bottom_right, -half_height, 0.0],
            [left, -half_height, 0.0],
        ],
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; 4])
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
    )
    .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]))
}
