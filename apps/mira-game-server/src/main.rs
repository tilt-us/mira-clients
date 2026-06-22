mod app;

use app::control_api::ServerControlApiSettings;
use app::network::ServerNetworkSettings;
use std::env;
use std::net::IpAddr;
use std::process::ExitCode;

/// Description:
/// Starts the dedicated server app.
fn main() -> ExitCode {
    let (network_settings, control_api_settings) =
        match server_settings_from_args(env::args().skip(1)) {
            Ok(Some(settings)) => settings,
            Ok(None) => return ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("{message}");
                eprintln!();
                eprintln!("{}", usage());
                return ExitCode::from(2);
            }
        };

    app::run(network_settings, control_api_settings);
    ExitCode::SUCCESS
}

/// Description:
/// Parses dedicated server networking settings from CLI args.
///
/// Params:
/// - `args`: Command line args without the binary path.
///
/// Returns:
/// - `Ok(Some(settings))`: Parsed server settings.
/// - `Ok(None)`: Help was printed and the server should exit.
/// - `Err(message)`: Invalid CLI arguments.
fn server_settings_from_args<I, S>(
    args: I,
) -> Result<Option<(ServerNetworkSettings, ServerControlApiSettings)>, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut settings = ServerNetworkSettings::default();
    let mut control_api_settings = ServerControlApiSettings::default();
    let mut args = args.into_iter().map(Into::into);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(None);
            }
            "--port" | "-p" => {
                let Some(port) = args.next() else {
                    return Err(format!("Missing value for {arg}"));
                };
                let port = parse_port(&port)?;
                settings.listen_addr.set_port(port);
                control_api_settings.listen_addr.set_port(port);
            }
            "--host" | "--bind-host" => {
                let Some(host) = args.next() else {
                    return Err(format!("Missing value for {arg}"));
                };
                settings.listen_addr.set_ip(parse_host(&host)?);
            }
            "--control-host" | "--control-bind-host" => {
                let Some(host) = args.next() else {
                    return Err(format!("Missing value for {arg}"));
                };
                control_api_settings.listen_addr.set_ip(parse_host(&host)?);
            }
            "--control-port" => {
                let Some(port) = args.next() else {
                    return Err(format!("Missing value for {arg}"));
                };
                control_api_settings
                    .listen_addr
                    .set_port(parse_port(&port)?);
            }
            _ if arg.starts_with("--port=") => {
                let port = parse_port(arg.trim_start_matches("--port="))?;
                settings.listen_addr.set_port(port);
                control_api_settings.listen_addr.set_port(port);
            }
            _ if arg.starts_with("--host=") => {
                settings
                    .listen_addr
                    .set_ip(parse_host(arg.trim_start_matches("--host="))?);
            }
            _ if arg.starts_with("--bind-host=") => {
                settings
                    .listen_addr
                    .set_ip(parse_host(arg.trim_start_matches("--bind-host="))?);
            }
            _ if arg.starts_with("--control-host=") => {
                control_api_settings
                    .listen_addr
                    .set_ip(parse_host(arg.trim_start_matches("--control-host="))?);
            }
            _ if arg.starts_with("--control-bind-host=") => {
                control_api_settings
                    .listen_addr
                    .set_ip(parse_host(arg.trim_start_matches("--control-bind-host="))?);
            }
            _ if arg.starts_with("--control-port=") => {
                control_api_settings
                    .listen_addr
                    .set_port(parse_port(arg.trim_start_matches("--control-port="))?);
            }
            _ if arg.starts_with("-p") && arg.len() > 2 => {
                let port = parse_port(arg.trim_start_matches("-p"))?;
                settings.listen_addr.set_port(port);
                control_api_settings.listen_addr.set_port(port);
            }
            _ => return Err(format!("Unknown argument: {arg}")),
        }
    }

    Ok(Some((settings, control_api_settings)))
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
/// Parses one IP address used as dedicated-server bind host.
///
/// Params:
/// - `value`: IP address from the command line.
///
/// Returns:
/// - Parsed IP address when `value` is valid.
fn parse_host(value: &str) -> Result<IpAddr, String> {
    value
        .parse::<IpAddr>()
        .map_err(|_| format!("Invalid host: {value}"))
}

