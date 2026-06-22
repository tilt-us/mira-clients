import { useState } from "react";
import { Gamepad2, Monitor, Users } from "lucide-react";
import type { AppLocale } from "../i18n";
import type { AppResolution, ClientAnimation, GameScreenMode } from "../settings";
import type { SettingsVision, Translate } from "../types/ui";

type SettingsModalProps = {
  accentColor: string;
  allowFriendRequests: boolean;
  clientAnimation: ClientAnimation;
  gameScreenMode: GameScreenMode;
  locale: AppLocale;
  resolution: AppResolution;
  supportsFourKResolution: boolean;
  supportsTwoKResolution: boolean;
  onAccentColorChange: (accentColor: string) => void;
  onAllowFriendRequestsChange: (allowFriendRequests: boolean) => void;
  onClientAnimationChange: (clientAnimation: ClientAnimation) => void;
  onClose: () => void;
  onGameScreenModeChange: (gameScreenMode: GameScreenMode) => void;
  onLocaleChange: (locale: AppLocale) => void;
  onResolutionChange: (resolution: AppResolution) => void;
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
];

const clientAnimationOptions: Array<{
  label: string;
  value: ClientAnimation;
}> = [
  { label: "All", value: "all" },
  { label: "UI Elements", value: "ui-elements" },
  { label: "Images", value: "images" },
  { label: "None", value: "none" },
];

const gameScreenModeOptions: Array<{
  label: string;
  value: GameScreenMode;
}> = [
  { label: "Borderless", value: "borderless" },
  { label: "Fullscreen", value: "full" },
  { label: "Window", value: "window" },
];

function SettingsModal({
  accentColor,
  allowFriendRequests,
  clientAnimation,
  gameScreenMode,
  locale,
  resolution,
  supportsFourKResolution,
  supportsTwoKResolution,
  onAccentColorChange,
  onAllowFriendRequestsChange,
  onClientAnimationChange,
  onClose,
  onGameScreenModeChange,
  onLocaleChange,
  onResolutionChange,
  t,
  vision,
}: SettingsModalProps) {
  const [activeSettingsTab, setActiveSettingsTab] =
    useState<SettingsTab>("interface");
  const [clientAnimationDropdownOpen, setClientAnimationDropdownOpen] =
    useState(false);
  const [gameScreenModeDropdownOpen, setGameScreenModeDropdownOpen] =
    useState(false);
  const [languageDropdownOpen, setLanguageDropdownOpen] = useState(false);
  const [resolutionDropdownOpen, setResolutionDropdownOpen] = useState(false);
  const selectedLanguage =
    locale === "de" ? t("language-german") : t("language-english");
  const selectedFlag = locale === "de" ? "🇩🇪" : "🇬🇧";
  const socialTabAvailable = vision === "Vision.ALL";
  const visibleResolutionOptions = resolutionOptions.filter((option) => {
    if (option.value === "1920x1080") {
      return supportsTwoKResolution || supportsFourKResolution;
    }

    if (option.value === "2140x1440") {
      return supportsFourKResolution;
    }

    return true;
  });
  const selectedResolution =
    visibleResolutionOptions.find((option) => option.value === resolution)?.label ??
    "1600 x 900";
  const selectedClientAnimation =
    clientAnimationOptions.find((option) => option.value === clientAnimation)?.label ??
    "All";
  const selectedGameScreenMode =
    gameScreenModeOptions.find((option) => option.value === gameScreenMode)?.label ??
    "Borderless";

  function handleLocaleSelect(nextLocale: AppLocale) {
    onLocaleChange(nextLocale);
    setLanguageDropdownOpen(false);
  }

  function handleClientAnimationSelect(nextClientAnimation: ClientAnimation) {
    onClientAnimationChange(nextClientAnimation);
    setClientAnimationDropdownOpen(false);
  }

  function handleGameScreenModeSelect(nextGameScreenMode: GameScreenMode) {
    onGameScreenModeChange(nextGameScreenMode);
    setGameScreenModeDropdownOpen(false);
  }

  function handleResolutionSelect(nextResolution: AppResolution) {
    onResolutionChange(nextResolution);
    setResolutionDropdownOpen(false);
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

        <div className="settings-dialog-body">
          <nav className="settings-tabs" aria-label={t("settings-title")}>
            <button
              className={activeSettingsTab === "interface" ? "active" : ""}
              type="button"
              onClick={() => setActiveSettingsTab("interface")}
            >
              <Monitor size={17} />
              <span>{t("settings-interface")}</span>
            </button>
            <button
              className={activeSettingsTab === "game" ? "active" : ""}
              type="button"
              onClick={() => setActiveSettingsTab("game")}
            >
              <Gamepad2 size={17} />
              <span>{t("settings-game")}</span>
            </button>
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
                        setClientAnimationDropdownOpen(false);
                        setGameScreenModeDropdownOpen(false);
                        setLanguageDropdownOpen(false);
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

                <label className="settings-row">
                  <span>{t("settings-accent")}</span>
                  <input
                    aria-label={t("settings-accent")}
                    type="color"
                    value={accentColor}
                    onChange={(event) => onAccentColorChange(event.target.value)}
                  />
                </label>

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
                        setGameScreenModeDropdownOpen(false);
                        setResolutionDropdownOpen(false);
                        setLanguageDropdownOpen(false);
                        setClientAnimationDropdownOpen((open) => !open);
                      }}
                    >
                      <span>{selectedClientAnimation}</span>
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
                            <span>{option.label}</span>
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
                        setClientAnimationDropdownOpen(false);
                        setGameScreenModeDropdownOpen(false);
                        setResolutionDropdownOpen(false);
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

            {activeSettingsTab === "game" ? (
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
                      setClientAnimationDropdownOpen(false);
                      setLanguageDropdownOpen(false);
                      setResolutionDropdownOpen(false);
                      setGameScreenModeDropdownOpen((open) => !open);
                    }}
                  >
                    <span>{selectedGameScreenMode}</span>
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
                          <span>{option.label}</span>
                        </button>
                      ))}
                    </div>
                  ) : null}
                </div>
              </div>
            ) : null}

            {socialTabAvailable && activeSettingsTab === "social" ? (
              <label className="settings-checkbox-row">
                <span>{t("settings-allow-friend-request")}</span>
                <input
                  checked={allowFriendRequests}
                  type="checkbox"
                  onChange={(event) =>
                    onAllowFriendRequestsChange(event.target.checked)
                  }
                />
              </label>
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

export default SettingsModal;
