use bevy::prelude::*;
use game_shared::game::player::{non_empty_string, public_display_name};
use game_shared::game::team::TeamSpec;
use game_shared::network::ChampionId;
use serde::Deserialize;
use std::collections::HashMap;

const MATCH_MANIFEST_ENV: &str = "MIRA_MATCH_MANIFEST_JSON";

/// Stores Server Match Manifest data used by the server match manifest system.
#[derive(Resource, Debug, Clone, Default)]
pub struct ServerMatchManifest {
    pub match_id: Option<String>,
    players: HashMap<u64, ServerMatchPlayer>,
}

/// Stores Server Match Player data used by the server match manifest system.
#[derive(Debug, Clone)]
pub struct ServerMatchPlayer {
    pub team: TeamSpec,
    pub champion: ChampionId,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

/// Stores Match Manifest File data used by the server match manifest system.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatchManifestFile {
    match_id: String,
    players: Vec<MatchManifestPlayerFile>,
}

/// Stores Match Manifest Player File data used by the server match manifest system.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatchManifestPlayerFile {
    player_public_id: u64,
    team: TeamSpec,
    champion_id: u32,
    #[serde(default, rename = "champion")]
    _champion: Option<String>,
    #[serde(default, alias = "display_name")]
    display_name: Option<String>,
    #[serde(default, alias = "avatar_url")]
    avatar_url: Option<String>,
}

impl ServerMatchManifest {
    /// Runs the load from environment step for the server match manifest system.
    pub fn load_from_environment() -> Self {
        let Ok(raw_manifest) = std::env::var(MATCH_MANIFEST_ENV) else {
            return Self::default();
        };

        let manifest = serde_json::from_str::<MatchManifestFile>(&raw_manifest)
            .unwrap_or_else(|error| panic!("Invalid {}: {}", MATCH_MANIFEST_ENV, error));
        let players = manifest
            .players
            .into_iter()
            .map(|player| {
                (
                    player.player_public_id,
                    ServerMatchPlayer {
                        team: player.team,
                        champion: ChampionId(player.champion_id),
                        display_name: player.display_name.as_deref().and_then(public_display_name),
                        avatar_url: player.avatar_url.as_deref().and_then(non_empty_string),
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        Self {
            match_id: Some(manifest.match_id),
            players,
        }
    }

    /// Runs the is enforced step for the server match manifest system.
    pub fn is_enforced(&self) -> bool {
        !self.players.is_empty()
    }

    /// Runs the player step for the server match manifest system.
    pub fn player(&self, player_public_id: u64) -> Option<ServerMatchPlayer> {
        self.players.get(&player_public_id).cloned()
    }

    /// Runs the player ids step for the server match manifest system.
    pub fn player_ids(&self) -> Vec<u64> {
        self.players.keys().copied().collect()
    }

    /// Runs the players step for the server match manifest system.
    pub fn players(&self) -> Vec<(u64, ServerMatchPlayer)> {
        self.players
            .iter()
            .map(|(player_id, player)| (*player_id, player.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies parses player profile fields from manifest behavior for the server match manifest system.
    #[test]
    fn parses_player_profile_fields_from_manifest() {
        let manifest = serde_json::from_str::<MatchManifestFile>(
            r#"{
                "matchId": "match-1",
                "players": [
                    {
                        "playerPublicId": 7,
                        "team": "Light",
                        "championId": 6606,
                        "displayName": "Exepta Mustermann",
                        "avatarUrl": "https://example.test/avatar.png"
                    },
                    {
                        "playerPublicId": 8,
                        "team": "Dark",
                        "championId": 6607,
                        "display_name": "Other Player",
                        "avatar_url": "avatars/other.png"
                    }
                ]
            }"#,
        )
        .expect("manifest should parse");

        let players = manifest
            .players
            .into_iter()
            .map(|player| {
                (
                    player.player_public_id,
                    (
                        player.display_name.as_deref().and_then(public_display_name),
                        player.avatar_url.as_deref().and_then(non_empty_string),
                    ),
                )
            })
            .collect::<HashMap<_, _>>();

        assert_eq!(players[&7].0.as_deref(), Some("Exepta"));
        assert_eq!(
            players[&7].1.as_deref(),
            Some("https://example.test/avatar.png")
        );
        assert_eq!(players[&8].0.as_deref(), Some("Other"));
        assert_eq!(players[&8].1.as_deref(), Some("avatars/other.png"));
    }
}
