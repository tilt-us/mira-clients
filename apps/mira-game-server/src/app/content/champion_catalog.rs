use bevy::prelude::*;
use game_shared::game::auto_attack::{AUTO_ATTACK_COMBO_LENGTH, AutoAttackCombo};
use game_shared::network::{
    AbilitySlot, ChampionCatalogUpdate, ChampionId, NetworkAbilityDamage, NetworkAbilityDefinition,
    NetworkAutoAttackDefinition, NetworkChampionAbilities, NetworkChampionBaseStats,
    NetworkChampionDefinition, NetworkChampionStats,
};
use serde::Deserialize;
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

const DEFAULT_CHAMPION_API_ADDR: &str = "localhost:8084";
const CHAMPION_API_BASE_PATH: &str = "/api/champions";
const CHAMPION_API_TIMEOUT: Duration = Duration::from_secs(5);
const DEVELOPMENT_CHAMPION_CATALOG_SOURCE_ENV: &str = "MIRA_DEVELOPMENT_CHAMPION_CATALOG";
const EMBEDDED_DEVELOPMENT_CHAMPION_CATALOG: &str =
    include_str!("development_champion_catalog.json");
/// Registers server-authoritative champion content for the active catalog source.
pub struct ServerContentPlugin;

impl Plugin for ServerContentPlugin {
    /// Registers Bevy resources, plugins, or systems for the server champion content catalog system.
    fn build(&self, app: &mut App) {
        app.insert_resource(ServerChampionCatalog::load_development_catalog());
    }
}
/// Stores server-authoritative champion definitions used by match simulation.
///
/// - `champions`: Champion definitions keyed by their stable content id.
#[derive(Resource, Debug, Clone)]
pub struct ServerChampionCatalog {
    champions: HashMap<ChampionId, ServerChampionDefinition>,
}

impl ServerChampionCatalog {
    /// Loads the development champion catalog from the selected local snapshot or champion API.
    ///
    /// - A catalog containing the current development champion definition.
    pub fn load_development_catalog() -> Self {
        let api_champions = if uses_embedded_development_catalog() {
            info!("Loaded embedded development champion catalog.");
            load_embedded_development_champion_catalog().unwrap_or_else(|error| {
                panic!("Failed to load embedded development champion catalog: {error}")
            })
        } else {
            load_champion_api_catalog().unwrap_or_else(|error| {
                panic!("Failed to load server champion catalog from API: {error}")
            })
        };
        let mut champions = HashMap::new();
        for champion_id in ChampionId::PROTOTYPE_ROSTER {
            let champion =
                load_champion_definition(champion_id, &api_champions).unwrap_or_else(|error| {
                    panic!(
                        "Failed to load server champion content for {}: {}",
                        champion_id.0, error
                    )
                });
            validate_development_champion(champion_id, &champion).unwrap_or_else(|error| {
                panic!(
                    "Invalid server champion content for {}: {}",
                    champion_id.0, error
                )
            });
            info!(
                "Loaded server champion content: {} ({})",
                champion.display_name, champion_id.0
            );
            champions.insert(champion_id, champion);
        }
        Self { champions }
    }
    /// Builds a network update containing all loaded champion definitions.
    ///
    /// - Serializable champion catalog update for connected clients.
    pub fn catalog_update(&self) -> ChampionCatalogUpdate {
        let mut champions = self
            .champions
            .iter()
            .map(|(champion_id, champion)| NetworkChampionDefinition {
                id: *champion_id,
                name: champion.display_name.clone(),
                stats: NetworkChampionStats {
                    base_stats: NetworkChampionBaseStats {
                        max_health: champion.base_stats.max_health,
                    },
                    auto_attack: (&champion.auto_attack).into(),
                    abilities: NetworkChampionAbilities {
                        q: (&champion.abilities.q).into(),
                        w: (&champion.abilities.w).into(),
                        e: (&champion.abilities.e).into(),
                        r: champion.abilities.r.as_ref().map(Into::into),
                    },
                },
            })
            .collect::<Vec<_>>();
        champions.sort_by_key(|champion| champion.id.0);
        ChampionCatalogUpdate { champions }
    }
    /// Returns an immutable champion definition for the requested id.
    ///
    /// - `champion_id`: Stable champion content id.
    ///
    /// - The matching champion definition when it exists in the loaded catalog.
    pub fn champion(&self, champion_id: ChampionId) -> Option<&ServerChampionDefinition> {
        self.champions.get(&champion_id)
    }
    /// Returns server-authoritative tuning for one champion ability slot.
    ///
    /// - `champion_id`: Stable champion content id.
    /// - `slot`: Ability slot whose tuning should be read.
    ///
    /// - Ability tuning for the requested champion ability when configured.
    pub fn ability(
        &self,
        champion_id: ChampionId,
        slot: AbilitySlot,
    ) -> Option<&ServerAbilityDefinition> {
        self.champion(champion_id)
            .and_then(|champion| champion.ability(slot))
    }

