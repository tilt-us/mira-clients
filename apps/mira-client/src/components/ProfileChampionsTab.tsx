import {
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type MouseEvent,
} from "react";
import { Flame, Sword } from "lucide-react";
import ignaraWallpaper from "../../../../assets/wallpapers/ignara-wallpaper.png";
import liraWallpaper from "../../../../assets/wallpapers/lira-wallpaper.png";
import sophiaWallpaper from "../../../../assets/wallpapers/sophia-wallpaper.png";
import yunaWallpaper from "../../../../assets/wallpapers/yuna-wallpaper.png";
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
type ChampionRadarStatId = "damage" | "utility" | "control" | "engage" | "defense";
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
  { id: "engage", labelKey: "profile-champions-radar-engage" },
  { id: "defense", labelKey: "profile-champions-radar-defense" },
];

const userPageChampions: UserPageChampion[] = [
  {
    categories: ["fighter"],
    id: "ignara",
    name: "Ignara",
    ownershipStatus: "weekly",
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
      radar: { damage: 86, utility: 44, control: 68, engage: 72, defense: 52 },
      resistance: 31,
    },
    wallpaper: ignaraWallpaper,
  },
  {
    categories: ["assassin"],
    id: "lira",
    name: "Lira",
    ownershipStatus: "weekly",
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
      radar: { damage: 82, utility: 56, control: 42, engage: 78, defense: 38 },
      resistance: 26,
    },
    wallpaper: liraWallpaper,
  },
  {
    categories: ["mage"],
    id: "sophia",
    name: "Sophia",
    ownershipStatus: "weekly",
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
      radar: { damage: 76, utility: 88, control: 58, engage: 38, defense: 46 },
      resistance: 34,
    },
    wallpaper: sophiaWallpaper,
  },
  {
    categories: ["guardian"],
    id: "yuna",
    name: "Yuna",
    ownershipStatus: "weekly",
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
      radar: { damage: 52, utility: 70, control: 76, engage: 54, defense: 86 },
      resistance: 38,
    },
    wallpaper: yunaWallpaper,
  },
];

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
  return (
    <article
      className="user-page-champion-card"
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
      </div>
    </article>
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
      <ChampionRadar champion={champion} t={t} />
    </section>
  );
}

function ProfileChampionsTab({ backSignal, onFocusChange, t }: ProfileChampionsTabProps) {
  const [championOwnershipFilters, setChampionOwnershipFilters] = useState<
    ChampionOwnershipStatus[]
  >(["owned", "weekly", "unowned"]);
  const [championRankSort, setChampionRankSort] =
    useState<ChampionRankSort>("highest");
  const [championCategoryFilters, setChampionCategoryFilters] = useState<
    ChampionCategoryId[]
  >(championCategoryFilterOptions.map((category) => category.id));
  const [focusedChampion, setFocusedChampion] = useState<ChampionFocusState>();
  const championFocusCloseTimerRef = useRef<number | undefined>(undefined);
  const lastHandledBackSignalRef = useRef(backSignal);

  const filteredUserPageChampions = userPageChampions
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
    });
  const weeklyUserPageChampions = filteredUserPageChampions.filter(
    (champion) => champion.ownershipStatus === "weekly",
  );
  const ownedUserPageChampions = filteredUserPageChampions.filter(
    (champion) => champion.ownershipStatus === "owned",
  );
  const unownedUserPageChampions = filteredUserPageChampions.filter(
    (champion) => champion.ownershipStatus === "unowned",
  );

  function clearChampionFocusCloseTimer() {
    if (championFocusCloseTimerRef.current !== undefined) {
      window.clearTimeout(championFocusCloseTimerRef.current);
      championFocusCloseTimerRef.current = undefined;
    }
  }

  function closeFocusedChampion() {
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
    closeFocusedChampion();
  }, [backSignal]);

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
        <UserPageChampionSection
          champions={weeklyUserPageChampions}
          onChampionSelect={handleChampionFocusOpen}
          title={t("profile-champions-weekly")}
        />
        <UserPageChampionSection
          champions={ownedUserPageChampions}
          onChampionSelect={handleChampionFocusOpen}
          title={t("profile-champions-owned")}
        />
        <UserPageChampionSection
          champions={unownedUserPageChampions}
          onChampionSelect={handleChampionFocusOpen}
          title={t("profile-champions-unowned")}
        />
      </div>
      {focusedChampion ? (
        <div
          className={
            focusedChampion.closing
              ? "user-page-champion-focus user-page-champion-focus-closing"
              : "user-page-champion-focus"
          }
          role="presentation"
        >
          <article
            className="user-page-champion-focus-card"
            style={
              {
                "--champion-card-wallpaper": `url(${focusedChampion.champion.wallpaper})`,
                "--champion-focus-start-left": `${focusedChampion.startLeft}px`,
                "--champion-focus-start-top": `${focusedChampion.startTop}px`,
              } as CSSProperties
            }
          >
            <div className="user-page-champion-card-name">
              <span>{focusedChampion.champion.name}</span>
            </div>
          </article>
          <ChampionBaseStats champion={focusedChampion.champion} t={t} />
          <ChampionFocusDetails champion={focusedChampion.champion} t={t} />
        </div>
      ) : null}
    </div>
  );
}

export default ProfileChampionsTab;
