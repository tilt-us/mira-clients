// Prevents an extra console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Runs the main step for the desktop client binary entrypoint.
fn main() {
    mira_client_lib::run()
}