    /// Returns the authoritative basic-attack combo for one champion.
    pub fn auto_attack_combo(&self, champion_id: ChampionId) -> Option<AutoAttackCombo> {
        self.champion(champion_id)
            .map(|champion| champion.auto_attack.combo())
    }

    #[cfg(test)]
    pub(crate) fn embedded_test_catalog() -> Self {
        let api_champions = load_embedded_development_champion_catalog().unwrap();
        Self::from_api_champions(&api_champions).unwrap()
    }

    #[cfg(test)]
    fn from_api_champions(api_champions: &[ChampionApiResponse]) -> Result<Self, String> {
        let mut champions = HashMap::new();
        for champion_id in ChampionId::PROTOTYPE_ROSTER {
            let champion = load_champion_definition(champion_id, api_champions)?;
            validate_development_champion(champion_id, &champion)?;
            champions.insert(champion_id, champion);
        }
        Ok(Self { champions })
    }
}
/// Stores one server-authoritative champion definition.
///
/// - `display_name`: Human-readable champion name used by server logs.
/// - `base_stats`: Server-authoritative base stats for the champion.
/// - `abilities`: Server-authoritative ability tuning for the champion.
#[derive(Debug, Clone)]
pub struct ServerChampionDefinition {
    pub display_name: String,
    pub base_stats: ServerChampionBaseStats,
    pub auto_attack: ServerAutoAttackDefinition,
    pub abilities: ServerChampionAbilities,
}

impl ServerChampionDefinition {
    /// Returns server-authoritative tuning for one ability slot.
    ///
    /// - `slot`: Ability slot whose tuning should be read.
    ///
    /// - Ability tuning for the requested ability when configured.
    pub fn ability(&self, slot: AbilitySlot) -> Option<&ServerAbilityDefinition> {
        match slot {
            AbilitySlot::Q => Some(&self.abilities.q),
            AbilitySlot::W => Some(&self.abilities.w),
            AbilitySlot::E => Some(&self.abilities.e),
            AbilitySlot::R => self.abilities.r.as_ref(),
        }
    }
}
/// Stores server-authoritative champion base stats.
///
/// - `max_health`: Maximum health assigned by the match server.
#[derive(Debug, Clone, Copy)]
pub struct ServerChampionBaseStats {
    pub max_health: f32,
}

/// Stores server-authoritative basic-attack tuning for one champion.
#[derive(Debug, Clone, Copy)]
pub struct ServerAutoAttackDefinition {
    pub base_damage: f32,
    pub attacks_per_second: f32,
    pub combo_damage_multipliers: [f32; AUTO_ATTACK_COMBO_LENGTH],
}

