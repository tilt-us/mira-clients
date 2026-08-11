use super::{
    LocalChampionAnimationState, LocalChampionAnimations, hierarchy_root,
    networked_players::LocalAuthoritativeTransform,
};
use bevy::prelude::*;
use mira_game_api::game::player::PlayerControlled;
use std::time::Duration;

const LOCAL_STOP_ANIMATION_GRACE_SECONDS: f32 = 0.12;
/// Initializes newly loaded animation players with Lira's graph and idle clip.
///
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
/// Switches the controlled champion between idle and walk animations.
///
/// - `animation_state`: Cached movement animation state for change detection.
/// - `animations`: Optional local champion animation data loaded during setup.
/// - `moving_query`: Query that reports the controlled player's server-authoritative locomotion.
/// - `animation_players`: Animation players and transitions to update.
pub(super) fn sync_controlled_player_animation(
    time: Res<Time>,
    mut animation_state: ResMut<LocalChampionAnimationState>,
    animations: Option<Res<LocalChampionAnimations>>,
    moving_query: Query<&LocalAuthoritativeTransform, With<PlayerControlled>>,
    controlled_query: Query<Entity, With<PlayerControlled>>,
    mut animation_players: Query<(Entity, &mut AnimationPlayer, &mut AnimationTransitions)>,
    parents: Query<&ChildOf>,
) {
    let Some(animations) = animations else {
        return;
    };

    let is_moving = moving_query.iter().any(|state| state.moving);
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
