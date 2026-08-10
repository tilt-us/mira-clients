use bevy::prelude::*;
use game_shared::game::player::{non_empty_string, public_display_name};
use game_shared::game::team::TeamSpec;
use game_shared::network::{ChampionId, LauncherMatchManifest};
use std::collections::HashMap;

const MATCH_MANIFEST_ENV: &str = "MIRA_MATCH_MANIFEST_JSON";

/// Authoritative launcher roster for the current server match.
#[derive(Resource, Debug, Clone, Default)]
pub struct ServerMatchManifest {
    pub match_id: Option<String>,
    players: HashMap<u64, ServerMatchPlayer>,
}

/// A player authorized by the launcher to join the current match.
#[derive(Debug, Clone)]
pub struct ServerMatchPlayer {
    pub team: TeamSpec,
    pub champion: ChampionId,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

impl ServerMatchManifest {
    /// Loads the launcher manifest from `MIRA_MATCH_MANIFEST_JSON`.
    pub fn load_from_environment() -> Self {
        let Ok(raw_manifest) = std::env::var(MATCH_MANIFEST_ENV) else {
            return Self::default();
        };

        let manifest = serde_json::from_str::<LauncherMatchManifest>(&raw_manifest)
            .unwrap_or_else(|error| panic!("Invalid {}: {}", MATCH_MANIFEST_ENV, error));
        let match_id = manifest
            .match_id
            .unwrap_or_else(|| panic!("Invalid {}: missing matchId", MATCH_MANIFEST_ENV));
        let players = manifest
            .players
            .into_iter()
            .map(|player| {
                (
                    player.player_public_id,
                    ServerMatchPlayer {
                        team: player.team,
                        champion: player.champion_id,
                        display_name: player.display_name.as_deref().and_then(public_display_name),
                        avatar_url: player.avatar_url.as_deref().and_then(non_empty_string),
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        Self {
            match_id: Some(match_id),
            players,
        }
    }

    /// Returns whether the launcher provided an enforced player roster.
    pub fn is_enforced(&self) -> bool {
        !self.players.is_empty()
    }

    /// Returns the authorized player with the given public id.
    pub fn player(&self, player_public_id: u64) -> Option<ServerMatchPlayer> {
        self.players.get(&player_public_id).cloned()
    }

    /// Returns the public ids authorized for this match.
    pub fn player_ids(&self) -> Vec<u64> {
        self.players.keys().copied().collect()
    }

    /// Returns all authorized players and their public ids.
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

    #[test]
    fn parses_player_profile_fields_from_manifest() {
        let manifest = serde_json::from_str::<LauncherMatchManifest>(
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
