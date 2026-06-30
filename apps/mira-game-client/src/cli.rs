use crate::app::settings::{ClientLaunchSettings, ClientLaunchStage, ClientScreenMode};
use crate::network::ClientNetworkSettings;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};

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
pub fn client_settings_from_args<I, S>(
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
            | "--stage"
            | "--port"
            | "-p"
            | "--char"
            | "-c"
            | "--team"
            | "-t" => {
                pending_key = Some(arg);
            }
            "--dev-preview" => {
                launch_settings.dev_preview = true;
                network_settings.auto_connect = false;
            }
            _ => {
                if let Some((key, value)) = arg.split_once('=') {
                    apply_client_arg(&mut launch_settings, &mut network_settings, key, value)?;
                } else if let Some(value) = arg.strip_prefix("-p") {
                    let port = parse_port(value)?;
                    launch_settings.server_port = Some(port);
                    network_settings.server_addr.set_port(port);
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
/// Returns CLI usage text for the playable client.
pub fn usage() -> &'static str {
    "Usage: mira-game-client [OPTIONS]\n\nOptions:\n  --access-token <TOKEN>                 Matchmaking access token\n  --accent-color <HEX>                   Mira client accent color override\n  --match-id <MATCH_ID>                  Matchmaking match id\n  --player-public-id <PLAYER_PUBLIC_ID>  Public player id\n  --champion <CHAMPION>                  Champion slug or id\n  --matchmaking-api-base-url <URL>       Matchmaking API base URL\n  --server-control-base-url <URL>        Dedicated server REST control API base URL\n  --server-host <HOST>                   Hostname or IP of the dedicated server\n  --stage <Local|Dev>                    API stage for release auth validation\n  --screen <full|window|borderless>      Game window mode\n  --dev-preview                          Development-only map and mechanics preview\n  -p, --port <PORT>                      UDP port of the dedicated server\n  -h, --help                             Print help"
}

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
        "stage" => launch_settings.stage = Some(parse_launch_stage(value)?),
        "screen" => launch_settings.screen_mode = parse_screen_mode(value)?,
        "server-host" => {
            launch_settings.server_host = Some(value.to_string());
            network_settings.server_addr =
                resolve_server_addr(value, network_settings.server_addr.port())?;
        }
        "port" | "p" => {
            let port = parse_port(value)?;
            launch_settings.server_port = Some(port);
            network_settings.server_addr.set_port(port);
        }
        "team" | "t" => {}
        _ => return Err(format!("Unknown argument: {key}")),
    }

    Ok(())
}

fn parse_launch_stage(value: &str) -> Result<ClientLaunchStage, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "local" => Ok(ClientLaunchStage::Local),
        "dev" => Ok(ClientLaunchStage::Dev),
        _ => Err(format!("Invalid stage: {value}")),
    }
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

fn non_empty_value<'a>(key: &str, value: &'a str) -> Result<&'a str, String> {
    if value.is_empty() {
        Err(format!("Missing value for {key}"))
    } else {
        Ok(value)
    }
}

fn parse_port(value: &str) -> Result<u16, String> {
    value
        .parse::<u16>()
        .map_err(|_| format!("Invalid port: {value}"))
}
