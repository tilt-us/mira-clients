use core::net::SocketAddr;
use game_shared::network::{DEFAULT_CLIENT_ADDR, DEFAULT_SERVER_ADDR};

/// Description:
/// Stores local client networking settings for the Lightyear connector.
///
/// Fields:
/// - `client_id`: Netcode client id used for development authentication.
/// - `local_addr`: UDP socket address bound by the client.
/// - `server_addr`: UDP socket address of the dedicated server.
/// - `auto_connect`: Whether the client should connect during startup.
#[derive(bevy::prelude::Resource, Debug, Clone, Copy)]
pub struct ClientNetworkSettings {
    pub client_id: u64,
    pub local_addr: SocketAddr,
    pub server_addr: SocketAddr,
    pub auto_connect: bool,
}

impl Default for ClientNetworkSettings {
    fn default() -> Self {
        Self {
            client_id: default_development_client_id(),
            local_addr: DEFAULT_CLIENT_ADDR,
            server_addr: DEFAULT_SERVER_ADDR,
            auto_connect: true,
        }
    }
}

/// Description:
/// Builds a unique default Netcode id for local development clients.
///
/// Return:
/// - Process-derived client id suitable for running multiple local test clients.
fn default_development_client_id() -> u64 {
    u64::from(std::process::id())
}
