use crate::app::settings::{ClientLaunchSettings, ClientScreenMode, normalize_accent_color};
use crate::network::ClientNetworkSettings;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
/// Parses matchmaking launch parameters and networking settings from CLI args.
///
/// - `arguments`: Command line arguments without the binary path.
///
/// - `Ok(Some(settings))`: Parsed launch and networking settings.
/// - `Ok(None)`: Help was printed and the client should exit.
/// - `Err(message)`: Invalid CLI arguments.
pub fn client_settings_from_args<I, S>(
    arguments: I,
) -> Result<Option<(ClientLaunchSettings, ClientNetworkSettings)>, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut launch_settings = ClientLaunchSettings::default();
    let mut network_settings = ClientNetworkSettings::default();
    let remaining_arguments = arguments.into_iter().map(Into::into);
    let mut pending_option = None::<String>;

    for argument in remaining_arguments {
        if let Some(option) = pending_option.take() {
            apply_client_arg(
                &mut launch_settings,
                &mut network_settings,
                &option,
                &argument,
            )?;
            continue;
        }

        match argument.as_str() {
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(None);
            }
            "--access-token"
            | "--accent-color"
            | "--match-id"
            | "--player-public-id"
            | "--champion"
            | "--server-control-base-url"
            | "--server-host"
            | "--screen"
            | "--port"
            | "-p"
            | "--char"
            | "-c"
            | "--team"
            | "-t" => {
                pending_option = Some(argument);
            }
            "--dev-preview" => {
                launch_settings.dev_preview = true;
            }
            "--offline-preview" => {
                launch_settings.dev_preview = true;
                network_settings.auto_connect = false;
            }
            _ => {
                if let Some((option, value)) = argument.split_once('=') {
                    apply_client_arg(&mut launch_settings, &mut network_settings, option, value)?;
                } else if let Some(port_value) = argument.strip_prefix("-p") {
                    apply_server_port(&mut launch_settings, &mut network_settings, port_value)?;
                } else {
                    return Err(format!("Unknown argument: {argument}"));
                }
            }
        }
    }

    if let Some(option) = pending_option {
        return Err(format!("Missing value for {option}"));
    }

    normalize_client_bind_addr(&mut network_settings);

    Ok(Some((launch_settings, network_settings)))
}
/// Returns CLI usage text for the playable client.
pub fn usage() -> &'static str {
    "Usage: mira-game-client [OPTIONS]\n\nOptions:\n  --access-token <TOKEN>                 Matchmaking access token\n  --accent-color <HEX>                   Mira client accent color override\n  --match-id <MATCH_ID>                  Matchmaking match id\n  --player-public-id <PLAYER_PUBLIC_ID>  Public player id\n  --champion <CHAMPION>                  Champion slug or id\n  --server-control-base-url <URL>        Dedicated server REST control API base URL\n  --server-host <HOST>                   Hostname or IP of the dedicated server\n  --screen <full|window|borderless>      Game window mode\n  --dev-preview                          Development preview using the configured server\n  --offline-preview                      Development preview without server networking\n  -p, --port <PORT>                      UDP port of the dedicated server\n  -h, --help                             Print help"
}

fn apply_client_arg(
    launch_settings: &mut ClientLaunchSettings,
    network_settings: &mut ClientNetworkSettings,
    option: &str,
    option_value: &str,
) -> Result<(), String> {
    let option_value = require_non_empty_option_value(option, option_value)?;

    match option.trim_start_matches('-') {
        "access-token" => launch_settings.access_token = Some(option_value.to_string()),
        "accent-color" => {
            launch_settings.accent_color = Some(normalize_accent_color(option_value)?)
        }
        "match-id" => launch_settings.match_id = Some(option_value.to_string()),
        "player-public-id" => {
            launch_settings.player_public_id = Some(option_value.to_string());
            network_settings.client_id = parse_player_public_id(option_value)?;
        }
        "champion" | "char" | "c" => launch_settings.champion = Some(option_value.to_string()),
        "server-control-base-url" => {
            launch_settings.server_control_base_url = Some(option_value.to_string());
        }
        "screen" => launch_settings.screen_mode = parse_screen_mode(option_value)?,
        "server-host" => {
            launch_settings.server_host = Some(option_value.to_string());
            network_settings.server_addr =
                resolve_server_addr(option_value, network_settings.server_addr.port())?;
        }
        "port" | "p" => apply_server_port(launch_settings, network_settings, option_value)?,
        "team" | "t" => {}
        _ => return Err(format!("Unknown argument: {option}")),
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

fn apply_server_port(
    launch_settings: &mut ClientLaunchSettings,
    network_settings: &mut ClientNetworkSettings,
    port_value: &str,
) -> Result<(), String> {
    let port = parse_port(port_value)?;
    launch_settings.server_port = Some(port);
    network_settings.server_addr.set_port(port);
    Ok(())
}

fn require_non_empty_option_value<'a>(
    option: &str,
    option_value: &'a str,
) -> Result<&'a str, String> {
    if option_value.is_empty() {
        Err(format!("Missing value for {option}"))
    } else {
        Ok(option_value)
    }
}

fn parse_port(value: &str) -> Result<u16, String> {
    value
        .parse::<u16>()
        .map_err(|_| format!("Invalid port: {value}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_preview_keeps_server_connection_enabled() {
        let (launch_settings, network_settings) = client_settings_from_args(["--dev-preview"])
            .unwrap()
            .unwrap();

        assert!(launch_settings.dev_preview);
        assert!(network_settings.auto_connect);
    }

    #[test]
    fn offline_preview_disables_server_connection() {
        let (launch_settings, network_settings) = client_settings_from_args(["--offline-preview"])
            .unwrap()
            .unwrap();

        assert!(launch_settings.dev_preview);
        assert!(!network_settings.auto_connect);
    }

    #[test]
    fn parses_local_server_launch_params_without_access_token() {
        let (launch_settings, network_settings) = client_settings_from_args([
            "--dev-preview",
            "--match-id",
            "local-dev",
            "--player-public-id",
            "1001",
            "--champion",
            "lira",
            "--server-host",
            "127.0.0.1",
            "--port",
            "5000",
            "--server-control-base-url",
            "http://127.0.0.1:6000",
        ])
        .unwrap()
        .unwrap();

        assert_eq!(launch_settings.access_token, None);
        assert_eq!(launch_settings.match_id.as_deref(), Some("local-dev"));
        assert_eq!(launch_settings.player_public_id.as_deref(), Some("1001"));
        assert_eq!(launch_settings.champion.as_deref(), Some("lira"));
        assert_eq!(network_settings.client_id, 1001);
        assert_eq!(network_settings.server_addr.to_string(), "127.0.0.1:5000");
        assert!(network_settings.auto_connect);
    }
}
