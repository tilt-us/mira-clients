import { ChevronDown } from "lucide-react";
import { useState } from "react";

export type InstallerLocale = "de" | "en";

type LanguageDropdownProps = {
  locale: InstallerLocale;
  t: (id: string) => string;
  onLocaleChange: (locale: InstallerLocale) => void;
};

const languages: Array<{
  countryMessage: string;
  flag: string;
  locale: InstallerLocale;
}> = [
  { countryMessage: "language-germany", flag: "🇩🇪", locale: "de" },
  { countryMessage: "language-united-states", flag: "🇺🇸", locale: "en" },
];

function LanguageDropdown({ locale, t, onLocaleChange }: LanguageDropdownProps) {
  const [open, setOpen] = useState(false);
  const activeLanguage = languages.find((language) => language.locale === locale) ?? languages[0];
  const activeCountry = t(activeLanguage.countryMessage);

  function selectLocale(nextLocale: InstallerLocale) {
    onLocaleChange(nextLocale);
    setOpen(false);
  }

  return (
    <div className="language-dropdown">
      <button
        aria-expanded={open}
        aria-label={activeCountry}
        className="language-dropdown-button"
        title={activeCountry}
        type="button"
        onClick={() => setOpen((currentOpen) => !currentOpen)}
      >
        <span aria-hidden="true">{activeLanguage.flag}</span>
        <ChevronDown size={14} />
      </button>

      {open ? (
        <div className="language-dropdown-menu" role="menu">
          {languages.map((language) => (
            <button
              key={language.locale}
              aria-label={t(language.countryMessage)}
              aria-pressed={language.locale === locale}
              title={t(language.countryMessage)}
              type="button"
              onClick={() => selectLocale(language.locale)}
            >
              <span aria-hidden="true">{language.flag}</span>
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}

export default LanguageDropdown;
