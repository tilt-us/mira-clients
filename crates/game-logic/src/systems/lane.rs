use super::{TrainingDummy, healthbar};
use bevy::ecs::query::QueryFilter;
use bevy::math::primitives::{Cuboid, Cylinder, Sphere};
use bevy::prelude::*;
use game_shared::game::{
    lane::{LaneUnitKind, TOWER_ATTACK_RANGE},
    player::{Health, Player, PlayerControlled, PlayerProfile},
    team::{Team, TeamSpec},
};
use game_shared::network::{LaneSnapshot, NetworkLaneUnit, NetworkTargetId};
use lightyear::prelude::*;
use std::collections::{HashMap, HashSet};

const LANE_POSITION_SMOOTHING: f32 = 20.0;
const LANE_ROTATION_SMOOTHING: f32 = 16.0;
const MAX_INTERPOLATED_POSITION_DELTA: f32 = 14.0;
const TOWER_ATTACK_LINE_THICKNESS: f32 = 0.075;
const TOWER_ATTACK_LINE_HEIGHT: f32 = 1.55;

const LIGHT_TEAM_COLOR: Color = Color::srgb_u8(0x2e, 0x7d, 0xf6);
const DARK_TEAM_COLOR: Color = Color::srgb_u8(0xe2, 0x3a, 0x3a);
const NEUTRAL_TEAM_COLOR: Color = Color::srgb_u8(0x82, 0x88, 0x94);

/// Tracks one client-side presentation entity for a replicated lane unit.
///
/// Fields:
/// - `id`: Stable server-provided lane-unit id.
/// - `kind`: Server-provided lane-unit visual role.
/// - `team`: Team that owns the lane unit.
/// - `is_enemy`: Whether this unit is currently hostile to the local player.
/// - `health_bar`: Standalone overhead health bar following the unit.
/// - `target_position`: Latest authoritative position used for interpolation.
/// - `target_rotation`: Latest authoritative facing rotation used for interpolation.
/// - `attack_target`: Current authoritative attack target used by tower attack lines.
#[derive(Component, Debug, Clone, Copy)]
pub(super) struct RemoteLaneUnit {
    id: u64,
    kind: LaneUnitKind,
    team: TeamSpec,
    is_enemy: bool,
    health_bar: Entity,
    target_position: Vec3,
    target_rotation: Quat,
    attack_target: Option<NetworkTargetId>,
}

/// Marks the translucent 6 metre attack radius displayed below each tower.
#[derive(Component, Debug, Clone, Copy)]
struct TowerAttackRange;

/// Marks the visible line from a tower to its current server-authoritative target.
#[derive(Component, Debug, Clone, Copy)]
pub(super) struct TowerAttackLine {
    tower_id: u64,
}

/// Stores a resolved tower-attack line transform before it is applied to an entity.
#[derive(Debug, Clone, Copy)]
struct TowerAttackLineGeometry {
    center: Vec3,
    rotation: Quat,
    length: f32,
    team: TeamSpec,
}

