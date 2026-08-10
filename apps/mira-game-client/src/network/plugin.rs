use super::ClientNetworkSettings;
use bevy::prelude::*;
use core::time::Duration;
use mira_game_api::network::{
    NETCODE_CLIENT_TIMEOUT_SECS, PROTOCOL_ID, SharedNetworkPlugin, fixed_timestep_duration,
};
use lightyear::netcode::Key;
use lightyear::prelude::client::*;
use lightyear::prelude::*;

const NETCODE_TOKEN_EXPIRY_TIMEOUT_MULTIPLIER: i32 = 4;
const MILLISECONDS_PER_SECOND: f32 = 1_000.0;
const EXCELLENT_PING_MAX_MILLIS: u32 = 40;
const ACCEPTABLE_PING_MAX_MILLIS: u32 = 80;
const HIGH_PING_MAX_MILLIS: u32 = 120;

/// Registers Lightyear client networking and starts the development client link.
pub struct ClientNetworkPlugin;

/// Stores the most recent round-trip time reported by the connected client.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct NetworkPingState {
    /// Round-trip time when connection statistics are available.
    pub round_trip_time: Option<Duration>,
}

impl Plugin for ClientNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ClientPlugins {
            tick_duration: fixed_timestep_duration(),
        })
        .add_plugins(SharedNetworkPlugin)
        .init_resource::<ClientNetworkSettings>()
        .init_resource::<NetworkPingState>()
        .add_systems(Startup, connect_to_server)
        .add_systems(Update, update_network_ping);
    }
}
/// Spawns and connects the Lightyear client entity when auto-connect is enabled.
///
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
            ReplicationReceiver,
            NetcodeClient::new(
                auth,
                NetcodeConfig {
                    client_timeout_secs: NETCODE_CLIENT_TIMEOUT_SECS,
                    token_expire_secs: NETCODE_CLIENT_TIMEOUT_SECS
                        * NETCODE_TOKEN_EXPIRY_TIMEOUT_MULTIPLIER,
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

fn update_network_ping(
    mut network_ping: ResMut<NetworkPingState>,
    client_links: Query<&Link, With<Client>>,
) {
    network_ping.round_trip_time = client_links
        .iter()
        .next()
        .map(|link| link.stats.rtt)
        .filter(|round_trip_time| !round_trip_time.is_zero());
}

/// Returns the latest round-trip time rounded to milliseconds.
pub fn ping_millis(network_ping: &NetworkPingState) -> u32 {
    network_ping
        .round_trip_time
        .map(|round_trip_time| round_trip_time.as_secs_f32() * MILLISECONDS_PER_SECOND)
        .unwrap_or(0.0)
        .round()
        .max(0.0) as u32
}

/// Formats the latest round-trip time for the HUD.
pub fn ping_text(network_ping: &NetworkPingState) -> String {
    format!("{}ms", ping_millis(network_ping))
}

/// Returns the HUD color associated with the latest round-trip time.
pub fn ping_color(network_ping: &NetworkPingState) -> Color {
    let milliseconds = ping_millis(network_ping);

    if milliseconds <= EXCELLENT_PING_MAX_MILLIS {
        Color::srgb_u8(0x2B, 0xB8, 0x61)
    } else if milliseconds <= ACCEPTABLE_PING_MAX_MILLIS {
        Color::srgb_u8(0xCC, 0x90, 0x1C)
    } else if milliseconds <= HIGH_PING_MAX_MILLIS {
        Color::srgb_u8(0xCC, 0x39, 0x1C)
    } else {
        Color::srgb_u8(0x99, 0x19, 0x00)
    }
}
