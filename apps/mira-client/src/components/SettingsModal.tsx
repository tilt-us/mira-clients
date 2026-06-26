import { useEffect, useState } from "react";
import { Gamepad2, Monitor, Search, Users } from "lucide-react";
import type { AppLocale } from "../i18n";
import type {
  AppResolution,
  BackgroundChampion,
  ClientAnimation,
  FriendRequestPolicy,
  GameScreenMode,
  UiScale,
} from "../settings";
import type { SettingsVision, Translate } from "../types/ui";

type SettingsModalProps = {
  accentColor: string;
  backgroundChampion: BackgroundChampion;
  clientAnimation: ClientAnimation;
  friendRequestPolicy: FriendRequestPolicy;
  gameScreenMode: GameScreenMode;
  locale: AppLocale;
  resolution: AppResolution;
  supportsFourKResolution: boolean;
  supportsTwoKResolution: boolean;
  uiScale: UiScale;
  onAccentColorChange: (accentColor: string) => void;
  onBackgroundChampionChange: (backgroundChampion: BackgroundChampion) => void;
  onClientAnimationChange: (clientAnimation: ClientAnimation) => void;
  onClose: () => void;
  onFriendRequestPolicyChange: (friendRequestPolicy: FriendRequestPolicy) => void;
  onGameScreenModeChange: (gameScreenMode: GameScreenMode) => void;
  onLocaleChange: (locale: AppLocale) => void;
  onResolutionChange: (resolution: AppResolution) => void;
  onUiScaleChange: (uiScale: UiScale) => void;
  t: Translate;
  vision: SettingsVision;
};

type SettingsTab = "interface" | "game" | "social";

const resolutionOptions: Array<{
  label: string;
  value: AppResolution;
}> = [
  { label: "1270 x 720", value: "1270x720" },
  { label: "1400 x 800", value: "1400x800" },
  { label: "1600 x 900", value: "1600x900" },
  {
    label: "1920 x 1080",
    value: "1920x1080",
  },
  { label: "2140 x 1440", value: "2140x1440" },
  { label: "2600 x 1600", value: "2600x1600" },
];

const uiScaleOptions: Array<{
  label: string;
  value: UiScale;
}> = [
  { label: "150%", value: 1.5 },
  { label: "125%", value: 1.25 },
  { label: "110%", value: 1.1 },
  { label: "100%", value: 1 },
  { label: "90%", value: 0.9 },
  { label: "80%", value: 0.8 },
  { label: "70%", value: 0.7 },
  { label: "60%", value: 0.6 },
  { label: "50%", value: 0.5 },
];

const maxUiScaleByResolution: Record<AppResolution, UiScale> = {
  "1270x720": 1,
  "1400x800": 1.1,
  "1600x900": 1.25,
  "1920x1080": 1.5,
  "2600x1600": 1.5,
  "2140x1440": 1.5,
};

const clientAnimationOptions: Array<{
  labelId: string;
  value: ClientAnimation;
}> = [
  { labelId: "settings-client-animation-all", value: "all" },
  { labelId: "settings-client-animation-ui-elements", value: "ui-elements" },
  { labelId: "settings-client-animation-images", value: "images" },
  { labelId: "settings-client-animation-none", value: "none" },
];

const gameScreenModeOptions: Array<{
  labelId: string;
  value: GameScreenMode;
}> = [
  { labelId: "settings-screen-mode-borderless", value: "borderless" },
  { labelId: "settings-screen-mode-full", value: "full" },
  { labelId: "settings-screen-mode-window", value: "window" },
];

const backgroundChampionOptions: Array<{
  label: string;
  value: BackgroundChampion;
}> = [
  { label: "Lira", value: "lira" },
  { label: "Ignara", value: "ignara" },
  { label: "Yuna", value: "yuna" },
  { label: "Sophia", value: "sophia" },
];

const friendRequestPolicyOptions: Array<{
  labelId: string;
  value: FriendRequestPolicy;
}> = [
  { labelId: "settings-friend-request-allow", value: "allow" },
  { labelId: "settings-friend-request-disallow", value: "disallow" },
  { labelId: "settings-friend-request-vip", value: "vip" },
];

