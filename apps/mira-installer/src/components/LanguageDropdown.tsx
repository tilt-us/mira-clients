import { ChevronDown } from "lucide-react";
import { useState } from "react";
import germanFlagUrl from "../../../../assets/icons/flags/de.svg";
import unitedStatesFlagUrl from "../../../../assets/icons/flags/us.svg";

export type InstallerLocale = "de" | "en";

type LanguageDropdownProps = {
  locale: InstallerLocale;
  t: (id: string) => string;
  onLocaleChange: (locale: InstallerLocale) => void;
};

const languages: Array<{
  countryMessage: string;
  flagUrl: string;
  locale: InstallerLocale;
}> = [
  { countryMessage: "language-germany", flagUrl: germanFlagUrl, locale: "de" },
  { countryMessage: "language-united-states", flagUrl: unitedStatesFlagUrl, locale: "en" },
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
        <img alt="" aria-hidden="true" src={activeLanguage.flagUrl} />
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
              <img alt="" aria-hidden="true" src={language.flagUrl} />
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}

export default LanguageDropdown;
