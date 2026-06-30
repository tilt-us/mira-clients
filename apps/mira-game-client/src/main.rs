use std::env;
use std::process::ExitCode;

/// Description:
/// Starts the playable client app.
fn main() -> ExitCode {
    let (launch_settings, network_settings) =
        match mira_game_client::cli::client_settings_from_args(env::args().skip(1)) {
            Ok(Some(settings)) => settings,
            Ok(None) => return ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("{message}");
                eprintln!();
                eprintln!("{}", mira_game_client::cli::usage());
                return ExitCode::from(2);
            }
        };

    let launch_gate = launch_settings.release_launch_gate();
    mira_game_client::app::run(launch_settings, network_settings, launch_gate);
    ExitCode::SUCCESS
}