/// Reconciles replicated towers and minions from the latest server lane snapshot.
///
/// Params:
/// - `commands`: ECS command buffer used to spawn and remove lane presentation entities.
/// - `asset_server`: Asset server used for overhead health bar text.
/// - `receivers`: Lane snapshot receivers attached to the local client link.
/// - `local_player`: Controlled player's current team used to color ally and enemy health bars.
/// - `lane_units`: Existing lane-unit presentation entities to update or remove.
/// - `meshes`: Mesh asset collection used for lane geometry and health bars.
/// - `materials`: Material asset collection used for lane geometry and health bars.
/// - `health_bar_style`: Configured HUD accent color.
pub(super) fn sync_lane_units_from_snapshot(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut receivers: Query<&mut MessageReceiver<LaneSnapshot>, With<Client>>,
    local_player: Query<&Team, With<PlayerControlled>>,
    mut lane_units: Query<
        (
            Entity,
            &mut RemoteLaneUnit,
            &mut Transform,
            &mut Health,
            Option<&mut TrainingDummy>,
            &mut Team,
            &mut PlayerProfile,
        ),
        Without<PlayerControlled>,
    >,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    health_bar_style: Res<healthbar::OverheadHealthBarStyle>,
) {
    let mut latest_snapshot = None;
    for mut receiver in &mut receivers {
        for snapshot in receiver.receive() {
            latest_snapshot = Some(snapshot);
        }
    }

    let Some(snapshot) = latest_snapshot else {
        return;
    };
    let local_team = local_player
        .single()
        .map(|team| team.0)
        .unwrap_or(TeamSpec::Light);
    let mut retained_ids = HashSet::with_capacity(snapshot.units.len());

    for (
        entity,
        mut lane_unit,
        mut transform,
        mut health,
        mut maybe_dummy,
        mut team,
        mut profile,
    ) in &mut lane_units
    {
        let Some(snapshot_unit) = snapshot.units.iter().find(|unit| unit.id == lane_unit.id) else {
            despawn_lane_unit(&mut commands, entity, lane_unit.health_bar);
            continue;
        };

        let is_enemy = snapshot_unit.team.is_playable() && snapshot_unit.team != local_team;
        if lane_unit.kind != snapshot_unit.kind
            || lane_unit.team != snapshot_unit.team
            || lane_unit.is_enemy != is_enemy
        {
            despawn_lane_unit(&mut commands, entity, lane_unit.health_bar);
            continue;
        }

        apply_lane_unit_snapshot(
            &mut lane_unit,
            &mut transform,
            &mut health,
            maybe_dummy.as_deref_mut(),
            &mut team,
            &mut profile,
            snapshot_unit,
        );
        retained_ids.insert(snapshot_unit.id);
    }

    for snapshot_unit in &snapshot.units {
        if retained_ids.contains(&snapshot_unit.id) {
            continue;
        }

        spawn_lane_unit(
            &mut commands,
            &asset_server,
            &mut meshes,
            &mut materials,
            snapshot_unit,
            local_team,
            health_bar_style.accent_color,
        );
    }
}

/// Smoothly moves lane units toward the latest server-provided position and facing direction.
///
/// Params:
/// - `time`: Bevy time resource used to calculate frame-rate independent smoothing.
/// - `lane_units`: Replicated lane-unit transforms and latest targets.
pub(super) fn interpolate_lane_unit_positions(
    time: Res<Time>,
    mut lane_units: Query<(&RemoteLaneUnit, &mut Transform)>,
) {
    let position_smoothing = 1.0 - (-LANE_POSITION_SMOOTHING * time.delta_secs()).exp();
    let rotation_smoothing = 1.0 - (-LANE_ROTATION_SMOOTHING * time.delta_secs()).exp();

    for (lane_unit, mut transform) in &mut lane_units {
        transform.translation = transform
            .translation
            .lerp(lane_unit.target_position, position_smoothing);
        transform.rotation = transform
            .rotation
            .slerp(lane_unit.target_rotation, rotation_smoothing);
    }
}

