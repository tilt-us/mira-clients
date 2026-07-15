use super::{TrainingDummy, TrainingDummyHealthChangeKind};
use bevy::prelude::*;
use bevy_fontmesh::{JustifyText, TextAnchor, TextMesh, TextMeshStyle};
use game_shared::game::camera::TopDownCamera;

const DAMAGE_NUMBER_FONT_PATH: &str = "fonts/Roboto-Bold.ttf";
const DAMAGE_NUMBER_HOLD_SECONDS: f32 = 1.0;
const DAMAGE_NUMBER_FALL_SECONDS: f32 = 0.45;
const DAMAGE_NUMBER_Y_OFFSET: f32 = 2.15;
const DAMAGE_NUMBER_FALL_DISTANCE: f32 = 0.85;
const DAMAGE_NUMBER_SCALE: f32 = 0.22;
const AUTO_ATTACK_DAMAGE_NUMBER_COLOR: Color = Color::srgb_u8(0xff, 0xf0, 0x86);
const SPELL_DAMAGE_NUMBER_COLOR: Color = Color::srgb_u8(0xc6, 0x7d, 0xff);
const HEAL_NUMBER_COLOR: Color = Color::srgb_u8(0x68, 0xff, 0x8d);
const DAMAGE_NUMBER_SHADOW_COLOR: Color = Color::srgba(0.0, 0.0, 0.0, 0.88);
const DUMMY_IDLE_HEAL_SECONDS: f32 = 2.0;
const DUMMY_TOTAL_DAMAGE_IDLE_SECONDS: f32 = 10.0;

/// Description:
/// Tracks the previously observed health for one target.
#[derive(Component, Debug, Clone)]
pub(super) struct DamageNumberHealthTracker {
    previous_health: f32,
}

/// Description:
/// Stores animation state for one floating damage number.
#[derive(Component, Debug, Clone)]
pub(super) struct DamageNumber {
    start: Vec3,
    timer: Timer,
    color: Color,
    shadow: bool,
}

/// Description:
/// Initializes health trackers for new dummy targets without spawning damage text.
pub(super) fn initialize_damage_number_health_trackers(
    mut commands: Commands,
    target_query: Query<(Entity, &TrainingDummy), Without<DamageNumberHealthTracker>>,
) {
    for (entity, target) in &target_query {
        commands.entity(entity).insert(DamageNumberHealthTracker {
            previous_health: target.health.max(0.0),
        });
    }
}

/// Description:
/// Restores idle dummy targets to full health after they have not been hit for a short time.
pub(super) fn heal_idle_training_dummies(
    time: Res<Time>,
    mut target_query: Query<&mut TrainingDummy>,
) {
    for mut target in &mut target_query {
        if target.total_damage > 0.0 {
            target.total_damage_idle_seconds += time.delta_secs();
            if target.total_damage_idle_seconds >= DUMMY_TOTAL_DAMAGE_IDLE_SECONDS {
                target.total_damage = 0.0;
                target.total_damage_idle_seconds = 0.0;
            }
        }

        if target.health >= target.max_health {
            target.idle_seconds = 0.0;
            continue;
        }

        target.idle_seconds += time.delta_secs();
        if target.idle_seconds >= DUMMY_IDLE_HEAL_SECONDS {
            let healed = target.heal_to_full();
            if healed > f32::EPSILON {
                info!("TrainingDummy healed after idle: +{:.1} HP", healed);
            }
        }
    }
}

/// Description:
/// Detects target health changes and spawns floating combat numbers.
pub(super) fn spawn_damage_numbers_from_dummy_health(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut target_query: Query<(
        &mut TrainingDummy,
        &GlobalTransform,
        &mut DamageNumberHealthTracker,
    )>,
) {
    for (mut target, transform, mut tracker) in &mut target_query {
        let current_health = target.health.max(0.0);
        let delta = current_health - tracker.previous_health;
        tracker.previous_health = current_health;

        let pending_number = target.take_pending_combat_number();
        let (amount, kind) = if let Some((amount, kind)) = pending_number {
            (amount, kind)
        } else if delta.abs() > f32::EPSILON {
            let kind = if delta > 0.0 {
                TrainingDummyHealthChangeKind::Heal
            } else {
                target.last_health_change_kind
            };
            (delta.abs(), kind)
        } else {
            continue;
        };

        spawn_combat_number(
            &mut commands,
            &asset_server,
            &mut materials,
            transform.translation() + Vec3::Y * DAMAGE_NUMBER_Y_OFFSET,
            amount,
            kind,
        );
    }
}

