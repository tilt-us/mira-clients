use super::geometry::horizontal_distance;
use super::lobby::{
    ConnectedPlayerState, ConnectedPlayers, DEVELOPMENT_PLAYER_HIT_RADIUS, RESPAWN_SECONDS,
};
use bevy::prelude::*;
use game_shared::network::{NetworkCombatNumberEvent, NetworkCombatNumberKind, PlayerStateChannel};
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::*;

const COMBAT_NUMBER_FLUSH_SECONDS: f32 = 0.15;

/// Description:
/// Queues server-authoritative combat numbers until they are broadcast to clients.
#[derive(Resource, Debug)]
pub(super) struct ServerCombatNumberEvents {
    pub(super) events: Vec<NetworkCombatNumberEvent>,
    flush_timer: Timer,
}

impl Default for ServerCombatNumberEvents {
    /// Returns the default server combat-number flush cadence.
    fn default() -> Self {
        Self {
            events: Vec::new(),
            flush_timer: Timer::from_seconds(COMBAT_NUMBER_FLUSH_SECONDS, TimerMode::Repeating),
        }
    }
}

/// Description:
/// Broadcasts server-authoritative combat numbers to all connected clients.
pub(super) fn broadcast_combat_number_events(
    mut clients: Query<
        &mut MessageSender<NetworkCombatNumberEvent>,
        (With<ClientOf>, With<Connected>),
    >,
    mut combat_events: ResMut<ServerCombatNumberEvents>,
    time: Res<Time>,
) {
    if combat_events.events.is_empty() {
        combat_events.flush_timer.tick(time.delta());
        return;
    }

    if !combat_events.flush_timer.tick(time.delta()).just_finished() {
        return;
    }

    let events = std::mem::take(&mut combat_events.events);
    for event in events {
        for mut sender in &mut clients {
            sender.send::<PlayerStateChannel>(event);
        }
    }
}

/// Description:
/// Applies area damage to all valid enemy players in radius.
pub(super) fn apply_area_damage(
    combat_events: &mut ServerCombatNumberEvents,
    players: &mut ConnectedPlayers,
    caster_player_id: u64,
    center: Vec3,
    radius: f32,
    amount: f32,
) {
    let caster_team = players
        .states
        .get(&caster_player_id)
        .map(|caster| caster.team);
    for (target_player_id, target_state) in &mut players.states {
        if *target_player_id == caster_player_id
            || Some(target_state.team) == caster_team
            || target_state.health <= 0.0
        {
            continue;
        }

        if horizontal_distance(target_state.position, center)
            <= radius + DEVELOPMENT_PLAYER_HIT_RADIUS
        {
            apply_damage(
                combat_events,
                *target_player_id,
                target_state,
                amount,
                NetworkCombatNumberKind::Spell,
            );
        }
    }
}

/// Description:
/// Applies damage to one server-side player state.
pub(super) fn apply_damage(
    combat_events: &mut ServerCombatNumberEvents,
    target_player_id: u64,
    target: &mut ConnectedPlayerState,
    amount: f32,
    kind: NetworkCombatNumberKind,
) {
    if target.health <= 0.0 {
        return;
    }

    let previous_health = target.health;
    target.health = (target.health - amount).max(0.0);
    push_combat_number(
        combat_events,
        target_player_id,
        previous_health - target.health,
        kind,
    );
    if target.health <= 0.0 {
        target.moving = false;
        target.respawn_timer = Some(RESPAWN_SECONDS);
    }
}

/// Description:
/// Applies capped healing to one server-side player state.
pub(super) fn apply_heal(
    combat_events: &mut ServerCombatNumberEvents,
    target_player_id: u64,
    target: &mut ConnectedPlayerState,
    amount: f32,
    max_health: f32,
) {
    if target.health <= 0.0 {
        return;
    }

    let previous_health = target.health;
    target.health = (target.health + amount).min(max_health.max(1.0));
    push_combat_number(
        combat_events,
        target_player_id,
        target.health - previous_health,
        NetworkCombatNumberKind::Heal,
    );
}

/// Runs the push combat number step for the dedicated server lobby simulation system.
fn push_combat_number(
    combat_events: &mut ServerCombatNumberEvents,
    target_player_id: u64,
    amount: f32,
    kind: NetworkCombatNumberKind,
) {
    if amount <= f32::EPSILON {
        return;
    }

    if let Some(event) = combat_events
        .events
        .iter_mut()
        .find(|event| event.target_player_id == target_player_id && event.kind == kind)
    {
        event.amount += amount;
        return;
    }

    combat_events.events.push(NetworkCombatNumberEvent {
        target_player_id,
        amount,
        kind,
    });
}