impl ServerAutoAttackDefinition {
    fn combo(self) -> AutoAttackCombo {
        AutoAttackCombo {
            base_damage: self.base_damage,
            combo_length: AUTO_ATTACK_COMBO_LENGTH,
            attacks_per_second: self.attacks_per_second,
            damage_multipliers: self.combo_damage_multipliers,
        }
    }
}
/// Stores server-authoritative ability tuning for a champion.
///
/// - `q`: Tuning for the first basic ability.
/// - `w`: Tuning for the second basic ability.
/// - `e`: Tuning for the third basic ability.
/// - `r`: Optional tuning for the ultimate ability.
#[derive(Debug, Clone)]
pub struct ServerChampionAbilities {
    pub q: ServerAbilityDefinition,
    pub w: ServerAbilityDefinition,
    pub e: ServerAbilityDefinition,
    pub r: Option<ServerAbilityDefinition>,
}
/// Stores server-authoritative ability data for one slot.
///
/// - `damage`: Damage values applied by this ability.
/// - `cooldown_seconds`: Cooldown duration applied by the match server.
/// - `range`: Maximum cast or search range in world units.
/// - `travel_seconds`: Travel duration for projectile-style ability simulations.
/// - `projectile_height`: Height offset used for projectile spawn positions.
/// - `projectile_radius`: Collision radius used by projectile hit tests.
/// - `target_height`: Height offset used for target or landing positions.
/// - `explosion_radius`: Radius used by area damage checks.
/// - `missile_count`: Number of contact missiles spawned by missile-style abilities.
/// - `missile_lifetime_seconds`: Lifetime of contact missiles.
/// - `missile_search_radius`: Search radius used by contact missiles.
/// - `missile_orbit_radius`: Orbit radius used by contact missiles.
/// - `missile_orbit_height`: Orbit height used by contact missiles.
/// - `missile_orbit_speed`: Orbit speed used by contact missiles.
/// - `missile_chase_speed`: Chase speed used by contact missiles.
/// - `missile_radius`: Collision radius used by contact missiles.
#[derive(Debug, Clone)]
pub struct ServerAbilityDefinition {
    pub damage: ServerAbilityDamage,
    pub cooldown_seconds: f32,
    pub range: f32,
    pub travel_seconds: f32,
    pub projectile_height: f32,
    pub projectile_radius: f32,
    pub target_height: f32,
    pub explosion_radius: f32,
    pub missile_count: usize,
    pub missile_lifetime_seconds: f32,
    pub missile_search_radius: f32,
    pub missile_orbit_radius: f32,
    pub missile_orbit_height: f32,
    pub missile_orbit_speed: f32,
    pub missile_chase_speed: f32,
    pub missile_radius: f32,
    pub width: f32,
    pub lifetime_seconds: f32,
    pub target_radius: f32,
    pub damage_per_second: f32,
    pub pull_speed: f32,
    pub move_speed_multiplier: f32,
    pub heal: f32,
    pub stun_seconds: f32,
    pub slow_seconds: f32,
    pub speed_seconds: f32,
    pub damage_multiplier: f32,
    pub small_distance: f32,
    pub medium_distance: f32,
    pub small_damage: f32,
    pub medium_damage: f32,
    pub large_damage: f32,
}
/// Stores server-authoritative damage values used by ability simulations.
///
/// - `direct_hit`: Damage applied by direct projectile or contact hits.
/// - `area`: Damage applied by area explosions or impact zones.
/// - `missile`: Damage applied by individual homing/contact missiles.
#[derive(Debug, Clone, Copy, Default)]
pub struct ServerAbilityDamage {
    pub direct_hit: f32,
    pub area: f32,
    pub missile: f32,
}
/// Represents one champion response from the champion API.
///
/// - `name`: Human-readable champion name used by server logs.
/// - `stats`: Server-authoritative champion stats and ability tuning.
#[derive(Debug, Clone, Deserialize)]
struct ChampionApiResponse {
    name: String,
    stats: NetworkChampionStats,
}