function SettingsModal({
  accentColor,
  backgroundChampion,
  clientAnimation,
  friendRequestPolicy,
  gameScreenMode,
  locale,
  resolution,
  supportsFourKResolution,
  supportsTwoKResolution,
  uiScale,
  onAccentColorChange,
  onBackgroundChampionChange,
  onClientAnimationChange,
  onClose,
  onFriendRequestPolicyChange,
  onGameScreenModeChange,
  onLocaleChange,
  onResolutionChange,
  onUiScaleChange,
  t,
  vision,
}: SettingsModalProps) {
  const [activeSettingsTab, setActiveSettingsTab] =
    useState<SettingsTab>("interface");
  const [backgroundChampionDropdownOpen, setBackgroundChampionDropdownOpen] =
    useState(false);
  const [backgroundChampionSearch, setBackgroundChampionSearch] = useState("");
  const [clientAnimationDropdownOpen, setClientAnimationDropdownOpen] =
    useState(false);
  const [friendRequestPolicyDropdownOpen, setFriendRequestPolicyDropdownOpen] =
    useState(false);
  const [gameScreenModeDropdownOpen, setGameScreenModeDropdownOpen] =
    useState(false);
  const [languageDropdownOpen, setLanguageDropdownOpen] = useState(false);
  const [resolutionDropdownOpen, setResolutionDropdownOpen] = useState(false);
  const [uiScaleDropdownOpen, setUiScaleDropdownOpen] = useState(false);
  const selectedLanguage =
    locale === "de" ? t("language-german") : t("language-english");
  const selectedFlag = locale === "de" ? "🇩🇪" : "🇬🇧";
  const gameTabAvailable = vision === "Vision.ALL";
  const socialTabAvailable = vision === "Vision.ALL";
  const visibleResolutionOptions = resolutionOptions.filter((option) => {
    return isResolutionVisible(
      option.value,
      supportsTwoKResolution,
      supportsFourKResolution,
    );
  });
  const selectedResolution =
    visibleResolutionOptions.find((option) => option.value === resolution)?.label ??
    "1600 x 900";
  const visibleUiScaleOptions = uiScaleOptions.filter((option) => {
    return option.value <= maxUiScaleByResolution[resolution];
  });
  const selectedUiScale =
    visibleUiScaleOptions.find((option) => option.value === uiScale)?.label ?? "90%";
  const selectedClientAnimation =
    clientAnimationOptions.find((option) => option.value === clientAnimation)?.labelId ??
    "settings-client-animation-all";
  const selectedGameScreenMode =
    gameScreenModeOptions.find((option) => option.value === gameScreenMode)?.labelId ??
    "settings-screen-mode-borderless";
  const selectedBackgroundChampion =
    backgroundChampionOptions.find((option) => option.value === backgroundChampion)?.label ??
    "Yuna";
  const selectedFriendRequestPolicy =
    friendRequestPolicyOptions.find((option) => option.value === friendRequestPolicy)
      ?.labelId ?? "settings-friend-request-allow";
  const filteredBackgroundChampionOptions = backgroundChampionOptions.filter((option) =>
    option.label.toLowerCase().includes(backgroundChampionSearch.trim().toLowerCase()),
  );

  useEffect(() => {
    if (!gameTabAvailable && activeSettingsTab === "game") {
      setActiveSettingsTab("interface");
    }
  }, [activeSettingsTab, gameTabAvailable]);

  function closeDropdowns() {
    setBackgroundChampionDropdownOpen(false);
    setClientAnimationDropdownOpen(false);
    setFriendRequestPolicyDropdownOpen(false);
    setGameScreenModeDropdownOpen(false);
    setLanguageDropdownOpen(false);
    setResolutionDropdownOpen(false);
    setUiScaleDropdownOpen(false);
  }

  function handleLocaleSelect(nextLocale: AppLocale) {
    onLocaleChange(nextLocale);
    setLanguageDropdownOpen(false);
  }

  function handleClientAnimationSelect(nextClientAnimation: ClientAnimation) {
    onClientAnimationChange(nextClientAnimation);
    setClientAnimationDropdownOpen(false);
  }

  function handleBackgroundChampionSelect(nextBackgroundChampion: BackgroundChampion) {
    onBackgroundChampionChange(nextBackgroundChampion);
    setBackgroundChampionDropdownOpen(false);
    setBackgroundChampionSearch("");
  }

  function handleFriendRequestPolicySelect(nextFriendRequestPolicy: FriendRequestPolicy) {
    onFriendRequestPolicyChange(nextFriendRequestPolicy);
    setFriendRequestPolicyDropdownOpen(false);
  }

  function handleGameScreenModeSelect(nextGameScreenMode: GameScreenMode) {
    onGameScreenModeChange(nextGameScreenMode);
    setGameScreenModeDropdownOpen(false);
  }

  function handleResolutionSelect(nextResolution: AppResolution) {
    onResolutionChange(nextResolution);
    setResolutionDropdownOpen(false);
  }

  function handleUiScaleSelect(nextUiScale: UiScale) {
    onUiScaleChange(nextUiScale);
    setUiScaleDropdownOpen(false);
  }

  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={onClose}>
      <div
        aria-labelledby="settings-dialog-title"
        aria-modal="true"
        className="settings-dialog"
        data-vision={vision}
        role="dialog"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="settings-dialog-header">
          <h2 id="settings-dialog-title">{t("settings-title")}</h2>
        </div>

        <div className="settings-dialog-body" onMouseDown={closeDropdowns}>
          <nav className="settings-tabs" aria-label={t("settings-title")}>
            <button
              className={activeSettingsTab === "interface" ? "active" : ""}
              type="button"
              onClick={() => setActiveSettingsTab("interface")}
            >
              <Monitor size={17} />
              <span>{t("settings-interface")}</span>
            </button>
            {gameTabAvailable ? (
              <button
                className={activeSettingsTab === "game" ? "active" : ""}
                type="button"
                onClick={() => setActiveSettingsTab("game")}
              >
                <Gamepad2 size={17} />
                <span>{t("settings-game")}</span>
              </button>
            ) : null}
            {socialTabAvailable ? (
              <button
                className={activeSettingsTab === "social" ? "active" : ""}
                type="button"
                onClick={() => setActiveSettingsTab("social")}
              >
                <Users size={17} />
                <span>{t("settings-social")}</span>
              </button>
            ) : null}
          </nav>

          <div className="settings-pane">
            {activeSettingsTab === "interface" ? (
              <>
                <div className="settings-row">
                  <span>{t("settings-resolution")}</span>
                  <div
                    className="settings-dropdown"
                    onMouseDown={(event) => event.stopPropagation()}
                  >
                    <button
                      aria-expanded={resolutionDropdownOpen}
                      aria-haspopup="listbox"
                      className="settings-dropdown-trigger"
                      type="button"
                      onClick={() => {
                        closeDropdowns();
                        setResolutionDropdownOpen((open) => !open);
                      }}
                    >
                      <span>{selectedResolution}</span>
                    </button>

                    {resolutionDropdownOpen ? (
                      <div className="settings-dropdown-menu" role="listbox">
                        {visibleResolutionOptions.map((option) => (
                          <button
                            aria-selected={resolution === option.value}
                            key={option.value}
                            role="option"
                            type="button"
                            onClick={() => handleResolutionSelect(option.value)}
                          >
                            <span>{option.label}</span>
                          </button>
                        ))}
                      </div>
                    ) : null}
                  </div>
                </div>

                <div className="settings-row">
                  <span>{t("settings-ui-scale")}</span>
                  <div
                    className="settings-dropdown"
                    onMouseDown={(event) => event.stopPropagation()}
                  >
                    <button
                      aria-expanded={uiScaleDropdownOpen}
                      aria-haspopup="listbox"
                      className="settings-dropdown-trigger"
                      type="button"
                      onClick={() => {
                        closeDropdowns();
                        setUiScaleDropdownOpen((open) => !open);
                      }}
                    >
                      <span>{selectedUiScale}</span>
                    </button>

                    {uiScaleDropdownOpen ? (
                      <div
                        className="settings-dropdown-menu settings-dropdown-menu-scroll"
                        role="listbox"
                      >
                        {visibleUiScaleOptions.map((option) => (
                          <button
                            aria-selected={uiScale === option.value}
                            key={option.value}
                            role="option"
                            type="button"
                            onClick={() => handleUiScaleSelect(option.value)}
                          >
                            <span>{option.label}</span>
                          </button>
                        ))}
                      </div>
                    ) : null}
                  </div>
                </div>

                <label className="settings-row">
                  <span>{t("settings-accent")}</span>
                  <input
                    aria-label={t("settings-accent")}
                    type="color"
                    value={accentColor}
                    onChange={(event) => onAccentColorChange(event.target.value)}
                  />
                </label>

                {vision === "Vision.ALL" ? (
                  <div className="settings-row">
                    <span>{t("settings-background")}</span>
                    <div
                      className="settings-dropdown"
                      onMouseDown={(event) => event.stopPropagation()}
                    >
                      <button
                        aria-expanded={backgroundChampionDropdownOpen}
                        aria-haspopup="listbox"
                        className="settings-dropdown-trigger"
                        type="button"
                        onClick={() => {
                          closeDropdowns();
                          setBackgroundChampionDropdownOpen((open) => !open);
                        }}
                      >
                        <span>{selectedBackgroundChampion}</span>
                      </button>

                      {backgroundChampionDropdownOpen ? (
                        <div className="settings-dropdown-menu" role="listbox">
                          <label className="settings-dropdown-search">
                            <Search size={15} />
                            <input
                              aria-label={t("settings-background-search")}
                              placeholder={t("settings-background-search")}
                              value={backgroundChampionSearch}
                              onChange={(event) =>
                                setBackgroundChampionSearch(event.target.value)
                              }
                            />
                          </label>
                          {filteredBackgroundChampionOptions.length > 0 ? (
                            filteredBackgroundChampionOptions.map((option) => (
                              <button
                                aria-selected={backgroundChampion === option.value}
                                key={option.value}
                                role="option"
                                type="button"
                                onClick={() => handleBackgroundChampionSelect(option.value)}
                              >
                                <span>{option.label}</span>
                              </button>
                            ))
                          ) : (
                            <span className="settings-dropdown-empty">
                              {t("settings-background-empty")}
                            </span>
                          )}
                        </div>
                      ) : null}
                    </div>
                  </div>
                ) : null}

                <div className="settings-row">
                  <span>{t("settings-client-animation")}</span>
                  <div
                    className="settings-dropdown"
                    onMouseDown={(event) => event.stopPropagation()}
                  >
                    <button
                      aria-expanded={clientAnimationDropdownOpen}
                      aria-haspopup="listbox"
                      className="settings-dropdown-trigger"
                      type="button"
                      onClick={() => {
                        closeDropdowns();
                        setClientAnimationDropdownOpen((open) => !open);
                      }}
                    >
                      <span>{t(selectedClientAnimation)}</span>
                    </button>

                    {clientAnimationDropdownOpen ? (
                      <div className="settings-dropdown-menu" role="listbox">
                        {clientAnimationOptions.map((option) => (
                          <button
                            aria-selected={clientAnimation === option.value}
                            key={option.value}
                            role="option"
                            type="button"
                            onClick={() => handleClientAnimationSelect(option.value)}
                          >
                            <span>{t(option.labelId)}</span>
                          </button>
                        ))}
                      </div>
                    ) : null}
                  </div>
                </div>

                <div className="settings-row">
                  <span>{t("settings-language")}</span>
                  <div
                    className="settings-dropdown"
                    onMouseDown={(event) => event.stopPropagation()}
                  >
                    <button
                      aria-expanded={languageDropdownOpen}
                      aria-haspopup="listbox"
                      className="settings-dropdown-trigger"
                      type="button"
                      onClick={() => {
                        closeDropdowns();
                        setLanguageDropdownOpen((open) => !open);
                      }}
                    >
                      <span>{selectedFlag}</span>
                      <span>{selectedLanguage}</span>
                    </button>

                    {languageDropdownOpen ? (
                      <div className="settings-dropdown-menu" role="listbox">
                        <button
                          aria-selected={locale === "de"}
                          role="option"
                          type="button"
                          onClick={() => handleLocaleSelect("de")}
                        >
                          <span>🇩🇪</span>
                          <span>{t("language-german")}</span>
                        </button>
                        <button
                          aria-selected={locale === "en"}
                          role="option"
                          type="button"
                          onClick={() => handleLocaleSelect("en")}
                        >
                          <span>🇬🇧</span>
                          <span>{t("language-english")}</span>
                        </button>
                      </div>
                    ) : null}
                  </div>
                </div>
              </>
            ) : null}

            {gameTabAvailable && activeSettingsTab === "game" ? (
              <div className="settings-row">
                <span>{t("settings-game-screen-mode")}</span>
                <div
                  className="settings-dropdown"
                  onMouseDown={(event) => event.stopPropagation()}
                >
                  <button
                    aria-expanded={gameScreenModeDropdownOpen}
                    aria-haspopup="listbox"
                    className="settings-dropdown-trigger"
                    type="button"
                    onClick={() => {
                      closeDropdowns();
                      setGameScreenModeDropdownOpen((open) => !open);
                    }}
                  >
                    <span>{t(selectedGameScreenMode)}</span>
                  </button>

                  {gameScreenModeDropdownOpen ? (
                    <div className="settings-dropdown-menu" role="listbox">
                      {gameScreenModeOptions.map((option) => (
                        <button
                          aria-selected={gameScreenMode === option.value}
                          key={option.value}
                          role="option"
                          type="button"
                          onClick={() => handleGameScreenModeSelect(option.value)}
                        >
                          <span>{t(option.labelId)}</span>
                        </button>
                      ))}
                    </div>
                  ) : null}
                </div>
              </div>
            ) : null}

            {socialTabAvailable && activeSettingsTab === "social" ? (
              <div className="settings-row">
                <span>{t("settings-allow-friend-request")}</span>
                <div
                  className="settings-dropdown"
                  onMouseDown={(event) => event.stopPropagation()}
                >
                  <button
                    aria-expanded={friendRequestPolicyDropdownOpen}
                    aria-haspopup="listbox"
                    className="settings-dropdown-trigger"
                    type="button"
                    onClick={() => {
                      closeDropdowns();
                      setFriendRequestPolicyDropdownOpen((open) => !open);
                    }}
                  >
                    <span>{t(selectedFriendRequestPolicy)}</span>
                  </button>

                  {friendRequestPolicyDropdownOpen ? (
                    <div className="settings-dropdown-menu" role="listbox">
                      {friendRequestPolicyOptions.map((option) => (
                        <button
                          aria-selected={friendRequestPolicy === option.value}
                          key={option.value}
                          role="option"
                          type="button"
                          onClick={() => handleFriendRequestPolicySelect(option.value)}
                        >
                          <span>{t(option.labelId)}</span>
                        </button>
                      ))}
                    </div>
                  ) : null}
                </div>
              </div>
            ) : null}
          </div>
        </div>

        <button className="secondary-button" type="button" onClick={onClose}>
          {t("settings-close")}
        </button>
      </div>
    </div>
  );
}

function isResolutionVisible(
  resolution: AppResolution,
  supportsTwoKResolution: boolean,
  supportsFourKResolution: boolean,
) {
  if (
    getResolutionRank(resolution) <
    getResolutionRank(
      getMinimumVisibleResolution(supportsTwoKResolution, supportsFourKResolution),
    )
  ) {
    return false;
  }

  if (resolution === "1920x1080") {
    return supportsTwoKResolution || supportsFourKResolution;
  }

  if (resolution === "2600x1600" || resolution === "2140x1440") {
    return supportsFourKResolution;
  }

  return true;
}

function getMinimumVisibleResolution(
  supportsTwoKResolution: boolean,
  supportsFourKResolution: boolean,
): AppResolution {
  if (supportsFourKResolution) {
    return "1600x900";
  }

  if (supportsTwoKResolution) {
    return "1400x800";
  }

  return "1270x720";
}

function getResolutionRank(resolution: AppResolution) {
  switch (resolution) {
    case "1270x720":
      return 0;
    case "1400x800":
      return 1;
    case "1600x900":
      return 2;
    case "1920x1080":
      return 3;
    case "2140x1440":
      return 4;
    case "2600x1600":
      return 5;
  }
}

export default SettingsModal;
