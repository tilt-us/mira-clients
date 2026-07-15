/// Stores Launcher Status data used by the desktop launcher status system.
#[derive(serde::Serialize)]
pub(crate) struct LauncherStatus {
    game_binary: &'static str,
    connected: bool,
}

/// Runs the launcher status step for the desktop launcher status system.
#[tauri::command]
pub(crate) fn launcher_status() -> LauncherStatus {
    LauncherStatus {
        game_binary: "mira-game-client",
        connected: false,
    }
}
