mod config;
mod content;
mod game;
mod launcher;
mod oauth;

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
            oauth::start_oauth_window,
            game::stop_game_client
        ])
        .setup(|app| {
            content::start_game_content_sync(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
