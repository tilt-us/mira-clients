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

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct NetworkPingState {
    pub rtt: Option<Duration>,
}

impl Plugin for ClientNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ClientPlugins {
            tick_duration: Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ),
        })
        .add_plugins(SharedNetworkPlugin)
        .init_resource::<ClientNetworkSettings>()
        .init_resource::<NetworkPingState>()
        .add_systems(Startup, connect_to_server)
        .add_systems(Update, update_network_ping);
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

fn update_network_ping(mut ping: ResMut<NetworkPingState>, clients: Query<&Link, With<Client>>) {
    ping.rtt = clients
        .iter()
        .next()
        .map(|link| link.stats.rtt)
        .filter(|rtt| !rtt.is_zero());
}

pub fn ping_millis(ping: &NetworkPingState) -> u32 {
    ping.rtt
        .map(|rtt| rtt.as_secs_f32() * 1000.0)
        .unwrap_or(0.0)
        .round()
        .max(0.0) as u32
}

pub fn ping_text(ping: &NetworkPingState) -> String {
    format!("{}ms", ping_millis(ping))
}

pub fn ping_color(ping: &NetworkPingState) -> Color {
    match ping_millis(ping) {
        0..=40 => Color::srgb_u8(0x2B, 0xB8, 0x61),
        41..=80 => Color::srgb_u8(0xCC, 0x90, 0x1C),
        81..=120 => Color::srgb_u8(0xCC, 0x39, 0x1C),
        _ => Color::srgb_u8(0x99, 0x19, 0x00),
    }
}
