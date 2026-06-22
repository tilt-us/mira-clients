use super::ServerNetworkSettings;
use super::lobby::{
    ActiveServerAbilities, ConnectedPlayers, LoadingScreenReadyPlayers,
    LoadingScreenStatusLogCache, MatchSnapshotBroadcastTimer, SentChampionCatalogClients,
    broadcast_loading_screen_status, broadcast_match_snapshots, rebroadcast_ability_visuals,
    receive_display_ready, receive_player_commands, receive_player_state_updates,
    send_champion_catalogs, update_player_death_and_respawn, update_server_abilities,
};
use bevy::prelude::*;
use core::time::Duration;
use game_shared::network::{
    FIXED_TIMESTEP_HZ, NETCODE_CLIENT_TIMEOUT_SECS, SERVER_REPLICATION_INTERVAL,
    SharedNetworkPlugin,
};
use lightyear::prelude::client::Connected;
use lightyear::prelude::server::*;
use lightyear::prelude::*;

const EMPTY_SERVER_SHUTDOWN_SECONDS: f32 = 60.0;

/// Description:
/// Registers Lightyear server networking and starts the development UDP listener.
pub struct ServerNetworkPlugin;

#[derive(Resource, Debug, Default)]
struct EmptyServerShutdown {
    had_clients: bool,
    idle_seconds: f32,
}

impl Plugin for ServerNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ServerPlugins {
            tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
        })
        .add_plugins(SharedNetworkPlugin)
        .init_resource::<ServerNetworkSettings>()
        .init_resource::<ConnectedPlayers>()
        .init_resource::<ActiveServerAbilities>()
        .init_resource::<LoadingScreenReadyPlayers>()
        .init_resource::<LoadingScreenStatusLogCache>()
        .init_resource::<MatchSnapshotBroadcastTimer>()
        .init_resource::<SentChampionCatalogClients>()
        .init_resource::<EmptyServerShutdown>()
        .add_systems(Startup, start_server)
        .add_systems(
            Update,
            (
                send_champion_catalogs,
                receive_display_ready,
                broadcast_loading_screen_status,
                receive_player_state_updates,
                receive_player_commands,
                update_server_abilities,
                update_player_death_and_respawn,
                rebroadcast_ability_visuals,
                broadcast_match_snapshots,
            )
                .chain(),
        )
        .add_systems(Update, shutdown_empty_server)
        .add_observer(handle_new_client);
    }
}

/// Description:
/// Adds server-to-client replication support to newly connected client links.
///
/// Params:
/// - `trigger`: Observer trigger for the connected client entity.
/// - `commands`: ECS command buffer used to insert replication components.
fn handle_new_client(trigger: On<Add, Connected>, mut commands: Commands) {
    commands
        .entity(trigger.entity)
        .insert(ReplicationSender::new(
            SERVER_REPLICATION_INTERVAL,
            SendUpdatesMode::SinceLastAck,
            false,
        ));
    info!("Lightyear client connected: {:?}", trigger.entity);
}

/// Description:
/// Gracefully exits the dedicated server after all connected players left.
fn shutdown_empty_server(
    clients: Query<Entity, (With<ClientOf>, With<Connected>)>,
    time: Res<Time>,
    mut shutdown: ResMut<EmptyServerShutdown>,
    mut app_exit: MessageWriter<AppExit>,
) {
    let connected_count = clients.iter().count();
    if connected_count > 0 {
        shutdown.had_clients = true;
        shutdown.idle_seconds = 0.0;
        return;
    }

    if !shutdown.had_clients {
        return;
    }

    shutdown.idle_seconds += time.delta_secs();
    if shutdown.idle_seconds < EMPTY_SERVER_SHUTDOWN_SECONDS {
        return;
    }

    info!(
        "No clients connected for {} seconds; shutting down dedicated server.",
        EMPTY_SERVER_SHUTDOWN_SECONDS
    );
    app_exit.write(AppExit::Success);
}

/// Description:
/// Spawns and starts the Lightyear server entity when auto-start is enabled.
///
/// Params:
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
    info!("Lightyear server listening on {}", settings.listen_addr);
    Ok(())
}
