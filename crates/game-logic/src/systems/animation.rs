use super::{LocalChampionAnimationState, LocalChampionAnimations};
use bevy::prelude::*;
use game_shared::game::player::{MoveTarget, PlayerControlled};
use std::time::Duration;

const LOCAL_STOP_ANIMATION_GRACE_SECONDS: f32 = 0.12;

/// Description:
/// Initializes newly loaded animation players with Lira's graph and idle clip.
///
/// Params:
/// - `commands`: ECS command buffer used to attach animation components.
/// - `animations`: Local champion animation graph and node indices.
/// - `players`: Newly added animation players waiting for graph setup.
pub(super) fn setup_animation_player_once_loaded(
    mut commands: Commands,
    animations: Res<LocalChampionAnimations>,
    mut players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
) {
    for (entity, mut player) in &mut players {
        let mut transitions = AnimationTransitions::new();
        transitions
            .play(&mut player, animations.idle, Duration::ZERO)
            .repeat();

        commands
            .entity(entity)
            .insert(AnimationGraphHandle(animations.graph.clone()))
            .insert(transitions);
    }
}

/// Description:
/// Switches the controlled champion between idle and walk animations.
///
/// Params:
/// - `animation_state`: Cached movement animation state for change detection.
/// - `animations`: Optional local champion animation data loaded during setup.
/// - `moving_query`: Query that reports whether the controlled player has a move target.
/// - `animation_players`: Animation players and transitions to update.
pub(super) fn sync_controlled_player_animation(
    time: Res<Time>,
    mut animation_state: ResMut<LocalChampionAnimationState>,
    animations: Option<Res<LocalChampionAnimations>>,
    moving_query: Query<Entity, (With<PlayerControlled>, With<MoveTarget>)>,
    controlled_query: Query<Entity, With<PlayerControlled>>,
    mut animation_players: Query<(Entity, &mut AnimationPlayer, &mut AnimationTransitions)>,
    parents: Query<&ChildOf>,
) {
    let Some(animations) = animations else {
        return;
    };

    let is_moving = !moving_query.is_empty();
    if !is_moving && animation_state.moving {
        animation_state.stop_grace_seconds += time.delta_secs();
        if animation_state.stop_grace_seconds < LOCAL_STOP_ANIMATION_GRACE_SECONDS {
            return;
        }
    } else {
        animation_state.stop_grace_seconds = 0.0;
    }

    if is_moving == animation_state.moving {
        return;
    }

    animation_state.moving = is_moving;
    animation_state.stop_grace_seconds = 0.0;
    let next_animation = if is_moving {
        animations.walk
    } else {
        animations.idle
    };

    for (animation_entity, mut player, mut transitions) in &mut animation_players {
        let animation_root = hierarchy_root(animation_entity, &parents);
        if controlled_query.get(animation_root).is_err() {
            continue;
        }

        transitions
            .play(&mut player, next_animation, Duration::from_millis(140))
            .repeat();
    }
}

/// Description:
/// Finds the top-most hierarchy root for a scene child entity.
///
/// Params:
/// - `entity`: Entity to walk upward from.
/// - `parents`: Parent relationship query.
///
/// Return:
/// - Top-most hierarchy entity.
fn hierarchy_root(mut entity: Entity, parents: &Query<&ChildOf>) -> Entity {
    while let Ok(parent) = parents.get(entity) {
        entity = parent.0;
    }
    entity
}
