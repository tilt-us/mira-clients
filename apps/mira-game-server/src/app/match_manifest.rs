use bevy::prelude::*;
use game_shared::game::team::TeamSpec;
use game_shared::network::ChampionId;
use serde::Deserialize;
use std::collections::HashMap;

const MATCH_MANIFEST_ENV: &str = "MIRA_MATCH_MANIFEST_JSON";

#[derive(Resource, Debug, Clone, Default)]
pub struct ServerMatchManifest {
    pub match_id: Option<String>,
    players: HashMap<u64, ServerMatchPlayer>,
}

#[derive(Debug, Clone)]
pub struct ServerMatchPlayer {
    pub team: TeamSpec,
    pub champion: ChampionId,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatchManifestFile {
    match_id: String,
    players: Vec<MatchManifestPlayerFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatchManifestPlayerFile {
    player_public_id: u64,
    team: TeamSpec,
    champion_id: u32,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    avatar_url: Option<String>,
}

impl ServerMatchManifest {
    pub fn load_from_environment() -> Self {
        let Ok(raw_manifest) = std::env::var(MATCH_MANIFEST_ENV) else {
            info!(
                "No {} set; dedicated server accepts development clients.",
                MATCH_MANIFEST_ENV
            );
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
                        display_name: player.display_name,
                        avatar_url: player.avatar_url,
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        info!(
            "Loaded match manifest {} with {} allowed players.",
            manifest.match_id,
            players.len()
        );
        Self {
            match_id: Some(manifest.match_id),
            players,
        }
    }

    pub fn is_enforced(&self) -> bool {
        !self.players.is_empty()
    }

    pub fn player(&self, player_public_id: u64) -> Option<ServerMatchPlayer> {
        self.players.get(&player_public_id).cloned()
    }

    pub fn player_ids(&self) -> Vec<u64> {
        self.players.keys().copied().collect()
    }

    pub fn players(&self) -> Vec<(u64, ServerMatchPlayer)> {
        self.players
            .iter()
            .map(|(player_id, player)| (*player_id, player.clone()))
            .collect()
    }
}
