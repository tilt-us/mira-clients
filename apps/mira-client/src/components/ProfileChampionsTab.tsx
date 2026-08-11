import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type MouseEvent,
} from "react";
import { Flame, Sword } from "lucide-react";
import { uiWallpaperUrl } from "../uiAssets";
import { getChampionCatalog, setOwnedChampion } from "../api/client";
import type { Translate } from "../types/ui";

type ChampionOwnershipStatus = "owned" | "weekly" | "unowned";
type ChampionRankSort = "highest" | "lowest";
type ChampionCategoryId =
  | "assassin"
  | "tank"
  | "fighter"
  | "guardian"
  | "caster"
  | "mage";
type ChampionRadarStatId = "damage" | "utility" | "control" | "mobility" | "defense";
type ChampionScalingStat = "ad" | "ap";

type ChampionAbility = {
  cooldown: number;
  damageType: ChampionScalingStat;
  directDamage: number;
  directScaling: number;
  explosionDamage: number;
  explosionScaling: number;
  name: string;
  slot: string;
};

type UserPageChampion = {
  categories: ChampionCategoryId[];
  id: string;
  name: string;
  ownershipStatus: ChampionOwnershipStatus;
  rank: number;
  serverStats: {
    armor: number;
    attackDamage: number;
    cooldown: number;
    control: number;
    health: number;
    healthRegen: number;
    manaRegen: number;
    radar: Record<ChampionRadarStatId, number>;
    resistance: number;
  };
  abilities: ChampionAbility[];
  wallpaper: string;
};

type ChampionFocusState = {
  champion: UserPageChampion;
  closing?: boolean;
  startLeft: number;
  startTop: number;
};

type ProfileChampionsTabProps = {
  backSignal: number;
  onFocusChange: (focused: boolean) => void;
  t: Translate;
  userId?: number;
};

const championOwnershipFilterOptions: Array<{
  id: ChampionOwnershipStatus;
  labelKey: string;
}> = [
  { id: "owned", labelKey: "profile-champions-owned" },
  { id: "weekly", labelKey: "profile-champions-weekly" },
  { id: "unowned", labelKey: "profile-champions-unowned" },
];

const championCategoryFilterOptions: Array<{ id: ChampionCategoryId; labelKey: string }> = [
  { id: "assassin", labelKey: "profile-champions-category-assassin" },
  { id: "tank", labelKey: "profile-champions-category-tank" },
  { id: "fighter", labelKey: "profile-champions-category-fighter" },
  { id: "guardian", labelKey: "profile-champions-category-guardian" },
  { id: "caster", labelKey: "profile-champions-category-caster" },
  { id: "mage", labelKey: "profile-champions-category-mage" },
];

const championRadarStats: Array<{ id: ChampionRadarStatId; labelKey: string }> = [
  { id: "damage", labelKey: "profile-champions-radar-damage" },
  { id: "utility", labelKey: "profile-champions-radar-utility" },
  { id: "control", labelKey: "profile-champions-radar-control" },
  { id: "mobility", labelKey: "profile-champions-radar-mobility" },
  { id: "defense", labelKey: "profile-champions-radar-defense" },
];

const championDefaultPrice = {
  gems: 900,
  tuc: 250,
};

const championSynergyByKey: Record<string, string[]> = {
  ignara: ["Yuna", "Sophia"],
  lira: ["Yuna", "Ignara"],
  sophia: ["Ignara", "Yuna"],
  yuna: ["Lira", "Ignara"],
};

