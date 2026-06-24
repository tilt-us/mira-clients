mod app;
mod network;

use app::settings::{ClientLaunchSettings, ClientScreenMode};
use network::ClientNetworkSettings;
use std::env;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::process::ExitCode;

/// Description:
/// Starts the playable client app.
fn main() -> ExitCode {
    let (launch_settings, network_settings) = match client_settings_from_args(env::args().skip(1)) {
        Ok(Some(settings)) => settings,
        Ok(None) => return ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            eprintln!();
            eprintln!("{}", usage());
            return ExitCode::from(2);
        }
    };

    app::run(launch_settings, network_settings);
    ExitCode::SUCCESS
}

/// Description:
/// Parses matchmaking launch parameters and networking settings from CLI args.
///
/// Params:
/// - `args`: Command line args without the binary path.
///
/// Returns:
/// - `Ok(Some(settings))`: Parsed launch and networking settings.
/// - `Ok(None)`: Help was printed and the client should exit.
/// - `Err(message)`: Invalid CLI arguments.
fn client_settings_from_args<I, S>(
    args: I,
) -> Result<Option<(ClientLaunchSettings, ClientNetworkSettings)>, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut launch_settings = ClientLaunchSettings::default();
    let mut network_settings = ClientNetworkSettings::default();
    let mut args = args.into_iter().map(Into::into);
    let mut pending_key = None::<String>;

    while let Some(arg) = args.next() {
        if let Some(key) = pending_key.take() {
            apply_client_arg(&mut launch_settings, &mut network_settings, &key, &arg)?;
            continue;
        }

        match arg.as_str() {
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(None);
            }
            "--access-token"
            | "--accent-color"
            | "--match-id"
            | "--player-public-id"
            | "--champion"
            | "--matchmaking-api-base-url"
            | "--server-control-base-url"
            | "--server-host"
            | "--screen"
            | "--port"
            | "-p"
            | "--char"
            | "-c"
            | "--team"
            | "-t" => {
                pending_key = Some(arg);
            }
            _ => {
                if let Some((key, value)) = arg.split_once('=') {
                    apply_client_arg(&mut launch_settings, &mut network_settings, key, value)?;
                } else if let Some(value) = arg.strip_prefix("-p") {
                    network_settings.server_addr.set_port(parse_port(value)?);
                } else {
                    return Err(format!("Unknown argument: {arg}"));
                }
            }
        }
    }

    if let Some(key) = pending_key {
        return Err(format!("Missing value for {key}"));
    }

    normalize_client_bind_addr(&mut network_settings);

    Ok(Some((launch_settings, network_settings)))
}

/// Description:
/// Applies one parsed CLI key-value pair to client settings.
///
/// Params:
/// - `launch_settings`: Launch settings being built.
/// - `network_settings`: Networking settings being built.
/// - `key`: CLI argument key.
/// - `value`: CLI argument value.
fn apply_client_arg(
    launch_settings: &mut ClientLaunchSettings,
    network_settings: &mut ClientNetworkSettings,
    key: &str,
    value: &str,
) -> Result<(), String> {
    let value = non_empty_value(key, value)?;

    match key.trim_start_matches('-') {
        "access-token" => launch_settings.access_token = Some(value.to_string()),
        "accent-color" => launch_settings.accent_color = Some(parse_accent_color(value)?),
        "match-id" => launch_settings.match_id = Some(value.to_string()),
        "player-public-id" => {
            launch_settings.player_public_id = Some(value.to_string());
            network_settings.client_id = parse_player_public_id(value)?;
        }
        "champion" | "char" | "c" => launch_settings.champion = Some(value.to_string()),
        "matchmaking-api-base-url" => {
            launch_settings.matchmaking_api_base_url = Some(value.to_string());
        }
        "server-control-base-url" => {
            launch_settings.server_control_base_url = Some(value.to_string());
        }
        "screen" => launch_settings.screen_mode = parse_screen_mode(value)?,
        "server-host" => {
            network_settings.server_addr =
                resolve_server_addr(value, network_settings.server_addr.port())?;
        }
        "port" | "p" => network_settings.server_addr.set_port(parse_port(value)?),
        "team" | "t" => {}
        _ => return Err(format!("Unknown argument: {key}")),
    }

    Ok(())
}

fn parse_screen_mode(value: &str) -> Result<ClientScreenMode, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "full" => Ok(ClientScreenMode::Full),
        "window" => Ok(ClientScreenMode::Window),
        "borderless" => Ok(ClientScreenMode::Borderless),
        _ => Err(format!("Invalid screen mode: {value}")),
    }
}