impl From<ChampionApiResponse> for ServerChampionDefinition {
    fn from(value: ChampionApiResponse) -> Self {
        Self {
            display_name: value.name,
            base_stats: ServerChampionBaseStats {
                max_health: value.stats.base_stats.max_health,
            },
            auto_attack: value.stats.auto_attack.into(),
            abilities: ServerChampionAbilities {
                q: value.stats.abilities.q.into(),
                w: value.stats.abilities.w.into(),
                e: value.stats.abilities.e.into(),
                r: value.stats.abilities.r.map(Into::into),
            },
        }
    }
}

impl From<NetworkAutoAttackDefinition> for ServerAutoAttackDefinition {
    fn from(value: NetworkAutoAttackDefinition) -> Self {
        Self {
            base_damage: value.base_damage,
            attacks_per_second: value.attacks_per_second,
            combo_damage_multipliers: value.combo_damage_multipliers,
        }
    }
}

impl From<&ServerAutoAttackDefinition> for NetworkAutoAttackDefinition {
    fn from(value: &ServerAutoAttackDefinition) -> Self {
        Self {
            base_damage: value.base_damage,
            attacks_per_second: value.attacks_per_second,
            combo_damage_multipliers: value.combo_damage_multipliers,
        }
    }
}

impl From<NetworkAbilityDefinition> for ServerAbilityDefinition {
    fn from(value: NetworkAbilityDefinition) -> Self {
        Self {
            damage: value.damage.into(),
            cooldown_seconds: value.cooldown_seconds,
            range: value.range,
            travel_seconds: value.travel_seconds,
            projectile_height: value.projectile_height,
            projectile_radius: value.projectile_radius,
            target_height: value.target_height,
            explosion_radius: value.explosion_radius,
            missile_count: value.missile_count,
            missile_lifetime_seconds: value.missile_lifetime_seconds,
            missile_search_radius: value.missile_search_radius,
            missile_orbit_radius: value.missile_orbit_radius,
            missile_orbit_height: value.missile_orbit_height,
            missile_orbit_speed: value.missile_orbit_speed,
            missile_chase_speed: value.missile_chase_speed,
            missile_radius: value.missile_radius,
            width: value.width,
            lifetime_seconds: value.lifetime_seconds,
            target_radius: value.target_radius,
            damage_per_second: value.damage_per_second,
            pull_speed: value.pull_speed,
            move_speed_multiplier: value.move_speed_multiplier,
            heal: value.heal,
            stun_seconds: value.stun_seconds,
            slow_seconds: value.slow_seconds,
            speed_seconds: value.speed_seconds,
            damage_multiplier: value.damage_multiplier,
            small_distance: value.small_distance,
            medium_distance: value.medium_distance,
            small_damage: value.small_damage,
            medium_damage: value.medium_damage,
            large_damage: value.large_damage,
        }
    }
}

impl From<&ServerAbilityDefinition> for NetworkAbilityDefinition {
    fn from(value: &ServerAbilityDefinition) -> Self {
        Self {
            damage: (&value.damage).into(),
            cooldown_seconds: value.cooldown_seconds,
            range: value.range,
            travel_seconds: value.travel_seconds,
            projectile_height: value.projectile_height,
            projectile_radius: value.projectile_radius,
            target_height: value.target_height,
            explosion_radius: value.explosion_radius,
            missile_count: value.missile_count,
            missile_lifetime_seconds: value.missile_lifetime_seconds,
            missile_search_radius: value.missile_search_radius,
            missile_orbit_radius: value.missile_orbit_radius,
            missile_orbit_height: value.missile_orbit_height,
            missile_orbit_speed: value.missile_orbit_speed,
            missile_chase_speed: value.missile_chase_speed,
            missile_radius: value.missile_radius,
            width: value.width,
            lifetime_seconds: value.lifetime_seconds,
            target_radius: value.target_radius,
            damage_per_second: value.damage_per_second,
            pull_speed: value.pull_speed,
            move_speed_multiplier: value.move_speed_multiplier,
            heal: value.heal,
            stun_seconds: value.stun_seconds,
            slow_seconds: value.slow_seconds,
            speed_seconds: value.speed_seconds,
            damage_multiplier: value.damage_multiplier,
            small_distance: value.small_distance,
            medium_distance: value.medium_distance,
            small_damage: value.small_damage,
            medium_damage: value.medium_damage,
            large_damage: value.large_damage,
        }
    }
}