const fallbackUserPageChampions: UserPageChampion[] = [
  {
    categories: ["fighter"],
    id: "ignara",
    name: "Ignara",
    ownershipStatus: "unowned",
    rank: 0,
    abilities: [
      {
        cooldown: 7.5,
        damageType: "ap",
        directDamage: 38,
        directScaling: 18,
        explosionDamage: 62,
        explosionScaling: 24,
        name: "Flare Lance",
        slot: "Q",
      },
      {
        cooldown: 9.0,
        damageType: "ap",
        directDamage: 30,
        directScaling: 12,
        explosionDamage: 70,
        explosionScaling: 28,
        name: "Molten Core",
        slot: "W",
      },
      {
        cooldown: 11.2,
        damageType: "ap",
        directDamage: 26,
        directScaling: 14,
        explosionDamage: 82,
        explosionScaling: 32,
        name: "Inferno Roll",
        slot: "E",
      },
    ],
    serverStats: {
      armor: 36,
      attackDamage: 54,
      cooldown: 7.6,
      control: 42,
      health: 620,
      healthRegen: 1.8,
      manaRegen: 1.2,
      radar: { damage: 86, utility: 44, control: 68, mobility: 72, defense: 52 },
      resistance: 31,
    },
    wallpaper: uiWallpaperUrl("ignara"),
  },
  {
    categories: ["assassin"],
    id: "lira",
    name: "Lira",
    ownershipStatus: "unowned",
    rank: 0,
    abilities: [
      {
        cooldown: 6.8,
        damageType: "ad",
        directDamage: 35,
        directScaling: 15,
        explosionDamage: 55,
        explosionScaling: 20,
        name: "Moonshot",
        slot: "Q",
      },
      {
        cooldown: 8.2,
        damageType: "ad",
        directDamage: 28,
        directScaling: 12,
        explosionDamage: 48,
        explosionScaling: 18,
        name: "Blade Bloom",
        slot: "W",
      },
      {
        cooldown: 10.0,
        damageType: "ad",
        directDamage: 42,
        directScaling: 18,
        explosionDamage: 34,
        explosionScaling: 14,
        name: "Silver Drift",
        slot: "E",
      },
    ],
    serverStats: {
      armor: 28,
      attackDamage: 62,
      cooldown: 6.8,
      control: 24,
      health: 560,
      healthRegen: 1.4,
      manaRegen: 1.5,
      radar: { damage: 82, utility: 56, control: 42, mobility: 78, defense: 38 },
      resistance: 26,
    },
    wallpaper: uiWallpaperUrl("lira"),
  },
  {
    categories: ["mage"],
    id: "sophia",
    name: "Sophia",
    ownershipStatus: "unowned",
    rank: 0,
    abilities: [
      {
        cooldown: 6.4,
        damageType: "ap",
        directDamage: 32,
        directScaling: 22,
        explosionDamage: 46,
        explosionScaling: 26,
        name: "Star Thread",
        slot: "Q",
      },
      {
        cooldown: 7.6,
        damageType: "ap",
        directDamage: 24,
        directScaling: 16,
        explosionDamage: 42,
        explosionScaling: 30,
        name: "Grace Field",
        slot: "W",
      },
      {
        cooldown: 9.4,
        damageType: "ap",
        directDamage: 18,
        directScaling: 12,
        explosionDamage: 64,
        explosionScaling: 34,
        name: "Astral Bloom",
        slot: "E",
      },
    ],
    serverStats: {
      armor: 24,
      attackDamage: 46,
      cooldown: 6.4,
      control: 36,
      health: 540,
      healthRegen: 1.2,
      manaRegen: 2.1,
      radar: { damage: 76, utility: 88, control: 58, mobility: 38, defense: 46 },
      resistance: 34,
    },
    wallpaper: uiWallpaperUrl("sophia"),
  },
  {
    categories: ["guardian"],
    id: "yuna",
    name: "Yuna",
    ownershipStatus: "unowned",
    rank: 0,
    abilities: [
      {
        cooldown: 8.1,
        damageType: "ap",
        directDamage: 26,
        directScaling: 10,
        explosionDamage: 44,
        explosionScaling: 16,
        name: "Ward Pulse",
        slot: "Q",
      },
      {
        cooldown: 10.4,
        damageType: "ad",
        directDamage: 30,
        directScaling: 14,
        explosionDamage: 38,
        explosionScaling: 18,
        name: "Guard Break",
        slot: "W",
      },
      {
        cooldown: 12.0,
        damageType: "ap",
        directDamage: 20,
        directScaling: 8,
        explosionDamage: 58,
        explosionScaling: 18,
        name: "Sanctum Wave",
        slot: "E",
      },
    ],
    serverStats: {
      armor: 42,
      attackDamage: 50,
      cooldown: 8.1,
      control: 54,
      health: 690,
      healthRegen: 2.2,
      manaRegen: 1.1,
      radar: { damage: 52, utility: 70, control: 76, mobility: 54, defense: 86 },
      resistance: 38,
    },
    wallpaper: uiWallpaperUrl("yuna"),
  },
];

type ApiChampionRecord = Record<string, unknown>;

const fallbackChampionByKey = new Map(
  fallbackUserPageChampions.flatMap((champion) => [
    [normalizeChampionKey(champion.id), champion],
    [normalizeChampionKey(champion.name), champion],
  ]),
);

function normalizeChampionKey(value: unknown) {
  return String(value ?? "").trim().toLowerCase();
}

function isSameUserPageChampion(left: UserPageChampion, right: UserPageChampion) {
  const leftKeys = [left.id, left.name].map(normalizeChampionKey).filter(Boolean);
  const rightKeys = new Set(
    [right.id, right.name].map(normalizeChampionKey).filter(Boolean),
  );

  return leftKeys.some((key) => rightKeys.has(key));
}

function isRecord(value: unknown): value is ApiChampionRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function getRecord(value: unknown, key: string) {
  if (!isRecord(value)) {
    return undefined;
  }

  return value[key];
}

function getStringValue(record: ApiChampionRecord, keys: string[]) {
  for (const key of keys) {
    const value = record[key];

    if (typeof value === "string" && value.trim()) {
      return value.trim();
    }

    if (typeof value === "number" && Number.isFinite(value)) {
      return String(value);
    }
  }

  return undefined;
}

function getNumberValue(record: unknown, keys: string[]) {
  if (!isRecord(record)) {
    return undefined;
  }

  for (const key of keys) {
    const value = record[key];

    if (typeof value === "number" && Number.isFinite(value)) {
      return value;
    }

    if (typeof value === "string") {
      const parsedValue = Number(value);

      if (Number.isFinite(parsedValue)) {
        return parsedValue;
      }
    }
  }

  return undefined;
}

function getChampionCatalogItems(response: unknown): ApiChampionRecord[] {
  const value = isRecord(response) && "data" in response ? response.data : response;

  if (Array.isArray(value)) {
    return value.filter(isRecord);
  }

  if (!isRecord(value)) {
    return [];
  }

  for (const key of ["champions", "items", "content", "results"]) {
    const items = value[key];

    if (Array.isArray(items)) {
      return items.filter(isRecord);
    }
  }

  return [];
}

function getApiChampionKey(champion: ApiChampionRecord): string {
  const nestedChampion = champion.champion;

  if (isRecord(nestedChampion)) {
    const nestedKey: string = getApiChampionKey(nestedChampion);

    if (nestedKey) {
      return nestedKey;
    }
  }

  const name = getStringValue(champion, [
    "name",
    "displayName",
    "display_name",
    "fullName",
    "full_name",
    "localizedName",
    "localized_name",
    "id",
    "champion",
    "championId",
    "champion_id",
    "championName",
    "champion_name",
  ]);

  return normalizeChampionKey(name);
}

