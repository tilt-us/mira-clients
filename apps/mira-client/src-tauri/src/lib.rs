mod config;
mod game;
mod launcher;
mod oauth;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(game::GameProcessState::default())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            config::client_config,
            game::game_client_status,
            launcher::launcher_status,
            game::launch_game,
            oauth::start_oauth_window,
            game::stop_game_client
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
