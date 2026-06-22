use super::ClientNetworkSettings;
use bevy::prelude::*;
use core::time::Duration;
use game_shared::network::{
    FIXED_TIMESTEP_HZ, NETCODE_CLIENT_TIMEOUT_SECS, PROTOCOL_ID, SharedNetworkPlugin,
};
use lightyear::netcode::Key;
use lightyear::prelude::client::*;
use lightyear::prelude::*;

/// Description:
/// Registers Lightyear client networking and starts the development client link.
pub struct ClientNetworkPlugin;

impl Plugin for ClientNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ClientPlugins {
            tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
        })
        .add_plugins(SharedNetworkPlugin)
        .init_resource::<ClientNetworkSettings>()
        .add_systems(Startup, connect_to_server);
    }
}

/// Description:
/// Spawns and connects the Lightyear client entity when auto-connect is enabled.
///
/// Params:
/// - `commands`: ECS command buffer used to spawn and connect the client entity.
/// - `settings`: Client networking settings used for local and remote addresses.
fn connect_to_server(mut commands: Commands, settings: Res<ClientNetworkSettings>) -> Result {
    if !settings.auto_connect {
        return Ok(());
    }

    let auth = Authentication::Manual {
        server_addr: settings.server_addr,
        client_id: settings.client_id,
        private_key: Key::default(),
        protocol_id: PROTOCOL_ID,
    };

    let client = commands
        .spawn((
            Name::new("LightyearClient"),
            Client::default(),
            LocalAddr(settings.local_addr),
            PeerAddr(settings.server_addr),
            Link::new(None),
            ReplicationReceiver::default(),
            NetcodeClient::new(
                auth,
                NetcodeConfig {
                    client_timeout_secs: NETCODE_CLIENT_TIMEOUT_SECS,
                    token_expire_secs: NETCODE_CLIENT_TIMEOUT_SECS * 4,
                    ..Default::default()
                },
            )?,
            UdpIo::default(),
        ))
        .id();

    commands.trigger(Connect { entity: client });
    info!(
        "Lightyear client connecting from {} to {}",
        settings.local_addr, settings.server_addr
    );
    Ok(())
}