function getApiChampionKeys(champion: ApiChampionRecord): string[] {
  const nestedChampion = champion.champion;
  const keys = [
    getApiChampionKey(champion),
    getStringValue(champion, ["id", "_id"]),
    getStringValue(champion, ["name", "displayName", "display_name"]),
    getStringValue(champion, ["champion", "championId", "champion_id", "championName", "champion_name"]),
  ];

  if (isRecord(nestedChampion)) {
    keys.push(getApiChampionKey(nestedChampion));
    keys.push(getStringValue(nestedChampion, ["id", "_id"]));
    keys.push(getStringValue(nestedChampion, ["name", "displayName", "display_name"]));
  }

  return Array.from(new Set(keys.map(normalizeChampionKey).filter(Boolean)));
}

function getApiChampionName(champion: ApiChampionRecord, fallback?: UserPageChampion) {
  return (
    getStringValue(champion, ["name", "displayName", "display_name", "fullName", "full_name"]) ??
    fallback?.name ??
    "Champion"
  );
}

function normalizeCategoryId(value: unknown): ChampionCategoryId | undefined {
  const normalizedValue = normalizeChampionKey(value).replace(/[^a-z]/g, "");

  if (normalizedValue === "assassin" || normalizedValue === "assasin") {
    return "assassin";
  }

  if (normalizedValue === "tank") {
    return "tank";
  }

  if (normalizedValue === "fighter" || normalizedValue === "kaempfer" || normalizedValue === "kampfer") {
    return "fighter";
  }

  if (normalizedValue === "guardian" || normalizedValue === "waechter" || normalizedValue === "wachter") {
    return "guardian";
  }

  if (normalizedValue === "caster" || normalizedValue === "wirker") {
    return "caster";
  }

  if (normalizedValue === "mage") {
    return "mage";
  }

  return undefined;
}

function getChampionCategories(champion: ApiChampionRecord, fallback: UserPageChampion) {
  const sourceCategories = champion.categories ?? champion.category ?? champion.role ?? champion.roles;
  const rawCategories = Array.isArray(sourceCategories) ? sourceCategories : [sourceCategories];
  const categories = rawCategories
    .map(normalizeCategoryId)
    .filter((category): category is ChampionCategoryId => Boolean(category));

  return categories.length > 0 ? categories : fallback.categories;
}

function getNestedStats(champion: ApiChampionRecord) {
  const stats = getRecord(champion, "stats") ?? getRecord(champion, "serverStats");
  const baseStats = getRecord(stats, "baseStats") ?? getRecord(stats, "base_stats");

  return { baseStats, stats };
}

function getChampionServerStats(champion: ApiChampionRecord, fallback: UserPageChampion) {
  const { baseStats, stats } = getNestedStats(champion);
  const source = isRecord(baseStats) ? baseStats : stats;
  const radar = isRecord(getRecord(stats, "radar"))
    ? getRecord(stats, "radar")
    : getRecord(champion, "radar");

  return {
    armor: getNumberValue(source, ["armor", "armour"]) ?? fallback.serverStats.armor,
    attackDamage:
      getNumberValue(source, ["attackDamage", "attack_damage", "ad"]) ??
      fallback.serverStats.attackDamage,
    cooldown:
      getNumberValue(source, ["cooldown", "avgCooldown", "averageCooldown", "cooldown_seconds"]) ??
      fallback.serverStats.cooldown,
    control: getNumberValue(source, ["control", "crowdControl", "crowd_control"]) ?? fallback.serverStats.control,
    health:
      getNumberValue(source, ["health", "maxHealth", "max_health", "hp"]) ??
      fallback.serverStats.health,
    healthRegen:
      getNumberValue(source, ["healthRegen", "health_regen", "healthRegenPerSecond"]) ??
      fallback.serverStats.healthRegen,
    manaRegen:
      getNumberValue(source, ["manaRegen", "mana_regen", "manaRegenPerSecond"]) ??
      fallback.serverStats.manaRegen,
    radar: {
      damage: getNumberValue(radar, ["damage"]) ?? fallback.serverStats.radar.damage,
      utility: getNumberValue(radar, ["utility"]) ?? fallback.serverStats.radar.utility,
      control:
        getNumberValue(radar, ["control", "crowdControl", "crowd_control"]) ??
        fallback.serverStats.radar.control,
      mobility:
        getNumberValue(radar, ["mobility", "engage"]) ??
        fallback.serverStats.radar.mobility,
      defense:
        getNumberValue(radar, ["defense", "defence"]) ?? fallback.serverStats.radar.defense,
    },
    resistance:
      getNumberValue(source, ["resistance", "magicResistance", "magic_resistance", "mr"]) ??
      fallback.serverStats.resistance,
  };
}

