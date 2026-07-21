use super::ServerNetworkSettings;
use super::combat::{ServerCombatNumberEvents, broadcast_combat_number_events};
use super::lane::{ServerLaneState, broadcast_lane_snapshots, update_server_lane};
use super::lobby::{
    ActiveServerAbilities, ConnectedPlayers, LeavingPlayers, LoadingScreenReadyPlayers,
    LoadingScreenStatusBroadcastTimer, MatchSnapshotBroadcastTimer, SentChampionCatalogClients,
    ServerPlayerNavigation, broadcast_loading_screen_status, broadcast_match_snapshots,
    handle_client_disconnection, rebroadcast_ability_visuals, receive_client_leave,
    receive_display_ready, receive_player_commands, receive_player_state_updates,
    send_champion_catalogs, update_player_death_and_respawn, update_player_health_regeneration,
    update_server_abilities, update_server_auto_attack_projectiles,
    update_server_player_navigation,
};
use crate::app::control_api::ServerReadiness;
use bevy::prelude::*;
use game_shared::network::{
    NETCODE_CLIENT_TIMEOUT_SECS, SharedNetworkPlugin, fixed_timestep_duration,
};
use lightyear::prelude::client::Connected;
use lightyear::prelude::server::*;
use lightyear::prelude::*;

/// Registers Lightyear server networking and starts the development UDP listener.
pub struct ServerNetworkPlugin;

impl Plugin for ServerNetworkPlugin {
    /// Registers Bevy resources, plugins, or systems for the dedicated server network plugin.
    fn build(&self, app: &mut App) {
        app.add_plugins(ServerPlugins {
            tick_duration: fixed_timestep_duration(),
        })
        .add_plugins(SharedNetworkPlugin)
        .init_resource::<ServerNetworkSettings>()
        .init_resource::<ConnectedPlayers>()
        .init_resource::<ServerCombatNumberEvents>()
        .init_resource::<ActiveServerAbilities>()
        .init_resource::<ServerPlayerNavigation>()
        .init_resource::<ServerLaneState>()
        .init_resource::<LoadingScreenReadyPlayers>()
        .init_resource::<LeavingPlayers>()
        .init_resource::<LoadingScreenStatusBroadcastTimer>()
        .init_resource::<MatchSnapshotBroadcastTimer>()
        .init_resource::<SentChampionCatalogClients>()
        .add_systems(Startup, start_server)
        .add_systems(
            Update,
            (
                send_champion_catalogs,
                receive_client_leave,
                receive_display_ready,
                broadcast_loading_screen_status,
                receive_player_state_updates,
                update_server_auto_attack_projectiles,
                receive_player_commands,
                update_server_abilities,
                update_player_health_regeneration,
                update_server_player_navigation,
                update_player_death_and_respawn,
                update_server_lane,
                broadcast_combat_number_events,
                rebroadcast_ability_visuals,
                broadcast_match_snapshots,
                broadcast_lane_snapshots,
            )
                .chain(),
        )
        .add_systems(Update, sync_server_readiness)
        .add_observer(handle_client_disconnection)
        .add_observer(handle_new_client);
    }
}
/// Adds server-to-client replication support to newly connected client links.
///
/// - `trigger`: Observer trigger for the connected client entity.
/// - `commands`: ECS command buffer used to insert replication components.
fn handle_new_client(trigger: On<Add, Connected>, mut commands: Commands) {
    commands.entity(trigger.entity).insert(ReplicationSender);
}

/// Synchronizes control API readiness with the live UDP listener state.
fn sync_server_readiness(
    servers: Query<
        &LocalAddr,
        (
            With<NetcodeServer>,
            With<ServerUdpIo>,
            With<Linked>,
            With<Started>,
        ),
    >,
    readiness: Res<ServerReadiness>,
) {
    let listen_addr = servers.iter().next();
    let changed = readiness.set_ready(listen_addr.is_some());

    if changed && let Some(listen_addr) = listen_addr {
        info!("Lightyear server ready on {}", listen_addr.0);
    }
}
/// Spawns and starts the Lightyear server entity when auto-start is enabled.
///
/// - `commands`: ECS command buffer used to spawn and start the server entity.
/// - `settings`: Server networking settings used for the listen address.
fn start_server(mut commands: Commands, settings: Res<ServerNetworkSettings>) -> Result {
    if !settings.auto_start {
        return Ok(());
    }

    let server = commands
        .spawn((
            Name::new("LightyearServer"),
            NetcodeServer::new(
                NetcodeConfig::default().with_client_timeout_secs(NETCODE_CLIENT_TIMEOUT_SECS),
            ),
            LocalAddr(settings.listen_addr),
            ServerUdpIo::default(),
        ))
        .id();

    commands.trigger(Start { entity: server });
    Ok(())
}
