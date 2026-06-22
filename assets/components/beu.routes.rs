use bevy_extended_ui::routing::Routes;
use bevy_extended_ui_macros::beu_routes;

#[beu_routes]
/// Description:
/// Builds the Extended UI route table for the client UI.
///
/// Return:
/// - Configured route table for the UI application.
pub fn routes() -> Routes {
    Routes::new()
        .route("/", "app-loading-screen")
        .route("/hud", "app-main")
        .redirect("", "/")
        .fallback("app-main")
}