function getChampionAbilities(
  champion: ApiChampionRecord,
  fallback: UserPageChampion,
): ChampionAbility[] {
  const stats = getRecord(champion, "stats");
  const sourceAbilities = champion.abilities ?? getRecord(stats, "abilities");

  if (!sourceAbilities) {
    return fallback.abilities;
  }

  const abilityRecords: ApiChampionRecord[] = Array.isArray(sourceAbilities)
    ? sourceAbilities.filter(isRecord)
    : ["q", "w", "e"].reduce<ApiChampionRecord[]>((records, slot) => {
        const ability =
          getRecord(sourceAbilities, slot) ?? getRecord(sourceAbilities, slot.toUpperCase());

        if (isRecord(ability)) {
          records.push({ ...ability, slot });
        }

        return records;
      }, []);

  if (abilityRecords.length === 0) {
    return fallback.abilities;
  }

  return abilityRecords.slice(0, 3).map((ability, index) => {
    const fallbackAbility = fallback.abilities[index] ?? fallback.abilities[0];
    const damage = getRecord(ability, "damage");
    const damageType: ChampionScalingStat =
      normalizeChampionKey(ability.damageType ?? ability.scaling) === "ad" ? "ad" : "ap";

    return {
      cooldown:
        getNumberValue(ability, ["cooldown", "cooldownSeconds", "cooldown_seconds"]) ??
        fallbackAbility.cooldown,
      damageType,
      directDamage:
        getNumberValue(ability, ["directDamage", "direct_damage"]) ??
        getNumberValue(damage, ["directHit", "direct_hit", "direct"]) ??
        fallbackAbility.directDamage,
      directScaling:
        getNumberValue(ability, ["directScaling", "direct_scaling"]) ??
        fallbackAbility.directScaling,
      explosionDamage:
        getNumberValue(ability, ["explosionDamage", "explosion_damage", "areaDamage", "area_damage"]) ??
        getNumberValue(damage, ["area", "explosion"]) ??
        fallbackAbility.explosionDamage,
      explosionScaling:
        getNumberValue(ability, ["explosionScaling", "explosion_scaling", "areaScaling", "area_scaling"]) ??
        fallbackAbility.explosionScaling,
      name: getStringValue(ability, ["name", "displayName", "display_name"]) ?? fallbackAbility.name,
      slot: (getStringValue(ability, ["slot"]) ?? fallbackAbility.slot).toUpperCase(),
    };
  });
}

function mergeApiChampion(
  champion: ApiChampionRecord,
  ownershipStatus: ChampionOwnershipStatus,
) {
  const championKey = getApiChampionKey(champion);
  const fallback =
    fallbackChampionByKey.get(championKey) ??
    fallbackUserPageChampions.find((candidate) => candidate.name === "Lira") ??
    fallbackUserPageChampions[0];
  const name = getApiChampionName(champion, fallback);

  return {
    abilities: getChampionAbilities(champion, fallback),
    categories: getChampionCategories(champion, fallback),
    id: getStringValue(champion, ["id", "localizedName", "localized_name"]) ?? normalizeChampionKey(name),
    name,
    ownershipStatus,
    rank: getNumberValue(champion, ["rank", "championRank", "champion_rank"]) ?? fallback.rank,
    serverStats: getChampionServerStats(champion, fallback),
    wallpaper: fallback.wallpaper,
  };
}

function UserPageChampionCard({
  champion,
  onSelect,
}: {
  champion: UserPageChampion;
  onSelect: (
    champion: UserPageChampion,
    event: KeyboardEvent<HTMLElement> | MouseEvent<HTMLElement>,
  ) => void;
}) {
  const showsPrice = champion.ownershipStatus !== "owned";
  const isUnavailable = champion.ownershipStatus === "unowned";

  return (
    <article
      className={
        isUnavailable
          ? "user-page-champion-card user-page-champion-card-not-owned"
          : "user-page-champion-card"
      }
      role="button"
      style={{ "--champion-card-wallpaper": `url(${champion.wallpaper})` } as CSSProperties}
      tabIndex={0}
      onClick={(event) => onSelect(champion, event)}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onSelect(champion, event);
        }
      }}
    >
      <div className="user-page-champion-card-name">
        <span>{champion.name}</span>
        {showsPrice ? <ChampionPrice /> : null}
      </div>
    </article>
  );
}

function ChampionPrice() {
  return (
    <div className="user-page-champion-card-price" aria-label="Champion price">
      <strong className="user-page-champion-card-price-gems">
        {championDefaultPrice.gems} Gems
      </strong>
      <strong className="user-page-champion-card-price-tuc">
        {championDefaultPrice.tuc} TUC
      </strong>
    </div>
  );
}

function UserPageChampionSection({
  champions,
  onChampionSelect,
  title,
}: {
  champions: UserPageChampion[];
  onChampionSelect: (
    champion: UserPageChampion,
    event: KeyboardEvent<HTMLElement> | MouseEvent<HTMLElement>,
  ) => void;
  title: string;
}) {
  return (
    <section className="user-page-champion-section" aria-label={title}>
      <h2>{title}</h2>
      {champions.length > 0 ? (
        <div className="user-page-champion-grid">
          {champions.map((champion) => (
            <UserPageChampionCard
              champion={champion}
              key={champion.id}
              onSelect={onChampionSelect}
            />
          ))}
        </div>
      ) : (
        <div className="user-page-champion-empty" aria-hidden="true" />
      )}
    </section>
  );
}

function getRadarPoints(values: number[], radius: number, center = 110) {
  return values
    .map((value, index) => {
      const angle = -Math.PI / 2 + (index * Math.PI * 2) / values.length;
      const normalizedRadius = radius * Math.max(0, Math.min(value, 100)) / 100;
      const x = center + Math.cos(angle) * normalizedRadius;
      const y = center + Math.sin(angle) * normalizedRadius;
      return `${x.toFixed(2)},${y.toFixed(2)}`;
    })
    .join(" ");
}