fn parse_player_public_id(value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("Invalid player public id: {value}"))
}

/// Description:
/// Validates and normalizes the accent color passed by the desktop launcher.
///
/// Params:
/// - `value`: CSS-style hex color, for example `#f2c45b`.
///
/// Returns:
/// - Lowercase six-digit hex color including the leading `#`.
fn parse_accent_color(value: &str) -> Result<String, String> {
    let color = value.trim();

    if color.len() == 7
        && color.starts_with('#')
        && color
            .chars()
            .skip(1)
            .all(|character| character.is_ascii_hexdigit())
    {
        return Ok(color.to_ascii_lowercase());
    }

    Err(format!("Invalid accent color: {value}"))
}

fn resolve_server_addr(host: &str, port: u16) -> Result<std::net::SocketAddr, String> {
    (host, port)
        .to_socket_addrs()
        .map_err(|error| format!("Could not resolve server host {host}: {error}"))?
        .next()
        .ok_or_else(|| format!("Could not resolve server host {host}"))
}

fn normalize_client_bind_addr(network_settings: &mut ClientNetworkSettings) {
    if !network_settings.local_addr.ip().is_loopback()
        || network_settings.server_addr.ip().is_loopback()
    {
        return;
    }

    let local_port = network_settings.local_addr.port();
    network_settings.local_addr = if network_settings.server_addr.is_ipv4() {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), local_port)
    } else {
        SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), local_port)
    };
}

/// Description:
/// Validates that a CLI value is present.
///
/// Params:
/// - `key`: CLI argument key.
/// - `value`: CLI argument value.
///
/// Returns:
/// - The original value when it is not empty.
fn non_empty_value<'a>(key: &str, value: &'a str) -> Result<&'a str, String> {
    if value.is_empty() {
        Err(format!("Missing value for {key}"))
    } else {
        Ok(value)
    }
}

/// Description:
/// Parses one UDP/TCP port value.
///
/// Params:
/// - `value`: Port value from the command line.
///
/// Returns:
/// - Parsed port when `value` is a valid `u16`.
fn parse_port(value: &str) -> Result<u16, String> {
    value
        .parse::<u16>()
        .map_err(|_| format!("Invalid port: {value}"))
}

