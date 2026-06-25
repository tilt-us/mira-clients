import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { executableDir } from "@tauri-apps/api/path";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import "./App.css";
import TitleBar from "./components/TitleBar";
import LanguageDropdown, { type InstallerLocale } from "./components/LanguageDropdown";
import { translate } from "./i18n";

type InstallStatus = "idle" | "running" | "done" | "error";

type InstallProgress = {
  labelKey: string;
  progress: number;
};

type InstallResult = {
  launcherPath: string;
};

function App() {
  const [locale, setLocale] = useState<InstallerLocale>("de");
  const [installationPath, setInstallationPath] = useState("");
  const [installStatus, setInstallStatus] = useState<InstallStatus>("idle");
  const [installProgress, setInstallProgress] = useState<InstallProgress>({
    labelKey: "install-status-idle",
    progress: 0,
  });
  const [installErrorCode, setInstallErrorCode] = useState("");
  const [showInstallConfirm, setShowInstallConfirm] = useState(false);

  useEffect(() => {
    function preventContextMenu(event: MouseEvent) {
      event.preventDefault();
    }

    window.addEventListener("contextmenu", preventContextMenu);

    return () => {
      window.removeEventListener("contextmenu", preventContextMenu);
    };
  }, []);

  useEffect(() => {
    async function loadDefaultPath() {
      try {
        setInstallationPath(await executableDir());
      } catch {
        setInstallationPath(".");
      }
    }

    void loadDefaultPath();
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    void listen<InstallProgress>("installer:progress", (event) => {
      setInstallProgress(event.payload);

      if (event.payload.labelKey === "install-status-done") {
        setInstallStatus("done");
      }
    }).then((cleanup) => {
      unlisten = cleanup;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  async function selectInstallationPath() {
    const selectedPath = await open({
      defaultPath: installationPath || undefined,
      directory: true,
      multiple: false,
      title: translate(locale, "installation-folder-title"),
    });

    if (typeof selectedPath === "string") {
      setInstallationPath(selectedPath);
    }
  }

  async function install() {
    if (installStatus === "running") {
      return;
    }

    let hasContent = false;

    try {
      hasContent = await invoke<boolean>("path_has_content", {
        installPath: installationPath || ".",
      });
    } catch {
      setInstallStatus("error");
      setInstallErrorCode("465");
      setInstallProgress({
        labelKey: "install-status-error",
        progress: 0,
      });
      return;
    }

    if (hasContent) {
      setShowInstallConfirm(true);
      return;
    }

    await runInstall();
  }

  async function runInstall() {
    setInstallErrorCode("");
    setInstallStatus("running");
    setInstallProgress({
      labelKey: "install-status-platform",
      progress: 0,
    });

    try {
      const result = await invoke<InstallResult>("install_game", {
        installPath: installationPath || ".",
      });
      setInstallStatus("done");
      setInstallProgress({
        labelKey: "install-status-done",
        progress: 1,
      });
      await invoke("launch_installed_launcher", {
        launcherPath: result.launcherPath,
      });
      await getCurrentWindow().close();
    } catch (error) {
      setInstallStatus("error");
      setInstallErrorCode(getInstallerErrorCode(error));
      setInstallProgress({
        labelKey: "install-status-error",
        progress: 0,
      });
    }
  }

  const showProgress = installStatus !== "idle";
  const progressPercent = Math.round(installProgress.progress * 100);
  const installErrorMessage = installErrorCode
    ? translate(locale, `installer-error-${installErrorCode}`)
    : "";
  const confirmMessage = translate(locale, "confirm-install-message", {
    folder: installationPath || ".",
  });

  return (
    <>
      <TitleBar t={(id) => translate(locale, id)}>
        <LanguageDropdown
          locale={locale}
          t={(id) => translate(locale, id)}
          onLocaleChange={setLocale}
        />
      </TitleBar>
      <main className="installer-shell">
        <section className="installer-stage" aria-label="Mira Installer">
          <button
            aria-busy={installStatus === "running"}
            className="installer-install-button"
            data-animated="true"
            disabled={installStatus === "running"}
            type="button"
            onClick={install}
          >
            <span>{translate(locale, "install-button")}</span>
          </button>
          {showProgress ? (
            <div className="installer-progress-block" title={installErrorMessage || undefined}>
              <div className="installer-progress-label">
                <span>{translate(locale, installProgress.labelKey)}</span>
                <span>{progressPercent}%</span>
              </div>
              <div
                aria-label={translate(locale, installProgress.labelKey)}
                aria-valuemax={100}
                aria-valuemin={0}
                aria-valuenow={progressPercent}
                className="installer-progress-track"
                role="progressbar"
              >
                <span style={{ width: `${progressPercent}%` }} />
              </div>
              {installStatus === "error" && installErrorMessage ? (
                <p className="installer-progress-error">{installErrorMessage}</p>
              ) : null}
            </div>
          ) : (
            <p className="installer-path-row">
              <span>{translate(locale, "installation-path-label")}</span>
              <button
                className="installer-path-link"
                title={installationPath}
                type="button"
                onClick={selectInstallationPath}
              >
                {installationPath}
              </button>
            </p>
          )}
        </section>
        {showInstallConfirm ? (
          <div className="installer-modal-backdrop" role="presentation">
            <section
              aria-labelledby="installer-confirm-title"
              aria-modal="true"
              className="installer-confirm-modal"
              role="dialog"
            >
              <h2 id="installer-confirm-title">{translate(locale, "confirm-install-title")}</h2>
              <p>{confirmMessage}</p>
              <div className="installer-confirm-actions">
                <button
                  className="installer-confirm-button"
                  type="button"
                  onClick={() => setShowInstallConfirm(false)}
                >
                  {translate(locale, "confirm-install-no")}
                </button>
                <button
                  className="installer-confirm-button installer-confirm-button-primary"
                  type="button"
                  onClick={() => {
                    setShowInstallConfirm(false);
                    void runInstall();
                  }}
                >
                  {translate(locale, "confirm-install-yes")}
                </button>
              </div>
            </section>
          </div>
        ) : null}
      </main>
    </>
  );
}

function getInstallerErrorCode(error: unknown) {
  const text = String(error);

  if (text.includes("19145")) {
    return "19145";
  }

  return "465";
}

export default App;