function ChampionRadar({ champion, t }: { champion: UserPageChampion; t: Translate }) {
  const values = championRadarStats.map((stat) => champion.serverStats.radar[stat.id]);
  const axisPoints = championRadarStats.map((_, index) => {
    const angle = -Math.PI / 2 + (index * Math.PI * 2) / championRadarStats.length;
    return {
      x: 110 + Math.cos(angle) * 92,
      y: 110 + Math.sin(angle) * 92,
    };
  });
  const labelPoints = championRadarStats.map((stat, index) => {
    const angle = -Math.PI / 2 + (index * Math.PI * 2) / championRadarStats.length;
    const x = 110 + Math.cos(angle) * 106;
    const y = 110 + Math.sin(angle) * 106;
    const textAnchor: "end" | "middle" | "start" =
      Math.abs(x - 110) < 8 ? "middle" : x > 110 ? "start" : "end";

    return {
      id: stat.id,
      label: t(stat.labelKey),
      textAnchor,
      x,
      y,
    };
  });

  return (
    <div className="user-page-champion-radar">
      <svg viewBox="-16 -16 252 252" aria-hidden="true">
        {[24, 46, 68, 90].map((radius) => (
          <polygon
            className="user-page-champion-radar-ring"
            key={radius}
            points={getRadarPoints([100, 100, 100, 100, 100], radius)}
          />
        ))}
        {axisPoints.map((point, index) => (
          <line
            className="user-page-champion-radar-axis"
            key={championRadarStats[index].id}
            x1="110"
            x2={point.x}
            y1="110"
            y2={point.y}
          />
        ))}
        <polygon
          className="user-page-champion-radar-shape"
          points={getRadarPoints(values, 90)}
        />
        {labelPoints.map((point) => (
          <text
            className="user-page-champion-radar-corner-label"
            key={point.id}
            textAnchor={point.textAnchor}
            x={point.x}
            y={point.y}
          >
            {point.label}
          </text>
        ))}
      </svg>
      <div className="user-page-champion-radar-labels">
        {championRadarStats.map((stat) => (
          <div className="user-page-champion-radar-label" key={stat.id}>
            <span>{t(stat.labelKey)}</span>
            <strong>{champion.serverStats.radar[stat.id]}</strong>
          </div>
        ))}
      </div>
    </div>
  );
}

function ScalingIcon({ stat, t }: { stat: ChampionScalingStat; t: Translate }) {
  return (
    <span
      className={`user-page-champion-scaling user-page-champion-scaling-${stat}`}
      title={t(
        stat === "ap"
          ? "profile-champions-scaling-ap"
          : "profile-champions-scaling-ad",
      )}
    >
      {stat === "ap" ? <Flame size={14} /> : <Sword size={14} />}
    </span>
  );
}

function AbilityDescription({ ability, t }: { ability: ChampionAbility; t: Translate }) {
  return (
    <p className="user-page-champion-ability-description">
      <span>{t("profile-champions-ability-projectile-prefix")} </span>
      <strong>{ability.directDamage}</strong>
      <span> + </span>
      <strong>{ability.directScaling}%</strong>
      <ScalingIcon stat={ability.damageType} t={t} />
      <span> {t("profile-champions-ability-projectile-middle")} </span>
      <strong>{ability.explosionDamage}</strong>
      <span> + </span>
      <strong>{ability.explosionScaling}%</strong>
      <ScalingIcon stat={ability.damageType} t={t} />
      <span> {t("profile-champions-ability-projectile-suffix")}</span>
    </p>
  );
}

function ChampionAbilities({ champion, t }: { champion: UserPageChampion; t: Translate }) {
  return (
    <section className="user-page-champion-abilities" aria-label={t("profile-champions-abilities")}>
      <h3>{t("profile-champions-abilities")}</h3>
      <div className="user-page-champion-ability-list">
        {champion.abilities.map((ability) => (
          <article
            className="user-page-champion-ability-card"
            key={ability.slot}
            title={t("profile-champions-ability-preview-tooltip")}
          >
            <div className="user-page-champion-ability-header">
              <span>{ability.slot}</span>
              <strong>{ability.name}</strong>
              <small>{ability.cooldown.toFixed(1)}s</small>
            </div>
            <AbilityDescription ability={ability} t={t} />
          </article>
        ))}
      </div>
    </section>
  );
}

function ChampionBaseStats({ champion, t }: { champion: UserPageChampion; t: Translate }) {
  const stats = [
    { label: t("profile-champions-stat-health"), value: champion.serverStats.health },
    {
      label: t("profile-champions-stat-attack-damage"),
      value: champion.serverStats.attackDamage,
    },
    { label: t("profile-champions-stat-armor"), value: champion.serverStats.armor },
    {
      label: t("profile-champions-stat-resistance"),
      value: champion.serverStats.resistance,
    },
    { label: t("profile-champions-stat-control"), value: champion.serverStats.control },
    {
      label: t("profile-champions-stat-mana-regen"),
      value: `${champion.serverStats.manaRegen.toFixed(1)}/s`,
    },
    {
      label: t("profile-champions-stat-health-regen"),
      value: `${champion.serverStats.healthRegen.toFixed(1)}/s`,
    },
    { label: t("profile-champions-stat-abilities"), value: champion.abilities.length },
    {
      label: t("profile-champions-stat-cooldown"),
      value: `${champion.serverStats.cooldown.toFixed(1)}s`,
    },
    { label: t("profile-champions-stat-rank"), value: champion.rank || "-" },
  ];

  return (
    <dl className="user-page-champion-base-stats">
      {stats.map((stat) => (
        <div key={stat.label}>
          <dt>{stat.label}</dt>
          <dd>{stat.value}</dd>
        </div>
      ))}
    </dl>
  );
}

function getChampionSynergies(champion: UserPageChampion) {
  const championKeys = [champion.id, champion.name].map(normalizeChampionKey);
  const synergyKeys = Array.from(
    new Set(
      championKeys
        .flatMap((key) => championSynergyByKey[key] ?? [])
        .map(normalizeChampionKey)
        .filter(Boolean),
    ),
  );
  const synergies = synergyKeys
    .map((key) => fallbackChampionByKey.get(key))
    .filter((candidate): candidate is UserPageChampion => Boolean(candidate))
    .filter((candidate) => !isSameUserPageChampion(candidate, champion));

  if (synergies.length > 0) {
    return synergies;
  }

  return fallbackUserPageChampions
    .filter((candidate) => !isSameUserPageChampion(candidate, champion))
    .slice(0, 2);
}