/// Description:
/// Returns CLI usage text for the dedicated server.
fn usage() -> &'static str {
    "Usage: mira-game-server [--host <IP>] [--port <PORT>]\n\nOptions:\n      --host <IP>              IP address the UDP server binds to\n      --bind-host <IP>         Alias for --host\n  -p, --port <PORT>            UDP port the server listens on\n      --control-host <IP>      IP address the REST control API binds to\n      --control-bind-host <IP> Alias for --control-host\n      --control-port <PORT>    TCP port the REST control API listens on\n  -h, --help                   Print help"
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_shared::network::DEFAULT_SERVER_ADDR;
    use std::net::SocketAddr;

    #[test]
    fn defaults_to_shared_server_addr() {
        let (settings, control_settings) = server_settings_from_args(Vec::<String>::new())
            .unwrap()
            .unwrap();

        assert_eq!(settings.listen_addr, DEFAULT_SERVER_ADDR);
        assert_eq!(
            control_settings.listen_addr,
            SocketAddr::new("127.0.0.1".parse::<IpAddr>().unwrap(), 6000)
        );
    }

    #[test]
    fn parses_long_port_arg() {
        let (settings, control_settings) = server_settings_from_args(["--port", "7777"])
            .unwrap()
            .unwrap();

        assert_eq!(settings.listen_addr.port(), 7777);
        assert_eq!(control_settings.listen_addr.port(), 7777);
    }

    #[test]
    fn parses_long_port_equals_arg() {
        let (settings, control_settings) =
            server_settings_from_args(["--port=7778"]).unwrap().unwrap();

        assert_eq!(settings.listen_addr.port(), 7778);
        assert_eq!(control_settings.listen_addr.port(), 7778);
    }

    #[test]
    fn parses_long_host_arg() {
        let (settings, _) = server_settings_from_args(["--host", "0.0.0.0"])
            .unwrap()
            .unwrap();

        assert_eq!(
            settings.listen_addr.ip(),
            "0.0.0.0".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn parses_long_bind_host_equals_arg() {
        let (settings, _) = server_settings_from_args(["--bind-host=0.0.0.0"])
            .unwrap()
            .unwrap();

        assert_eq!(
            settings.listen_addr.ip(),
            "0.0.0.0".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn parses_short_port_arg() {
        let (settings, control_settings) = server_settings_from_args(["-p7779"]).unwrap().unwrap();

        assert_eq!(settings.listen_addr.port(), 7779);
        assert_eq!(control_settings.listen_addr.port(), 7779);
    }

    #[test]
    fn explicit_control_port_overrides_main_port() {
        let (settings, control_settings) =
            server_settings_from_args(["--port", "7779", "--control-port", "6001"])
                .unwrap()
                .unwrap();

        assert_eq!(settings.listen_addr.port(), 7779);
        assert_eq!(control_settings.listen_addr.port(), 6001);
    }

    #[test]
    fn rejects_invalid_port() {
        let error = server_settings_from_args(["--port", "70000"]).unwrap_err();

        assert_eq!(error, "Invalid port: 70000");
    }

    #[test]
    fn rejects_invalid_host() {
        let error = server_settings_from_args(["--host", "localhost"]).unwrap_err();

        assert_eq!(error, "Invalid host: localhost");
    }

    #[test]
    fn parses_control_api_args() {
        let (_, control_settings) =
            server_settings_from_args(["--control-host", "0.0.0.0", "--control-port", "6001"])
                .unwrap()
                .unwrap();

        assert_eq!(
            control_settings.listen_addr,
            SocketAddr::new("0.0.0.0".parse::<IpAddr>().unwrap(), 6001)
        );
    }
}