/// Description:
/// Updates floating damage number hold, fall, fade, and billboard facing.
pub(super) fn update_damage_numbers(
    time: Res<Time>,
    mut commands: Commands,
    camera_query: Query<&GlobalTransform, With<TopDownCamera>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut number_query: Query<(
        Entity,
        &mut DamageNumber,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let camera_rotation = camera_query
        .single()
        .ok()
        .map(GlobalTransform::rotation)
        .unwrap_or(Quat::IDENTITY);

    for (entity, mut number, mut transform, material_handle) in &mut number_query {
        number.timer.tick(time.delta());

        let elapsed = number.timer.elapsed_secs();
        let fall_progress =
            ((elapsed - DAMAGE_NUMBER_HOLD_SECONDS) / DAMAGE_NUMBER_FALL_SECONDS).clamp(0.0, 1.0);
        transform.translation =
            number.start - Vec3::Y * (DAMAGE_NUMBER_FALL_DISTANCE * fall_progress);
        transform.rotation = camera_rotation;

        let alpha = 1.0 - fall_progress;
        if let Some(mut material) = materials.get_mut(&material_handle.0) {
            material.base_color = material.base_color.with_alpha(alpha);
            material.emissive = if number.shadow {
                DAMAGE_NUMBER_SHADOW_COLOR.with_alpha(alpha * 0.75).into()
            } else {
                number.color.with_alpha(alpha * 0.75).into()
            };
        }

        if number.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Runs the spawn combat number step for the floating combat text system.
fn spawn_combat_number(
    commands: &mut Commands,
    asset_server: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
    position: Vec3,
    amount: f32,
    kind: TrainingDummyHealthChangeKind,
) {
    let text = match kind {
        TrainingDummyHealthChangeKind::Heal => format!("+{:.0}", amount.ceil()),
        TrainingDummyHealthChangeKind::AutoAttack | TrainingDummyHealthChangeKind::Spell => {
            format!("{:.0}", amount.ceil())
        }
    };
    let color = combat_number_color(kind);
    let font = asset_server.load(DAMAGE_NUMBER_FONT_PATH);
    let lifetime = DAMAGE_NUMBER_HOLD_SECONDS + DAMAGE_NUMBER_FALL_SECONDS;
    let text_material = materials.add(damage_number_material(color));
    let shadow_material = materials.add(damage_number_material(DAMAGE_NUMBER_SHADOW_COLOR));

    commands.spawn((
        Name::new("DamageNumberShadow"),
        DamageNumber {
            start: position + Vec3::new(0.025, -0.025, -0.01),
            timer: Timer::from_seconds(lifetime, TimerMode::Once),
            color: DAMAGE_NUMBER_SHADOW_COLOR,
            shadow: true,
        },
        TextMesh {
            text: text.clone(),
            font: font.clone(),
            style: damage_number_text_style(),
        },
        MeshMaterial3d(shadow_material),
        Transform::from_translation(position + Vec3::new(0.025, -0.025, -0.01))
            .with_scale(Vec3::splat(DAMAGE_NUMBER_SCALE)),
    ));

    commands.spawn((
        Name::new("DamageNumber"),
        DamageNumber {
            start: position,
            timer: Timer::from_seconds(lifetime, TimerMode::Once),
            color,
            shadow: false,
        },
        TextMesh {
            text,
            font,
            style: damage_number_text_style(),
        },
        MeshMaterial3d(text_material),
        Transform::from_translation(position).with_scale(Vec3::splat(DAMAGE_NUMBER_SCALE)),
    ));
}

/// Runs the combat number color step for the floating combat text system.
fn combat_number_color(kind: TrainingDummyHealthChangeKind) -> Color {
    match kind {
        TrainingDummyHealthChangeKind::AutoAttack => AUTO_ATTACK_DAMAGE_NUMBER_COLOR,
        TrainingDummyHealthChangeKind::Spell => SPELL_DAMAGE_NUMBER_COLOR,
        TrainingDummyHealthChangeKind::Heal => HEAL_NUMBER_COLOR,
    }
}

/// Runs the damage number text style step for the floating combat text system.
fn damage_number_text_style() -> TextMeshStyle {
    TextMeshStyle {
        depth: 0.012,
        subdivision: 14,
        anchor: TextAnchor::Center,
        justify: JustifyText::Center,
    }
}

/// Runs the damage number material step for the floating combat text system.
fn damage_number_material(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        emissive: color.with_alpha(0.75).into(),
        alpha_mode: AlphaMode::Blend,
        double_sided: true,
        cull_mode: None,
        unlit: true,
        ..default()
    }
}