function ChampionSynergies({ champion, t }: { champion: UserPageChampion; t: Translate }) {
  const synergies = getChampionSynergies(champion);

  return (
    <section
      className="user-page-champion-synergies"
      aria-label={t("profile-champions-synergies")}
    >
      <h3>{t("profile-champions-synergies")}</h3>
      <div className="user-page-champion-synergy-list">
        {synergies.map((synergy) => (
          <article
            className="user-page-champion-synergy-card"
            key={synergy.id}
            style={
              {
                "--champion-synergy-wallpaper": `url(${synergy.wallpaper})`,
              } as CSSProperties
            }
          >
            <strong>{synergy.name}</strong>
          </article>
        ))}
      </div>
    </section>
  );
}

function ChampionFocusDetails({ champion, t }: { champion: UserPageChampion; t: Translate }) {
  return (
    <section className="user-page-champion-focus-details" aria-label={champion.name}>
      <div className="user-page-champion-focus-main">
        <div className="user-page-champion-focus-copy">
          <span>{t("profile-champions-server-stats")}</span>
          <h2>{champion.name}</h2>
          <p>{t("profile-champions-stats-body")}</p>
        </div>
        <ChampionAbilities champion={champion} t={t} />
      </div>
      <div className="user-page-champion-focus-side">
        <ChampionRadar champion={champion} t={t} />
        <ChampionSynergies champion={champion} t={t} />
      </div>
    </section>
  );
}

function ChampionPurchaseModal({
  busy,
  champion,
  error,
  onClose,
  onPurchase,
  t,
}: {
  busy?: boolean;
  champion: UserPageChampion;
  error?: string;
  onClose: () => void;
  onPurchase: () => void;
  t: Translate;
}) {
  return (
    <div
      className="user-page-champion-purchase-backdrop"
      role="presentation"
      onMouseDown={busy ? undefined : onClose}
    >
      <section
        aria-label={t("profile-champions-purchase-title")}
        aria-modal="true"
        className="user-page-champion-purchase-modal"
        role="dialog"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <span>{t("profile-champions-unowned-badge")}</span>
        <h2>{champion.name}</h2>
        <p>{t("profile-champions-purchase-body")}</p>
        <ChampionPrice />
        {error ? (
          <p className="user-page-champion-purchase-error">{error}</p>
        ) : null}
        <div className="user-page-champion-purchase-actions">
          <button
            className="user-page-champion-purchase-button user-page-champion-purchase-button-gems"
            disabled={busy}
            type="button"
            onClick={onPurchase}
          >
            {busy
              ? t("profile-champions-purchase-processing")
              : t("profile-champions-purchase-gems")}
          </button>
          <button
            className="user-page-champion-purchase-button user-page-champion-purchase-button-tuc"
            disabled={busy}
            type="button"
            onClick={onPurchase}
          >
            {busy
              ? t("profile-champions-purchase-processing")
              : t("profile-champions-purchase-tuc")}
          </button>
          <button
            className="user-page-champion-purchase-cancel"
            disabled={busy}
            type="button"
            onClick={onClose}
          >
            {t("profile-champions-purchase-cancel")}
          </button>
        </div>
      </section>
    </div>
  );
}

