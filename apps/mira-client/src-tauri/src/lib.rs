mod config;
mod content;
mod game;
mod launcher;
mod oauth;

use tauri::Manager;

/// Runs the run step for the desktop client Tauri bootstrap.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(game::GameProcessState::default())
        .manage(content::GameContentManager::default())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            config::client_config,
            content::game_content_status,
            content::ui_asset_root_path,
            game::game_client_status,
            launcher::launcher_status,
            game::launch_game,
            oauth::prepare_oauth_redirect_uri,
            oauth::cancel_oauth_attempt,
            oauth::start_oauth_window,
            oauth::open_system_browser,
            game::stop_game_client
        ])
        .setup(|app| {
            match content::ui_asset_root() {
                Ok(ui_assets) => {
                    if let Err(error) = app.asset_protocol_scope().allow_directory(ui_assets, true)
                    {
                        eprintln!("[mira-client] Could not scope external UI assets: {error}");
                    }
                }
                Err(error) => {
                    // Do not turn a missing external content tree into a Tauri
                    // setup panic. The frontend displays the repair guidance.
                    eprintln!("[mira-client] {error}");
                }
            }
            content::start_game_content_sync(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
