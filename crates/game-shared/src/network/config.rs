use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use core::time::Duration;
/// Fixed networking tick rate used by both client and server.
pub const FIXED_TIMESTEP_HZ: f64 = 64.0;

/// Returns the duration of one fixed networking tick.
pub fn fixed_timestep_duration() -> Duration {
    Duration::from_secs_f64(FIXED_TIMESTEP_HZ.recip())
}

/// Interval used by the server when sending replication updates.
pub const SERVER_REPLICATION_INTERVAL: Duration = Duration::from_millis(100);
/// Protocol id used by Netcode authentication.
pub const PROTOCOL_ID: u64 = 0;
/// Timeout in seconds before Netcode drops a temporarily silent client.
///
/// Keep this short enough that a crashed client can reconnect without holding
/// its Netcode client id for too long, but above normal loading hiccups.
pub const NETCODE_CLIENT_TIMEOUT_SECS: i32 = 8;
/// Default UDP socket address used by the dedicated server in development.
pub const DEFAULT_SERVER_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 5000);
/// Default UDP socket address used by local clients in development.
///
/// Port `0` lets the operating system assign a free UDP port per client process.
pub const DEFAULT_CLIENT_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