function ProfileChampionsTab({ backSignal, onFocusChange, t, userId }: ProfileChampionsTabProps) {
  const [championOwnershipFilters, setChampionOwnershipFilters] = useState<
    ChampionOwnershipStatus[]
  >(["owned", "weekly", "unowned"]);
  const [championRankSort, setChampionRankSort] =
    useState<ChampionRankSort>("highest");
  const [championCategoryFilters, setChampionCategoryFilters] = useState<
    ChampionCategoryId[]
  >(championCategoryFilterOptions.map((category) => category.id));
  const [focusedChampion, setFocusedChampion] = useState<ChampionFocusState>();
  const [purchaseChampion, setPurchaseChampion] = useState<UserPageChampion>();
  const [purchaseError, setPurchaseError] = useState<string>();
  const [purchasingChampionId, setPurchasingChampionId] = useState<string>();
  const [userPageChampions, setUserPageChampions] =
    useState<UserPageChampion[]>(fallbackUserPageChampions);
  const championFocusCloseTimerRef = useRef<number | undefined>(undefined);
  const lastHandledBackSignalRef = useRef(backSignal);

  const filteredUserPageChampions = useMemo(
    () =>
      userPageChampions
        .filter((champion) => championOwnershipFilters.includes(champion.ownershipStatus))
        .filter((champion) => {
          return champion.categories.some((category) =>
            championCategoryFilters.includes(category),
          );
        })
        .slice()
        .sort((left, right) => {
          const rankDifference =
            championRankSort === "highest"
              ? right.rank - left.rank
              : left.rank - right.rank;

          if (rankDifference !== 0) {
            return rankDifference;
          }

          return left.name.localeCompare(right.name);
        }),
    [championCategoryFilters, championOwnershipFilters, championRankSort, userPageChampions],
  );
  const weeklyUserPageChampions = filteredUserPageChampions.filter(
    (champion) => champion.ownershipStatus === "weekly",
  );
  const ownedUserPageChampions = filteredUserPageChampions.filter(
    (champion) => champion.ownershipStatus === "owned",
  );
  const unownedUserPageChampions = filteredUserPageChampions.filter(
    (champion) => champion.ownershipStatus === "unowned",
  );

  useEffect(() => {
    let ignore = false;

    async function loadChampions() {
      const [allResult, weeklyResult, ownedResult] = await Promise.all([
        getChampionCatalog().catch(() => undefined),
        getChampionCatalog({ query: { weekly: true } }).catch(() => undefined),
        userId !== undefined
          ? getChampionCatalog({ query: { owned: true, userId } }).catch(() => undefined)
          : Promise.resolve(undefined),
      ]);

      if (ignore) {
        return;
      }

      const allRecords = getChampionCatalogItems(allResult?.data);
      const weeklyRecords = getChampionCatalogItems(weeklyResult?.data);
      const ownedRecords = getChampionCatalogItems(ownedResult?.data);
      const allByKey = new Map<string, ApiChampionRecord>();
      const canonicalKeyByAlias = new Map<string, string>();

      for (const champion of [...allRecords, ...weeklyRecords, ...ownedRecords]) {
        const championKeys = getApiChampionKeys(champion);
        const knownAlias = championKeys.find((candidate) =>
          canonicalKeyByAlias.has(candidate),
        );
        const key = knownAlias ? canonicalKeyByAlias.get(knownAlias) : championKeys[0];

        if (key) {
          allByKey.set(key, {
            ...(allByKey.get(key) ?? {}),
            ...champion,
          });
          for (const alias of championKeys) {
            canonicalKeyByAlias.set(alias, key);
          }
        }
      }

      if (allByKey.size === 0) {
        setUserPageChampions(fallbackUserPageChampions);
        return;
      }

      const weeklyKeys = new Set(
        weeklyRecords
          .flatMap(getApiChampionKeys)
          .map((key) => canonicalKeyByAlias.get(key) ?? key)
          .filter(Boolean),
      );
      const ownedKeys = new Set(
        ownedRecords
          .flatMap(getApiChampionKeys)
          .map((key) => canonicalKeyByAlias.get(key) ?? key)
          .filter(Boolean),
      );
      const nextChampions = Array.from(allByKey.entries()).map(([key, champion]) => {
        const ownershipStatus: ChampionOwnershipStatus = ownedKeys.has(key)
          ? "owned"
          : weeklyKeys.has(key)
            ? "weekly"
            : "unowned";

        return mergeApiChampion(champion, ownershipStatus);
      });

      setUserPageChampions(nextChampions);
    }

    void loadChampions();

    return () => {
      ignore = true;
    };
  }, [userId]);

  function clearChampionFocusCloseTimer() {
    if (championFocusCloseTimerRef.current !== undefined) {
      window.clearTimeout(championFocusCloseTimerRef.current);
      championFocusCloseTimerRef.current = undefined;
    }
  }

  function closeFocusedChampion() {
    setPurchaseChampion(undefined);
    setPurchaseError(undefined);
    setFocusedChampion((current) => {
      if (!current || current.closing) {
        return current;
      }

      clearChampionFocusCloseTimer();
      championFocusCloseTimerRef.current = window.setTimeout(() => {
        championFocusCloseTimerRef.current = undefined;
        setFocusedChampion(undefined);
      }, 680);

      return { ...current, closing: true };
    });
  }

  async function handleChampionPurchase(champion: UserPageChampion) {
    if (userId === undefined) {
      setPurchaseError(t("profile-champions-purchase-user-missing"));
      return;
    }

    setPurchaseError(undefined);
    setPurchasingChampionId(champion.id);

    const result = await setOwnedChampion({
      body: {
        champion: champion.name,
        userId,
      },
    }).catch(() => undefined);

    setPurchasingChampionId(undefined);

    if (!result || result.error || (result.response?.status ?? 500) >= 400) {
      setPurchaseError(t("profile-champions-purchase-error"));
      return;
    }

    const markChampionOwned = (candidate: UserPageChampion): UserPageChampion =>
      isSameUserPageChampion(candidate, champion)
        ? { ...candidate, ownershipStatus: "owned" }
        : candidate;

    setUserPageChampions((current) => current.map(markChampionOwned));
    setFocusedChampion((current) =>
      current ? { ...current, champion: markChampionOwned(current.champion) } : current,
    );
    setPurchaseChampion(undefined);
  }

  function handleChampionFocusOpen(
    champion: UserPageChampion,
    event: KeyboardEvent<HTMLElement> | MouseEvent<HTMLElement>,
  ) {
    const cardRect = event.currentTarget.getBoundingClientRect();
    const pageRect =
      event.currentTarget.closest<HTMLElement>(".user-page")?.getBoundingClientRect();

    clearChampionFocusCloseTimer();
    setFocusedChampion({
      champion,
      startLeft: Math.max(0, cardRect.left - (pageRect?.left ?? 0)),
      startTop: Math.max(0, cardRect.top - (pageRect?.top ?? 0)),
    });
  }

  useEffect(() => {
    onFocusChange(Boolean(focusedChampion));
  }, [focusedChampion, onFocusChange]);

  useEffect(() => {
    return () => {
      clearChampionFocusCloseTimer();
      onFocusChange(false);
    };
  }, [onFocusChange]);

  useEffect(() => {
    if (backSignal === lastHandledBackSignalRef.current) {
      return;
    }

    lastHandledBackSignalRef.current = backSignal;
    if (purchaseChampion) {
      setPurchaseChampion(undefined);
      setPurchaseError(undefined);
      return;
    }

    closeFocusedChampion();
  }, [backSignal, purchaseChampion]);

  return (
    <div
      className={
        focusedChampion
          ? "user-page-champions user-page-champions-focused"
          : "user-page-champions"
      }
      aria-label="Champions"
    >
      <aside className="user-page-champion-filters" aria-label="Champion filters">
        <details className="user-page-champion-filter-dropdown">
          <summary>{t("profile-champions-filter-ownership")}</summary>
          {championOwnershipFilterOptions.map((option) => (
            <label className="user-page-champion-filter-option" key={option.id}>
              <input
                checked={championOwnershipFilters.includes(option.id)}
                type="checkbox"
                onChange={(event) => {
                  const checked = event.currentTarget.checked;
                  setChampionOwnershipFilters((filters) =>
                    checked
                      ? [...filters, option.id]
                      : filters.filter((filter) => filter !== option.id),
                  );
                }}
              />
              <span>{t(option.labelKey)}</span>
            </label>
          ))}
        </details>
        <details className="user-page-champion-filter-dropdown">
          <summary>{t("profile-champions-filter-rank")}</summary>
          <label className="user-page-champion-filter-option">
            <input
              checked={championRankSort === "highest"}
              name="champion-rank-sort"
              type="radio"
              onChange={() => setChampionRankSort("highest")}
            />
            <span>{t("profile-champions-rank-highest")}</span>
          </label>
          <label className="user-page-champion-filter-option">
            <input
              checked={championRankSort === "lowest"}
              name="champion-rank-sort"
              type="radio"
              onChange={() => setChampionRankSort("lowest")}
            />
            <span>{t("profile-champions-rank-lowest")}</span>
          </label>
        </details>
        <details className="user-page-champion-filter-dropdown">
          <summary>{t("profile-champions-filter-category")}</summary>
          {championCategoryFilterOptions.map((option) => (
            <label className="user-page-champion-filter-option" key={option.id}>
              <input
                checked={championCategoryFilters.includes(option.id)}
                type="checkbox"
                onChange={(event) => {
                  const checked = event.currentTarget.checked;
                  setChampionCategoryFilters((filters) =>
                    checked
                      ? [...filters, option.id]
                      : filters.filter((filter) => filter !== option.id),
                  );
                }}
              />
              <span>{t(option.labelKey)}</span>
            </label>
          ))}
        </details>
      </aside>
      <div className="user-page-champion-sections">
        {championOwnershipFilters.includes("weekly") ? (
          <UserPageChampionSection
            champions={weeklyUserPageChampions}
            onChampionSelect={handleChampionFocusOpen}
            title={t("profile-champions-weekly")}
          />
        ) : null}
        {championOwnershipFilters.includes("owned") ? (
          <UserPageChampionSection
            champions={ownedUserPageChampions}
            onChampionSelect={handleChampionFocusOpen}
            title={t("profile-champions-owned")}
          />
        ) : null}
        {championOwnershipFilters.includes("unowned") ? (
          <UserPageChampionSection
            champions={unownedUserPageChampions}
            onChampionSelect={handleChampionFocusOpen}
            title={t("profile-champions-unowned")}
          />
        ) : null}
      </div>
      {focusedChampion ? (
        <div
          className={
            focusedChampion.closing
              ? "user-page-champion-focus user-page-champion-focus-closing"
              : "user-page-champion-focus"
          }
          role="presentation"
          style={
            {
              "--champion-card-wallpaper": `url(${focusedChampion.champion.wallpaper})`,
            } as CSSProperties
          }
        >
          <article
            className={
              focusedChampion.champion.ownershipStatus !== "owned"
                ? "user-page-champion-focus-card user-page-champion-focus-card-purchaseable"
                : "user-page-champion-focus-card"
            }
            role={focusedChampion.champion.ownershipStatus !== "owned" ? "button" : undefined}
            style={
              {
                "--champion-card-wallpaper": `url(${focusedChampion.champion.wallpaper})`,
                "--champion-focus-start-left": `${focusedChampion.startLeft}px`,
                "--champion-focus-start-top": `${focusedChampion.startTop}px`,
              } as CSSProperties
            }
            tabIndex={focusedChampion.champion.ownershipStatus !== "owned" ? 0 : undefined}
            title={
              focusedChampion.champion.ownershipStatus !== "owned"
                ? t("profile-champions-purchase-tooltip")
                : undefined
            }
            onClick={() => {
              if (focusedChampion.champion.ownershipStatus !== "owned") {
                setPurchaseError(undefined);
                setPurchaseChampion(focusedChampion.champion);
              }
            }}
            onKeyDown={(event) => {
              if (
                focusedChampion.champion.ownershipStatus !== "owned" &&
                (event.key === "Enter" || event.key === " ")
              ) {
                event.preventDefault();
                setPurchaseError(undefined);
                setPurchaseChampion(focusedChampion.champion);
              }
            }}
          >
            {focusedChampion.champion.ownershipStatus !== "owned" ? (
              <div className="user-page-champion-unowned-badge">
                {t("profile-champions-unowned-badge")}
              </div>
            ) : null}
            <div className="user-page-champion-card-name">
              <span>{focusedChampion.champion.name}</span>
              {focusedChampion.champion.ownershipStatus !== "owned" ? <ChampionPrice /> : null}
            </div>
          </article>
          <ChampionBaseStats champion={focusedChampion.champion} t={t} />
          <ChampionFocusDetails champion={focusedChampion.champion} t={t} />
          {purchaseChampion ? (
            <ChampionPurchaseModal
              busy={purchasingChampionId === purchaseChampion.id}
              champion={purchaseChampion}
              error={purchaseError}
              onClose={() => {
                setPurchaseChampion(undefined);
                setPurchaseError(undefined);
              }}
              onPurchase={() => void handleChampionPurchase(purchaseChampion)}
              t={t}
            />
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

export default ProfileChampionsTab;
