use crate::{game::team::TeamSpec, network::ChampionId};
use serde::Deserialize;

/// Launcher-provided roster information for one match.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherMatchManifest {
    /// Optional launcher match identifier.
    #[serde(default)]
    pub match_id: Option<String>,
    /// Players authorized to join the match.
    pub players: Vec<LauncherMatchPlayer>,
}

/// Launcher-provided selection and profile information for one player.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherMatchPlayer {
    /// Stable public identifier of the player.
    pub player_public_id: u64,
    /// Team selected for the player.
    pub team: TeamSpec,
    /// Champion selected for the player.
    pub champion_id: ChampionId,
    /// Optional public display name.
    #[serde(default, alias = "display_name")]
    pub display_name: Option<String>,
    /// Optional avatar URL.
    #[serde(default, alias = "avatar_url")]
    pub avatar_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_launcher_manifest_player_metadata() {
        let manifest = serde_json::from_str::<LauncherMatchManifest>(
            r#"{
                "matchId": "match-1",
                "players": [{
                    "playerPublicId": 7,
                    "team": "Light",
                    "championId": 6606,
                    "display_name": "Player One",
                    "avatar_url": "avatars/player-one.png"
                }]
            }"#,
        )
        .expect("launcher manifest should parse");

        assert_eq!(manifest.match_id.as_deref(), Some("match-1"));
        assert_eq!(manifest.players[0].champion_id, ChampionId::LIRA);
        assert_eq!(
            manifest.players[0].display_name.as_deref(),
            Some("Player One")
        );
        assert_eq!(
            manifest.players[0].avatar_url.as_deref(),
            Some("avatars/player-one.png")
        );
    }
}
