import { useState } from "react";
import type { AppLocale } from "../i18n";
import type { SettingsVision, Translate } from "../types/ui";

type SettingsModalProps = {
  accentColor: string;
  locale: AppLocale;
  onAccentColorChange: (accentColor: string) => void;
  onClose: () => void;
  onLocaleChange: (locale: AppLocale) => void;
  t: Translate;
  vision: SettingsVision;
};

function SettingsModal({
  accentColor,
  locale,
  onAccentColorChange,
  onClose,
  onLocaleChange,
  t,
  vision,
}: SettingsModalProps) {
  const [languageDropdownOpen, setLanguageDropdownOpen] = useState(false);
  const selectedLanguage =
    locale === "de" ? t("language-german") : t("language-english");
  const selectedFlag = locale === "de" ? "🇩🇪" : "🇬🇧";

  function handleLocaleSelect(nextLocale: AppLocale) {
    onLocaleChange(nextLocale);
    setLanguageDropdownOpen(false);
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
          <span>{t("settings-language")}</span>
          <div
            className="language-dropdown"
            onMouseDown={(event) => event.stopPropagation()}
          >
            <button
              aria-expanded={languageDropdownOpen}
              aria-haspopup="listbox"
              className="language-dropdown-trigger"
              type="button"
              onClick={() => setLanguageDropdownOpen((open) => !open)}
            >
              <span>{selectedFlag}</span>
              <span>{selectedLanguage}</span>
            </button>

            {languageDropdownOpen ? (
              <div className="language-dropdown-menu" role="listbox">
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

        <button className="secondary-button" type="button" onClick={onClose}>
          {t("settings-close")}
        </button>
      </div>
    </div>
  );
}

export default SettingsModal;
