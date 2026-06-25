use mira_game_client::app::settings::ClientScreenMode;
use mira_game_client::cli::{client_settings_from_args, usage};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[test]
fn parses_complete_launch_settings() {
    let (launch_settings, network_settings) = client_settings_from_args([
        "--access-token",
        "token-123",
        "--accent-color=#ABCDEF",
        "--match-id",
        "match-1",
        "--player-public-id",
        "42",
        "--champion",
        "yuna",
        "--matchmaking-api-base-url",
        "https://api.test/match",
        "--server-control-base-url",
        "https://server.test",
        "--server-host",
        "8.8.8.8",
        "--screen",
        "window",
        "-p",
        "4100",
        "--team",
        "blue",
    ])
    .expect("settings should parse")
    .expect("help should not be requested");

    assert_eq!(launch_settings.access_token.as_deref(), Some("token-123"));
    assert_eq!(launch_settings.accent_color.as_deref(), Some("#abcdef"));
    assert_eq!(launch_settings.match_id.as_deref(), Some("match-1"));
    assert_eq!(launch_settings.player_public_id.as_deref(), Some("42"));
    assert_eq!(launch_settings.champion.as_deref(), Some("yuna"));
    assert_eq!(
        launch_settings.matchmaking_api_base_url.as_deref(),
        Some("https://api.test/match"),
    );
    assert_eq!(
        launch_settings.server_control_base_url.as_deref(),
        Some("https://server.test"),
    );
    assert_eq!(launch_settings.screen_mode, ClientScreenMode::Window);
    assert_eq!(network_settings.client_id, 42);
    assert_eq!(
        network_settings.server_addr.ip(),
        IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))
    );
    assert_eq!(network_settings.server_addr.port(), 4100);
    assert_eq!(
        network_settings.local_addr.ip(),
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    );
}

#[test]
fn parses_short_aliases_and_inline_port() {
    let (launch_settings, network_settings) =
        client_settings_from_args(["-c", "ignara", "-t", "red", "-p4242", "--screen=full"])
            .expect("settings should parse")
            .expect("help should not be requested");

    assert_eq!(launch_settings.champion.as_deref(), Some("ignara"));
    assert_eq!(launch_settings.screen_mode, ClientScreenMode::Full);
    assert_eq!(network_settings.server_addr.port(), 4242);
}

#[test]
fn normalizes_ipv6_bind_addr_for_remote_ipv6_servers() {
    let (_launch_settings, network_settings) =
        client_settings_from_args(["--server-host", "2001:4860:4860::8888"])
            .expect("settings should parse")
            .expect("help should not be requested");

    assert_eq!(
        network_settings.local_addr.ip(),
        IpAddr::V6(Ipv6Addr::UNSPECIFIED)
    );
}

#[test]
fn returns_none_for_help() {
    assert!(
        client_settings_from_args(["--help"])
            .expect("help should parse")
            .is_none()
    );
    assert!(usage().contains("--player-public-id"));
}

#[test]
fn rejects_invalid_arguments() {
    assert_eq!(
        client_settings_from_args(["--unknown"]).expect_err("unknown args should fail"),
        "Unknown argument: --unknown",
    );
    assert_eq!(
        client_settings_from_args(["--unknown=value"]).expect_err("unknown keys should fail"),
        "Unknown argument: --unknown",
    );
    assert_eq!(
        client_settings_from_args(["--match-id="]).expect_err("empty values should fail"),
        "Missing value for --match-id",
    );
    assert_eq!(
        client_settings_from_args(["--accent-color", "blue"])
            .expect_err("invalid colors should fail"),
        "Invalid accent color: blue",
    );
    assert_eq!(
        client_settings_from_args(["--screen", "tablet"])
            .expect_err("invalid screen modes should fail"),
        "Invalid screen mode: tablet",
    );
    assert_eq!(
        client_settings_from_args(["--player-public-id", "abc"])
            .expect_err("invalid public ids should fail"),
        "Invalid player public id: abc",
    );
    assert_eq!(
        client_settings_from_args(["--port", "abc"]).expect_err("invalid ports should fail"),
        "Invalid port: abc",
    );
    assert_eq!(
        client_settings_from_args(["--match-id"]).expect_err("missing values should fail"),
        "Missing value for --match-id",
    );
}
