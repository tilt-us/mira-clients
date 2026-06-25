import { FluentBundle, FluentResource } from "@fluent/bundle";
import type { InstallerLocale } from "./components/LanguageDropdown";

const messages: Record<InstallerLocale, string> = {
  de: `
install-button = Installieren
installation-path-label = Installationspfad:
installation-folder-title = Installationsordner auswählen
install-status-idle = Bereit
install-status-platform = System wird erkannt
install-status-manifest = Release-Manifest wird geladen
install-status-download-client = Client wird heruntergeladen
install-status-download-game = Game-Client wird heruntergeladen
install-status-download-assets = Assets werden heruntergeladen
install-status-unzip-game = Game-Client wird entpackt
install-status-unzip-assets = Assets werden entpackt
install-status-finalize = Installation wird vorbereitet
install-status-done = Installation abgeschlossen
install-status-error = Installation fehlgeschlagen
installer-error-465 = 465 - Konnte Spieledaten nicht beziehen, versuche es später erneut!
installer-error-19145 = 19145 - Server hat keine Antwort gegeben, versuche es später erneut!
confirm-install-title = Ordner hat Inhalt
confirm-install-message = Sicher in { $folder } installieren? Der Ordner hat Inhalt!
confirm-install-no = Nein
confirm-install-yes = Ja
language-germany = Deutschland
language-united-states = Vereinigte Staaten
window-minimize = Minimieren
window-close = Schliessen
`,
  en: `
install-button = Install
installation-path-label = Installation Path:
installation-folder-title = Select installation folder
install-status-idle = Ready
install-status-platform = Detecting system
install-status-manifest = Loading release manifest
install-status-download-client = Downloading client
install-status-download-game = Downloading game client
install-status-download-assets = Downloading assets
install-status-unzip-game = Extracting game client
install-status-unzip-assets = Extracting assets
install-status-finalize = Preparing installation
install-status-done = Installation complete
install-status-error = Installation failed
installer-error-465 = 465 - Could not retrieve game data, please try again later!
installer-error-19145 = 19145 - The server did not respond, please try again later!
confirm-install-title = Folder has content
confirm-install-message = Are you sure you want to install in { $folder }? The folder has content!
confirm-install-no = No
confirm-install-yes = Yes
language-germany = Germany
language-united-states = United States
window-minimize = Minimize
window-close = Close
`,
};

const bundles = new Map<InstallerLocale, FluentBundle>();

function getBundle(locale: InstallerLocale) {
  const cachedBundle = bundles.get(locale);
  if (cachedBundle) {
    return cachedBundle;
  }

  const bundle = new FluentBundle(locale);
  bundle.addResource(new FluentResource(messages[locale]));
  bundles.set(locale, bundle);
  return bundle;
}

export function translate(
  locale: InstallerLocale,
  id: string,
  args?: Record<string, string>,
) {
  const bundle = getBundle(locale);
  const message = bundle.getMessage(id);

  if (!message?.value) {
    return id;
  }

  return bundle.formatPattern(message.value, args);
}
