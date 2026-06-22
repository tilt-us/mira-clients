use core::net::SocketAddr;
use game_shared::network::DEFAULT_SERVER_ADDR;

/// Description:
/// Stores dedicated server networking settings for the Lightyear listener.
///
/// Fields:
/// - `listen_addr`: UDP socket address bound by the server.
/// - `auto_start`: Whether the server should start listening during startup.
#[derive(bevy::prelude::Resource, Debug, Clone, Copy)]
pub struct ServerNetworkSettings {
    pub listen_addr: SocketAddr,
    pub auto_start: bool,
}

impl Default for ServerNetworkSettings {
    fn default() -> Self {
        Self {
            listen_addr: DEFAULT_SERVER_ADDR,
            auto_start: true,
        }
    }
}
