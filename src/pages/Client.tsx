import CloseDialog from "../components/CloseDialog";
import SettingsModal from "../components/SettingsModal";
import Sidebar from "../components/Sidebar";
import type { AppLocale } from "../i18n";
import type { PresenceStatus, Translate } from "../types/ui";

type ClientProps = {
  accentColor: string;
  closeDialogOpen: boolean;
  error?: string;
  locale: AppLocale;
  onAccentColorChange: (accentColor: string) => void;
  onCloseDialogClose: () => void;
  onLocaleChange: (locale: AppLocale) => void;
  onLogout: () => void;
  onQuit: () => void;
  onSettingsClose: () => void;
  profileName: string;
  settingsOpen: boolean;
  t: Translate;
};

function Client({
  accentColor,
  closeDialogOpen,
  error,
  locale,
  onAccentColorChange,
  onCloseDialogClose,
  onLocaleChange,
  onLogout,
  onQuit,
  onSettingsClose,
  profileName,
  settingsOpen,
  t,
}: ClientProps) {
  const presenceStatus: PresenceStatus = "online";

  return (
    <>
      <Sidebar presenceStatus={presenceStatus} profileName={profileName} t={t} />

      <section className="dashboard-panel" aria-label="Dashboard">
        {error ? <p className="message error">{error}</p> : null}
      </section>

      {closeDialogOpen ? (
        <CloseDialog
          t={t}
          onClose={onCloseDialogClose}
          onLogout={onLogout}
          onQuit={onQuit}
        />
      ) : null}

      {settingsOpen ? (
        <SettingsModal
          accentColor={accentColor}
          locale={locale}
          t={t}
          vision="Vision.ALL"
          onAccentColorChange={onAccentColorChange}
          onClose={onSettingsClose}
          onLocaleChange={onLocaleChange}
        />
      ) : null}
    </>
  );
}

export default Client;
