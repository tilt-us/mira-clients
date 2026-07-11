#[derive(serde::Serialize)]
pub(crate) struct LauncherStatus {
    game_binary: &'static str,
    connected: bool,
}

#[tauri::command]
pub(crate) fn launcher_status() -> LauncherStatus {
    LauncherStatus {
        game_binary: "mira-game-client",
        connected: false,
    }
}