impl From<NetworkAbilityDamage> for ServerAbilityDamage {
    fn from(value: NetworkAbilityDamage) -> Self {
        Self {
            direct_hit: value.direct_hit,
            area: value.area,
            missile: value.missile,
        }
    }
}

impl From<&ServerAbilityDamage> for NetworkAbilityDamage {
    fn from(value: &ServerAbilityDamage) -> Self {
        Self {
            direct_hit: value.direct_hit,
            area: value.area,
            missile: value.missile,
        }
    }
}

/// Loads a supported prototype champion from the cached API catalog or detail endpoint.
fn load_champion_definition(
    champion_id: ChampionId,
    api_champions: &[ChampionApiResponse],
) -> Result<ServerChampionDefinition, String> {
    let champion_name = champion_id
        .display_name()
        .ok_or_else(|| format!("Unsupported prototype champion id {}", champion_id.0))?;
    let parsed = api_champions
        .iter()
        .find(|champion| champion.name.eq_ignore_ascii_case(champion_name))
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| load_champion_api_definition(champion_name))?;

    Ok(parsed.into())
}
/// Loads all champion definitions from the champion API list endpoint.
///
/// - Parsed champion API responses or an HTTP/parse error string.
fn load_champion_api_catalog() -> Result<Vec<ChampionApiResponse>, String> {
    let raw = http_get(CHAMPION_API_BASE_PATH)?;
    serde_json::from_str::<Vec<ChampionApiResponse>>(&raw)
        .map_err(|error| format!("Failed to parse champion catalog API response: {error}"))
}

/// Returns whether the local development catalog snapshot was explicitly selected.
fn uses_embedded_development_catalog() -> bool {
    std::env::var(DEVELOPMENT_CHAMPION_CATALOG_SOURCE_ENV)
        .is_ok_and(|source| source.eq_ignore_ascii_case("embedded"))
}