/// Shows or updates a line from each active tower to its current attack target.
///
/// Params:
/// - `commands`: ECS command buffer used to create and remove attack line entities.
/// - `tower_query`: Replicated lane-unit attack targets and source transforms.
/// - `target_query`: Player and lane-unit transforms used to resolve attack targets.
/// - `line_query`: Existing tower attack lines to update or remove.
/// - `meshes`: Mesh asset collection used by newly spawned attack lines.
/// - `materials`: Material asset collection used by newly spawned attack lines.
pub(super) fn update_tower_attack_lines(
    mut commands: Commands,
    tower_query: Query<(&RemoteLaneUnit, &Transform), Without<TowerAttackLine>>,
    target_query: Query<
        (&Transform, Option<&Player>, Option<&NetworkTargetId>),
        Without<TowerAttackLine>,
    >,
    mut line_query: Query<(Entity, &TowerAttackLine, &mut Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut wanted_lines = HashMap::new();
    for (tower, tower_transform) in &tower_query {
        if tower.kind != LaneUnitKind::Tower {
            continue;
        }
        let Some(target) = tower.attack_target else {
            continue;
        };
        let Some(target_position) = target_position(target, &target_query) else {
            continue;
        };
        let Some(geometry) =
            tower_attack_line_geometry(tower_transform.translation, target_position, tower.team)
        else {
            continue;
        };
        wanted_lines.insert(tower.id, geometry);
    }

    for (entity, line, mut transform) in &mut line_query {
        let Some(geometry) = wanted_lines.remove(&line.tower_id) else {
            commands.entity(entity).despawn();
            continue;
        };
        apply_tower_attack_line_transform(&mut transform, geometry);
    }

    for (tower_id, geometry) in wanted_lines {
        let mesh = meshes.add(Cuboid::new(
            TOWER_ATTACK_LINE_THICKNESS,
            TOWER_ATTACK_LINE_THICKNESS,
            1.0,
        ));
        let color = team_color(geometry.team);
        let material = materials.add(StandardMaterial {
            base_color: color.with_alpha(0.9),
            emissive: color.with_alpha(0.7).into(),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        let mut transform = Transform::default();
        apply_tower_attack_line_transform(&mut transform, geometry);
        commands.spawn((
            Name::new(format!("TowerAttackLine{tower_id}")),
            TowerAttackLine { tower_id },
            Mesh3d(mesh),
            MeshMaterial3d(material),
            transform,
        ));
    }
}

/// Spawns one visual stand-in for a tower or minion from a server lane snapshot entry.
#[allow(clippy::too_many_arguments)]
fn spawn_lane_unit(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    snapshot_unit: &NetworkLaneUnit,
    local_team: TeamSpec,
    accent_color: Color,
) {
    let is_enemy = snapshot_unit.team.is_playable() && snapshot_unit.team != local_team;
    let layout = lane_unit_layout(snapshot_unit.kind);
    let position = visual_position(snapshot_unit.position.into(), snapshot_unit.kind);
    let material = materials.add(lane_unit_material(snapshot_unit.team, snapshot_unit.kind));
    let entity = commands
        .spawn((
            Name::new(format!(
                "{}{}{}",
                team_name(snapshot_unit.team),
                lane_unit_name(snapshot_unit.kind),
                snapshot_unit.id
            )),
            NetworkTargetId::LaneUnit(snapshot_unit.id),
            Team(snapshot_unit.team),
            Health {
                current: snapshot_unit.health.max(0.0).ceil() as u32,
                max: snapshot_unit.max_health.max(1.0).ceil() as u32,
            },
            PlayerProfile {
                display_name: format!(
                    "{} {}",
                    team_name(snapshot_unit.team),
                    lane_unit_name(snapshot_unit.kind)
                ),
            },
            Mesh3d(meshes.add(lane_unit_mesh(snapshot_unit.kind))),
            MeshMaterial3d(material),
            Transform::from_translation(position)
                .with_rotation(Quat::from_rotation_y(snapshot_unit.yaw)),
        ))
        .id();

    if is_enemy {
        commands.entity(entity).insert(TrainingDummy::remote_player(
            snapshot_unit.health,
            snapshot_unit.max_health,
            snapshot_unit.hit_radius,
        ));
    }

    let health_bar = healthbar::spawn_remote_lane_unit_health_bar(
        commands,
        asset_server,
        meshes,
        materials,
        entity,
        snapshot_unit.max_health,
        is_enemy,
        layout.health_bar_offset,
        accent_color,
    );
    commands.entity(entity).insert(RemoteLaneUnit {
        id: snapshot_unit.id,
        kind: snapshot_unit.kind,
        team: snapshot_unit.team,
        is_enemy,
        health_bar,
        target_position: position,
        target_rotation: Quat::from_rotation_y(snapshot_unit.yaw),
        attack_target: snapshot_unit.attack_target,
    });

    if snapshot_unit.kind == LaneUnitKind::Tower {
        let range_material = materials.add(tower_range_material(snapshot_unit.team));
        let range_entity = commands
            .spawn((
                Name::new(format!("TowerAttackRange{}", snapshot_unit.id)),
                TowerAttackRange,
                Mesh3d(meshes.add(Cylinder::new(TOWER_ATTACK_RANGE, 0.025))),
                MeshMaterial3d(range_material),
                Transform::from_xyz(0.0, -layout.center_height + 0.025, 0.0),
            ))
            .id();
        commands.entity(entity).add_child(range_entity);
    }
}

/// Applies a new server snapshot to an existing lane-unit presentation entity.
#[allow(clippy::too_many_arguments)]
fn apply_lane_unit_snapshot(
    lane_unit: &mut RemoteLaneUnit,
    transform: &mut Transform,
    health: &mut Health,
    dummy: Option<&mut TrainingDummy>,
    team: &mut Team,
    profile: &mut PlayerProfile,
    snapshot_unit: &NetworkLaneUnit,
) {
    lane_unit.target_position = visual_position(snapshot_unit.position.into(), snapshot_unit.kind);
    lane_unit.target_rotation = Quat::from_rotation_y(snapshot_unit.yaw);
    lane_unit.attack_target = snapshot_unit.attack_target;
    if let Some(dummy) = dummy {
        dummy.set_server_health(snapshot_unit.health, snapshot_unit.max_health);
    }
    health.current = snapshot_unit.health.max(0.0).ceil() as u32;
    health.max = snapshot_unit.max_health.max(1.0).ceil() as u32;
    team.0 = snapshot_unit.team;
    profile.display_name = format!(
        "{} {}",
        team_name(snapshot_unit.team),
        lane_unit_name(snapshot_unit.kind)
    );

    if transform
        .translation
        .distance_squared(lane_unit.target_position)
        > MAX_INTERPOLATED_POSITION_DELTA.powi(2)
    {
        transform.translation = lane_unit.target_position;
        transform.rotation = lane_unit.target_rotation;
    }
}

/// Removes a lane unit, its standalone health bar, and any child presentation entities.
fn despawn_lane_unit(commands: &mut Commands, entity: Entity, health_bar: Entity) {
    if health_bar != Entity::PLACEHOLDER {
        commands.entity(health_bar).despawn();
    }
    commands.entity(entity).despawn_children();
    commands.entity(entity).despawn();
}

/// Resolves the current entity position for one server-authoritative target identifier.
fn target_position<F>(
    target: NetworkTargetId,
    target_query: &Query<(&Transform, Option<&Player>, Option<&NetworkTargetId>), F>,
) -> Option<Vec3>
where
    F: QueryFilter,
{
    target_query
        .iter()
        .find_map(|(transform, player, lane_target)| {
            let matches_target = match target {
                NetworkTargetId::Player(player_id) => {
                    player.is_some_and(|player| player.id.0 == player_id)
                }
                NetworkTargetId::LaneUnit(lane_unit_id) => lane_target.is_some_and(|lane_target| {
                    *lane_target == NetworkTargetId::LaneUnit(lane_unit_id)
                }),
            };
            matches_target.then_some(transform.translation)
        })
}

/// Computes the position, orientation, and length used to render one tower attack line.
fn tower_attack_line_geometry(
    tower_position: Vec3,
    target_position: Vec3,
    team: TeamSpec,
) -> Option<TowerAttackLineGeometry> {
    let start = tower_position + Vec3::Y * (TOWER_ATTACK_LINE_HEIGHT - tower_position.y);
    let end = target_position + Vec3::Y * 0.8;
    let delta = end - start;
    let length = delta.length();
    if length <= f32::EPSILON {
        return None;
    }

    Some(TowerAttackLineGeometry {
        center: start + delta * 0.5,
        rotation: Quat::from_rotation_arc(Vec3::Z, delta / length),
        length,
        team,
    })
}

/// Applies a resolved tower attack line geometry to a Bevy transform.
fn apply_tower_attack_line_transform(transform: &mut Transform, geometry: TowerAttackLineGeometry) {
    transform.translation = geometry.center;
    transform.rotation = geometry.rotation;
    transform.scale = Vec3::new(1.0, 1.0, geometry.length);
}

/// Returns the mesh used by one lane-unit visual role.
fn lane_unit_mesh(kind: LaneUnitKind) -> Mesh {
    match kind {
        LaneUnitKind::MeleeBox => Mesh::from(Cuboid::new(0.75, 0.75, 0.75)),
        LaneUnitKind::LargeRangedBox => Mesh::from(Cuboid::new(1.25, 1.25, 1.25)),
        LaneUnitKind::RangedOrb => Mesh::from(Sphere::new(0.45)),
        LaneUnitKind::Tower => Mesh::from(Cylinder::new(1.25, 2.6)),
    }
}

/// Returns the fixed vertical layout used by one lane-unit role.
fn lane_unit_layout(kind: LaneUnitKind) -> LaneUnitLayout {
    match kind {
        LaneUnitKind::MeleeBox => LaneUnitLayout {
            center_height: 0.375,
            health_bar_offset: 1.25,
        },
        LaneUnitKind::LargeRangedBox => LaneUnitLayout {
            center_height: 0.625,
            health_bar_offset: 1.55,
        },
        LaneUnitKind::RangedOrb => LaneUnitLayout {
            center_height: 0.45,
            health_bar_offset: 1.3,
        },
        LaneUnitKind::Tower => LaneUnitLayout {
            center_height: 1.3,
            health_bar_offset: 2.05,
        },
    }
}

/// Holds the vertical offsets used for a lane-unit presentation entity.
struct LaneUnitLayout {
    center_height: f32,
    health_bar_offset: f32,
}

/// Converts a ground-level network position into the center position for a visible lane mesh.
fn visual_position(position: Vec3, kind: LaneUnitKind) -> Vec3 {
    position + Vec3::Y * lane_unit_layout(kind).center_height
}

/// Builds a team-colored material for a minion or tower mesh.
fn lane_unit_material(team: TeamSpec, kind: LaneUnitKind) -> StandardMaterial {
    let color = team_color(team);
    let roughness = if kind == LaneUnitKind::Tower {
        0.38
    } else {
        0.62
    };
    StandardMaterial {
        base_color: color,
        emissive: color
            .with_alpha(if kind == LaneUnitKind::Tower {
                0.32
            } else {
                0.18
            })
            .into(),
        metallic: if kind == LaneUnitKind::Tower {
            0.22
        } else {
            0.0
        },
        perceptual_roughness: roughness,
        ..default()
    }
}

/// Builds the subtle translucent circle that communicates the tower attack radius.
fn tower_range_material(team: TeamSpec) -> StandardMaterial {
    let color = team_color(team);
    StandardMaterial {
        base_color: color.with_alpha(0.13),
        emissive: color.with_alpha(0.12).into(),
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        unlit: true,
        ..default()
    }
}

/// Returns the display color used by a team-owned lane object.
fn team_color(team: TeamSpec) -> Color {
    match team {
        TeamSpec::Light => LIGHT_TEAM_COLOR,
        TeamSpec::Dark => DARK_TEAM_COLOR,
        TeamSpec::Neutral => NEUTRAL_TEAM_COLOR,
    }
}

/// Returns the concise visual role name used by lane unit entities and health bars.
fn lane_unit_name(kind: LaneUnitKind) -> &'static str {
    match kind {
        LaneUnitKind::MeleeBox => "Melee Minion",
        LaneUnitKind::LargeRangedBox => "Ranged Minion",
        LaneUnitKind::RangedOrb => "Orb Minion",
        LaneUnitKind::Tower => "Tower",
    }
}

/// Returns the display name used for one team in lane unit labels.
fn team_name(team: TeamSpec) -> &'static str {
    match team {
        TeamSpec::Light => "Light",
        TeamSpec::Dark => "Dark",
        TeamSpec::Neutral => "Neutral",
    }
}