/// Description:
/// Returns CLI usage text for the playable client.
fn usage() -> &'static str {
    "Usage: mira-game-client [OPTIONS]\n\nOptions:\n  --access-token <TOKEN>                 Matchmaking access token\n  --accent-color <HEX>                   Mira client accent color override\n  --match-id <MATCH_ID>                  Matchmaking match id\n  --player-public-id <PLAYER_PUBLIC_ID>  Public player id\n  --champion <CHAMPION>                  Champion slug or id\n  --matchmaking-api-base-url <URL>       Matchmaking API base URL\n  --server-control-base-url <URL>        Dedicated server REST control API base URL\n  --server-host <HOST>                   Hostname or IP of the dedicated server\n  --screen <full|window|borderless>      Game window mode\n  -p, --port <PORT>                      UDP port of the dedicated server\n  -h, --help                             Print help"
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_shared::network::DEFAULT_SERVER_ADDR;

    #[test]
    fn defaults_to_empty_launch_settings() {
        let (launch_settings, network_settings) = client_settings_from_args(Vec::<String>::new())
            .unwrap()
            .unwrap();

        assert_eq!(launch_settings, ClientLaunchSettings::default());
        assert_eq!(network_settings.server_addr, DEFAULT_SERVER_ADDR);
    }

    #[test]
    fn parses_matchmaking_wrapper_args() {
        let (launch_settings, network_settings) = client_settings_from_args([
            "--access-token",
            "token",
            "--accent-color",
            "#38BDF8",
            "--match-id",
            "match-1",
            "--player-public-id",
            "1",
            "--champion",
            "yuna",
            "--matchmaking-api-base-url",
            "https://matchmaking.example.test",
            "--server-control-base-url",
            "http://127.0.0.1:6000",
            "--screen",
            "full",
        ])
        .unwrap()
        .unwrap();

        assert_eq!(launch_settings.access_token.as_deref(), Some("token"));
        assert_eq!(launch_settings.accent_color.as_deref(), Some("#38bdf8"));
        assert_eq!(launch_settings.match_id.as_deref(), Some("match-1"));
        assert_eq!(launch_settings.player_public_id.as_deref(), Some("1"));
        assert_eq!(launch_settings.champion.as_deref(), Some("yuna"));
        assert_eq!(
            launch_settings.matchmaking_api_base_url.as_deref(),
            Some("https://matchmaking.example.test")
        );
        assert_eq!(
            launch_settings.server_control_base_url.as_deref(),
            Some("http://127.0.0.1:6000")
        );
        assert_eq!(launch_settings.screen_mode, ClientScreenMode::Full);
        assert_eq!(
            network_settings.server_addr.port(),
            DEFAULT_SERVER_ADDR.port()
        );
        assert_eq!(network_settings.client_id, 1);
    }

    #[test]
    fn parses_equals_args() {
        let (launch_settings, network_settings) = client_settings_from_args([
            "--access-token=token",
            "--match-id=match-1",
            "--player-public-id=2",
            "--champion=ignara",
            "--matchmaking-api-base-url=https://matchmaking.example.test",
            "--server-control-base-url=http://127.0.0.1:6000",
            "--screen=window",
            "--port=7777",
        ])
        .unwrap()
        .unwrap();

        assert_eq!(launch_settings.access_token.as_deref(), Some("token"));
        assert_eq!(launch_settings.match_id.as_deref(), Some("match-1"));
        assert_eq!(launch_settings.player_public_id.as_deref(), Some("2"));
        assert_eq!(launch_settings.champion.as_deref(), Some("ignara"));
        assert_eq!(
            launch_settings.matchmaking_api_base_url.as_deref(),
            Some("https://matchmaking.example.test")
        );
        assert_eq!(
            launch_settings.server_control_base_url.as_deref(),
            Some("http://127.0.0.1:6000")
        );
        assert_eq!(launch_settings.screen_mode, ClientScreenMode::Window);
        assert_eq!(network_settings.server_addr.port(), 7777);
    }

    #[test]
    fn defaults_to_borderless_screen_mode() {
        let (launch_settings, _) = client_settings_from_args(Vec::<String>::new())
            .unwrap()
            .unwrap();

        assert_eq!(launch_settings.screen_mode, ClientScreenMode::Borderless);
    }

    #[test]
    fn keeps_legacy_char_arg_supported() {
        let (launch_settings, _) = client_settings_from_args(["--char", "sophia"])
            .unwrap()
            .unwrap();

        assert_eq!(launch_settings.champion.as_deref(), Some("sophia"));
    }

    #[test]
    fn parses_long_port_arg() {
        let (_, network_settings) = client_settings_from_args(["--port", "7778"])
            .unwrap()
            .unwrap();

        assert_eq!(network_settings.server_addr.port(), 7778);
    }

    #[test]
    fn parses_server_host_arg() {
        let (_, network_settings) =
            client_settings_from_args(["--server-host", "127.0.0.1", "--port", "7780"])
                .unwrap()
                .unwrap();

        assert_eq!(network_settings.server_addr.ip().to_string(), "127.0.0.1");
        assert_eq!(network_settings.server_addr.port(), 7780);
        assert_eq!(network_settings.local_addr.ip().to_string(), "127.0.0.1");
    }

    #[test]
    fn binds_unspecified_local_addr_for_remote_server_host() {
        let (_, network_settings) =
            client_settings_from_args(["--server-host", "85.215.116.15", "--port", "7780"])
                .unwrap()
                .unwrap();

        assert_eq!(
            network_settings.server_addr.ip().to_string(),
            "85.215.116.15"
        );
        assert_eq!(network_settings.server_addr.port(), 7780);
        assert_eq!(network_settings.local_addr.ip().to_string(), "0.0.0.0");
    }

    #[test]
    fn parses_short_port_arg() {
        let (_, network_settings) = client_settings_from_args(["-p7779"]).unwrap().unwrap();

        assert_eq!(network_settings.server_addr.port(), 7779);
    }

    #[test]
    fn rejects_unknown_arg() {
        let error = client_settings_from_args(["--unknown"]).unwrap_err();

        assert_eq!(error, "Unknown argument: --unknown");
    }

    #[test]
    fn rejects_missing_arg_value() {
        let error = client_settings_from_args(["--access-token"]).unwrap_err();

        assert_eq!(error, "Missing value for --access-token");
    }

    #[test]
    fn rejects_invalid_port() {
        let error = client_settings_from_args(["--port", "70000"]).unwrap_err();

        assert_eq!(error, "Invalid port: 70000");
    }

    #[test]
    fn rejects_invalid_screen_mode() {
        let error = client_settings_from_args(["--screen", "giant"]).unwrap_err();

        assert_eq!(error, "Invalid screen mode: giant");
    }
}