/// Loads the prototype champion snapshot used by local development recipes.
fn load_embedded_development_champion_catalog() -> Result<Vec<ChampionApiResponse>, String> {
    serde_json::from_str(EMBEDDED_DEVELOPMENT_CHAMPION_CATALOG)
        .map_err(|error| format!("Failed to parse embedded champion catalog: {error}"))
}
/// Loads one champion definition from the champion API detail endpoint.
///
/// - `champion_name`: Champion name appended to `/api/champions`.
///
/// - Parsed champion API response or an HTTP/parse error string.
fn load_champion_api_definition(champion_name: &str) -> Result<ChampionApiResponse, String> {
    let raw = http_get(&format!("{CHAMPION_API_BASE_PATH}/{champion_name}"))?;
    serde_json::from_str::<ChampionApiResponse>(&raw).map_err(|error| {
        format!("Failed to parse champion API response for {champion_name}: {error}")
    })
}
/// Performs a simple blocking HTTP GET against the local champion API.
///
/// - `path`: Absolute HTTP path to request from `MIRA_CHAMPION_API_ADDR`.
///
/// - Response body when the request returns a successful status.
fn http_get(path: &str) -> Result<String, String> {
    let api_addr = champion_api_addr();
    let mut stream = TcpStream::connect(api_addr.as_str())
        .map_err(|error| format!("Failed to connect to champion API at {api_addr}: {error}"))?;
    stream
        .set_read_timeout(Some(CHAMPION_API_TIMEOUT))
        .map_err(|error| format!("Failed to set champion API read timeout: {error}"))?;
    stream
        .set_write_timeout(Some(CHAMPION_API_TIMEOUT))
        .map_err(|error| format!("Failed to set champion API write timeout: {error}"))?;

    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {api_addr}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    )
    .map_err(|error| format!("Failed to write champion API request for {path}: {error}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| format!("Failed to read champion API response for {path}: {error}"))?;

    parse_http_response(&response, path)
}

/// Parses a successful champion API response and decodes its HTTP body.
fn parse_http_response(response: &str, path: &str) -> Result<String, String> {
    let (headers, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| format!("Champion API response for {path} did not contain HTTP headers"))?;
    let status = headers
        .lines()
        .next()
        .ok_or_else(|| format!("Champion API response for {path} was empty"))?;
    if !status.contains(" 200 ") {
        return Err(format!(
            "Champion API request for {path} failed with status `{status}`"
        ));
    }

    if has_chunked_transfer_encoding(headers) {
        decode_chunked_body(body).map_err(|error| {
            format!("Failed to decode chunked champion API response for {path}: {error}")
        })
    } else {
        Ok(body.to_string())
    }
}

/// Returns whether an HTTP response uses chunked transfer encoding.
fn has_chunked_transfer_encoding(headers: &str) -> bool {
    headers.lines().skip(1).any(|line| {
        let Some((name, value)) = line.split_once(':') else {
            return false;
        };

        name.trim().eq_ignore_ascii_case("transfer-encoding")
            && value
                .split(',')
                .any(|encoding| encoding.trim().eq_ignore_ascii_case("chunked"))
    })
}

/// Decodes an HTTP/1.1 chunked response body into its UTF-8 payload.
fn decode_chunked_body(body: &str) -> Result<String, String> {
    let bytes = body.as_bytes();
    let mut offset = 0;
    let mut decoded = Vec::with_capacity(bytes.len());

    loop {
        let line_end = bytes[offset..]
            .windows(2)
            .position(|window| window == b"\r\n")
            .map(|position| offset + position)
            .ok_or_else(|| "chunk length was not terminated".to_string())?;
        let chunk_size_text = std::str::from_utf8(&bytes[offset..line_end])
            .map_err(|_| "chunk length was not valid UTF-8".to_string())?;
        let chunk_size = chunk_size_text.split(';').next().unwrap_or_default().trim();
        let chunk_size = usize::from_str_radix(chunk_size, 16)
            .map_err(|_| format!("invalid chunk length `{chunk_size_text}`"))?;
        offset = line_end + 2;

        if chunk_size == 0 {
            return String::from_utf8(decoded)
                .map_err(|_| "chunked response body was not valid UTF-8".to_string());
        }

        let chunk_end = offset
            .checked_add(chunk_size)
            .ok_or_else(|| "chunk length overflowed".to_string())?;
        if chunk_end + 2 > bytes.len() {
            return Err("chunk body was truncated".to_string());
        }
        if &bytes[chunk_end..chunk_end + 2] != b"\r\n" {
            return Err("chunk body was not terminated".to_string());
        }

        decoded.extend_from_slice(&bytes[offset..chunk_end]);
        offset = chunk_end + 2;
    }
}
fn champion_api_addr() -> String {
    std::env::var("MIRA_CHAMPION_API_ADDR")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_CHAMPION_API_ADDR.to_string())
}
/// Validates the current development champion content before the match server starts.
///
/// - `champion_id`: Stable champion content id used for error messages.
/// - `champion`: Parsed server-authoritative champion definition.
///
/// - `Ok(())` when the champion contains the required server-side tuning values.
fn validate_development_champion(
    champion_id: ChampionId,
    champion: &ServerChampionDefinition,
) -> Result<(), String> {
    require_positive(
        champion.base_stats.max_health,
        champion_id,
        "base_stats.max_health",
    )?;
    require_positive(
        champion.auto_attack.base_damage,
        champion_id,
        "auto_attack.base_damage",
    )?;
    require_positive(
        champion.auto_attack.attacks_per_second,
        champion_id,
        "auto_attack.attacks_per_second",
    )?;
    for (stage, multiplier) in champion
        .auto_attack
        .combo_damage_multipliers
        .iter()
        .copied()
        .enumerate()
    {
        require_positive(
            multiplier,
            champion_id,
            format!("auto_attack.combo_damage_multipliers[{stage}]"),
        )?;
    }
    validate_basic_ability(champion_id, AbilitySlot::Q, &champion.abilities.q)?;
    validate_basic_ability(champion_id, AbilitySlot::W, &champion.abilities.w)?;
    validate_basic_ability(champion_id, AbilitySlot::E, &champion.abilities.e)?;

    Ok(())
}
/// Validates that an ability has the minimum server-authoritative tuning.
///
/// - `champion_id`: Stable champion content id used for error messages.
/// - `slot`: Ability slot used for error messages.
/// - `ability`: Server-authoritative ability definition to validate.
///
/// - `Ok(())` when the ability has valid minimum required values.
fn validate_basic_ability(
    champion_id: ChampionId,
    slot: AbilitySlot,
    ability: &ServerAbilityDefinition,
) -> Result<(), String> {
    require_positive(
        ability.cooldown_seconds,
        champion_id,
        slot_field(slot, "cooldown_seconds"),
    )
}
/// Validates that a server content value is strictly positive.
///
/// - `value`: Numeric content value to validate.
/// - `champion_id`: Stable champion content id used for error messages.
/// - `field`: Field path used for error messages.
///
/// - `Ok(())` when the value is finite and greater than zero.
fn require_positive(
    value: f32,
    champion_id: ChampionId,
    field: impl AsRef<str>,
) -> Result<(), String> {
    if value.is_finite() && value > 0.0 {
        return Ok(());
    }

    Err(format!(
        "champion {} field `{}` must be positive, got {}",
        champion_id.0,
        field.as_ref(),
        value
    ))
}
/// Builds an ability field path for validation errors.
///
/// - `slot`: Ability slot used as the path prefix.
/// - `field`: Ability field name.
///
/// - Human-readable ability field path.
fn slot_field(slot: AbilitySlot, field: &str) -> String {
    format!("{:?}.{}", slot, field)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_chunked_champion_api_responses() {
        let response = concat!(
            "HTTP/1.1 200 OK\r\n",
            "Content-Type: application/json\r\n",
            "Transfer-Encoding: chunked\r\n",
            "\r\n",
            "2\r\n[1\r\n",
            "3\r\n,2]\r\n",
            "0\r\n\r\n",
        );

        assert_eq!(
            parse_http_response(response, CHAMPION_API_BASE_PATH).unwrap(),
            "[1,2]"
        );
    }

    #[test]
    fn keeps_content_length_champion_api_responses_unchanged() {
        let response = concat!(
            "HTTP/1.1 200 OK\r\n",
            "Content-Type: application/json\r\n",
            "Content-Length: 5\r\n",
            "\r\n",
            "[1,2]",
        );

        assert_eq!(
            parse_http_response(response, CHAMPION_API_BASE_PATH).unwrap(),
            "[1,2]"
        );
    }

    #[test]
    fn rejects_truncated_chunked_champion_api_responses() {
        let response = concat!(
            "HTTP/1.1 200 OK\r\n",
            "Transfer-Encoding: chunked\r\n",
            "\r\n",
            "4\r\n[1]",
        );

        assert!(parse_http_response(response, CHAMPION_API_BASE_PATH).is_err());
    }

    #[test]
    fn rejects_unsupported_champion_ids() {
        let error = load_champion_definition(ChampionId(9_999), &[]).unwrap_err();

        assert_eq!(error, "Unsupported prototype champion id 9999");
    }

    #[test]
    fn loads_every_prototype_champion_from_the_embedded_development_catalog() {
        let champions = load_embedded_development_champion_catalog().unwrap();

        for champion_id in ChampionId::PROTOTYPE_ROSTER {
            let champion = load_champion_definition(champion_id, &champions).unwrap();
            validate_development_champion(champion_id, &champion).unwrap();
        }
    }
}
