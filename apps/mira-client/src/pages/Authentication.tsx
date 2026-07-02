import { useEffect, useMemo, useState, type FormEvent } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogIn, UserPlus } from "lucide-react";
import {
  loginOptions,
  me,
  register,
  setApiAccessToken,
  type UserProfileResponse,
} from "../api/client";
import { API_BASE_URL, LIVE_API_BASE_URL } from "../api/config";
import { apiFetch } from "../api/http";
import {
  assertAccessTokenIssuer,
  completeRedirectLogin,
  getValidDesktopApiToken,
  getValidAccessToken,
  loginWithPassword,
  startDiscordLogin,
  startGithubLogin,
  startGoogleLogin,
  startKeycloakLogout,
} from "../auth/keycloak";
import { clearTokens, readTokens, saveTokens } from "../auth/storage";
import SettingsModal from "../components/SettingsModal";
import { useNotifications } from "../notifications";
import { useClientSettings } from "../settingsStore";
import Client from "./Client";
import {
  getProfileAvatarUrl,
  getProfileLevel,
  getProfileName,
  getProfileTagId,
} from "../utils/profile";

/**
 * Description
 * Available authentication form modes.
 */
type AuthMode = "login" | "register";

/**
 * Description
 * Lightweight loading state used by auth actions.
 */
type LoadState = "idle" | "loading";

type OAuthCallbackPayload = {
  url: string;
};

class ProfileLoadError extends Error {
  readonly status?: number;

  constructor(message: string, status?: number) {
    super(message);
    this.name = "ProfileLoadError";
    this.status = status;
  }
}

type LoadProfileOptions = {
  recoverDesktopConflict?: boolean;
};

/**
 * Description
 * Normalizes unknown API, Keycloak, and runtime errors into a user-facing message.
 *
 * Params
 * error - The unknown error value to inspect.
 * fallback - Message returned when no useful error text is available.
 *
 * Returns
 * A display-safe error message.
 */
function getErrorMessage(error: unknown, fallback = "Aktion fehlgeschlagen.") {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "string") {
    return error;
  }

  if (error && typeof error === "object") {
    const errorObject = error as {
      error?: unknown;
      error_description?: unknown;
      message?: unknown;
      status?: unknown;
    };

    for (const value of [
      errorObject.message,
      errorObject.error_description,
      errorObject.error,
      errorObject.status,
    ]) {
      if (typeof value === "string" && value.trim()) {
        return value;
      }

      if (typeof value === "number") {
        return value.toString();
      }
    }
  }

  return fallback;
}

function getOAuthErrorMessage(error: unknown, t: (id: string) => string) {
  if (error instanceof Error && error.message === "oauth_email_provider_conflict") {
    return t("auth-oauth-email-provider-conflict");
  }

  if (error === "oauth_email_provider_conflict") {
    return t("auth-oauth-email-provider-conflict");
  }

  return getErrorMessage(error, t("auth-action-failed"));
}

function getApiResultErrorMessage(
  error: unknown,
  response: Response | undefined,
  _request: Request | undefined,
  fallback: string,
) {
  const details = [];
  const errorMessage =
    response?.status === 401
      ? "Anmeldung ist abgelaufen. Bitte erneut einloggen."
      : response?.status === 409
        ? "User ist bereits eingeloggt."
        : response?.status === 502
          ? "Auth-Service ist nicht erreichbar."
          : getErrorMessage(error, "");

  if (response) {
    details.push(`${response.status} ${response.statusText}`.trim());
  }

  if (errorMessage) {
    details.push(errorMessage);
  }

  return details.length > 0 ? `${fallback} (${details.join(" - ")})` : fallback;
}

async function sendAuthServiceSessionRequest(
  accessToken: string,
  path: string,
  method = "POST",
) {
  try {
    const response = await apiFetch(`${API_BASE_URL}${path}`, {
      method,
      headers: {
        Authorization: `Bearer ${accessToken}`,
        "X-Device-Type": "Web",
      },
    });

    return response.ok;
  } catch {
    return false;
  }
}

async function releaseCurrentAuthServiceSession(accessToken: string) {
  for (const request of [
    { path: "/api/auth/session/logout" },
    { path: "/api/auth/logout" },
    { method: "DELETE", path: "/api/auth/session" },
  ]) {
    if (await sendAuthServiceSessionRequest(accessToken, request.path, request.method)) {
      return true;
    }
  }

  return false;
}

async function sendDesktopSessionHeartbeat() {
  const accessToken = await getValidDesktopApiToken();

  if (!accessToken) {
    return;
  }

  await Promise.all([
    apiFetch(`${API_BASE_URL}/api/auth/session/heartbeat`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${accessToken}`,
        "X-Device-Type": "Desktop",
      },
    }).catch(() => undefined),
    apiFetch(`${LIVE_API_BASE_URL}/api/live/heartbeat`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${accessToken}`,
        "X-Device-Type": "Desktop",
      },
    }).catch(() => undefined),
  ]);
}

function addUniqueToken(tokens: string[], token: string | undefined) {
  if (token && !tokens.includes(token)) {
    tokens.push(token);
  }
}

async function getStoredTokensForCleanup() {
  const cleanupTokens: string[] = [];

  try {
    addUniqueToken(cleanupTokens, await getValidDesktopApiToken());
  } catch {
    // Use stored tokens below when refresh or issuer validation fails.
  }

  const storedTokens = readTokens();
  addUniqueToken(cleanupTokens, storedTokens?.accessToken);
  addUniqueToken(cleanupTokens, storedTokens?.idToken);

  return cleanupTokens;
}

async function releaseStoredAuthServiceSessions() {
  const cleanupTokens = await getStoredTokensForCleanup();
  let released = false;

  for (const accessToken of cleanupTokens) {
    released = (await releaseCurrentAuthServiceSession(accessToken)) || released;
  }

  return released;
}

async function prepareNewLogin() {
  await releaseStoredAuthServiceSessions();

  clearTokens();
  setApiAccessToken(undefined);
}

async function releaseStoredAuthServiceSession() {
  return releaseStoredAuthServiceSessions();
}

function cleanupRejectedLoginSession() {
  clearTokens();
  setApiAccessToken(undefined);
}

/**
 * Description
 * Renders the Google brand mark used by the Google sign-in button.
 *
 * Returns
 * The Google icon SVG element.
 */
function GoogleIcon() {
  return (
    <svg
      aria-hidden="true"
      focusable="false"
      height="18"
      viewBox="0 0 18 18"
      width="18"
    >
      <path
        d="M17.64 9.2c0-.64-.06-1.25-.16-1.84H9v3.48h4.84a4.14 4.14 0 0 1-1.8 2.72v2.26h2.92c1.7-1.57 2.68-3.88 2.68-6.62Z"
        fill="#4285f4"
      />
      <path
        d="M9 18c2.43 0 4.47-.8 5.96-2.18l-2.92-2.26c-.8.54-1.84.86-3.04.86-2.35 0-4.34-1.58-5.05-3.72H.93v2.33A9 9 0 0 0 9 18Z"
        fill="#34a853"
      />
      <path
        d="M3.95 10.7A5.41 5.41 0 0 1 3.67 9c0-.59.1-1.16.28-1.7V4.97H.93A9 9 0 0 0 0 9c0 1.45.34 2.82.93 4.03l3.02-2.33Z"
        fill="#fbbc05"
      />
      <path
        d="M9 3.58c1.32 0 2.5.45 3.44 1.35l2.58-2.58A8.65 8.65 0 0 0 9 0 9 9 0 0 0 .93 4.97L3.95 7.3C4.66 5.16 6.65 3.58 9 3.58Z"
        fill="#ea4335"
      />
    </svg>
  );
}

function GitHubIcon() {
  return (
    <svg
      aria-hidden="true"
      focusable="false"
      height="18"
      viewBox="0 0 24 24"
      width="18"
    >
      <path
        d="M12 .5A11.5 11.5 0 0 0 8.36 22.9c.58.11.79-.25.79-.56v-2.02c-3.22.7-3.9-1.38-3.9-1.38-.53-1.34-1.29-1.7-1.29-1.7-1.05-.72.08-.71.08-.71 1.17.08 1.78 1.2 1.78 1.2 1.04 1.78 2.72 1.27 3.38.97.11-.75.41-1.27.74-1.56-2.57-.29-5.27-1.28-5.27-5.72 0-1.26.45-2.3 1.19-3.11-.12-.29-.52-1.47.11-3.07 0 0 .98-.31 3.18 1.19a11.05 11.05 0 0 1 5.8 0c2.2-1.5 3.17-1.19 3.17-1.19.64 1.6.24 2.78.12 3.07.74.81 1.19 1.85 1.19 3.11 0 4.45-2.7 5.43-5.28 5.72.42.36.79 1.07.79 2.16v3.04c0 .31.21.68.8.56A11.5 11.5 0 0 0 12 .5Z"
        fill="currentColor"
      />
    </svg>
  );
}

function DiscordIcon() {
  return (
    <svg
      aria-hidden="true"
      focusable="false"
      height="18"
      viewBox="0 0 24 24"
      width="18"
    >
      <path
        d="M19.54 5.23a18.88 18.88 0 0 0-4.68-1.46 12.96 12.96 0 0 0-.6 1.24 17.5 17.5 0 0 0-5.19 0 12.1 12.1 0 0 0-.61-1.24 18.8 18.8 0 0 0-4.68 1.46C.82 9.65.02 13.95.42 18.19a18.93 18.93 0 0 0 5.74 2.9c.46-.62.87-1.28 1.22-1.97-.67-.25-1.31-.56-1.92-.92l.47-.37c3.7 1.71 7.72 1.71 11.38 0l.47.37c-.61.36-1.25.67-1.92.92.35.69.76 1.35 1.22 1.97a18.87 18.87 0 0 0 5.74-2.9c.48-4.91-.82-9.17-3.28-12.96ZM8.09 15.59c-1.12 0-2.04-1.03-2.04-2.29 0-1.26.9-2.29 2.04-2.29 1.14 0 2.06 1.04 2.04 2.29 0 1.26-.9 2.29-2.04 2.29Zm7.53 0c-1.12 0-2.04-1.03-2.04-2.29 0-1.26.9-2.29 2.04-2.29 1.14 0 2.06 1.04 2.04 2.29 0 1.26-.9 2.29-2.04 2.29Z"
        fill="#5865f2"
      />
    </svg>
  );
}

/**
 * Description
 * Coordinates authentication, settings persistence, auth bootstrap, and the switch
 * between the auth forms and the signed-in client shell.
 *
 * Returns
 * The authentication or client view for the current session state.
 */
function Authentication() {
  const [authMode, setAuthMode] = useState<AuthMode>("login");
  const [providers, setProviders] = useState<string[]>([]);
  const [profile, setProfile] = useState<UserProfileResponse>();
  const [loginName, setLoginName] = useState("");
  const [loginPassword, setLoginPassword] = useState("");
  const [registerEmail, setRegisterEmail] = useState("");
  const [registerDisplayName, setRegisterDisplayName] = useState("");
  const [registerPassword, setRegisterPassword] = useState("");
  const [loadState, setLoadState] = useState<LoadState>("idle");
  const [oauthModalOpen, setOauthModalOpen] = useState(false);
  const [closeDialogOpen, setCloseDialogOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const {
    accentColor,
    backgroundChampion,
    chatPosition,
    clientAnimation,
    friendRequestPolicy,
    gameScreenMode,
    locale,
    resolution,
    supportsFourKResolution,
    supportsTwoKResolution,
    t,
    uiScale,
    setAccentColor,
    setBackgroundChampion,
    setChatPosition,
    setClientAnimation,
    setFriendRequestPolicy,
    setGameScreenMode,
    setLocale,
    setResolution,
    setUiScale,
  } = useClientSettings();
  const { notify } = useNotifications();

  const googleEnabled = useMemo(
    () => providers.length === 0 || providers.includes("google"),
    [providers],
  );
  const githubEnabled = useMemo(
    () => providers.length === 0 || providers.includes("github"),
    [providers],
  );
  const discordEnabled = useMemo(
    () => providers.length === 0 || providers.includes("discord"),
    [providers],
  );

  /**
   * Description
   * Loads the current user profile with the supplied API access token and stores it
   * as the active signed-in profile.
   *
   * Params
   * accessToken - API access token used for the profile request.
   */
  async function loadProfile(accessToken: string, options: LoadProfileOptions = {}) {
    const validAccessToken = (await getValidAccessToken()) ?? accessToken;

    assertAccessTokenIssuer(validAccessToken);
    setApiAccessToken(validAccessToken);
    let result = await me();

    if (result.error && result.response?.status === 409 && options.recoverDesktopConflict) {
      await releaseCurrentAuthServiceSession(validAccessToken);
      setApiAccessToken(validAccessToken);
      result = await me();
    }

    if (result.error) {
      if (result.response?.status === 401) {
        clearTokens();
        setApiAccessToken(undefined);
      }

      console.error("Profile request failed", {
        error: result.error,
        status: result.response?.status,
        statusText: result.response?.statusText,
        url: result.request?.url,
      });
      throw new ProfileLoadError(
        getApiResultErrorMessage(
          result.error,
          result.response,
          result.request,
          t("auth-profile-load-error"),
        ),
        result.response?.status,
      );
    }

    if (!result.data) {
      throw new Error(t("auth-profile-empty-error"));
    }

    setProfile(result.data);
  }

  useEffect(() => {
    let cancelled = false;

    /**
     * Description
     * Completes a pending OAuth redirect or restores stored tokens on app startup.
     */
    async function bootstrapAuth() {
      const currentUrl = new URL(window.location.href);
      const hasOAuthResponse =
        currentUrl.searchParams.has("code") ||
        currentUrl.searchParams.has("error") ||
        currentUrl.searchParams.has("error_description");

      try {
        const redirectTokens = await completeRedirectLogin();

        if (cancelled) {
          return;
        }

        const existingTokens = redirectTokens ?? readTokens();

        if (redirectTokens) {
          saveTokens(redirectTokens);
        }

        if (existingTokens?.accessToken) {
          await loadProfile(existingTokens.accessToken, {
            recoverDesktopConflict: !redirectTokens,
          });
        }

        if (redirectTokens) {
          notify({
            type: "info",
            message: t("auth-oauth-success"),
          });
        }
      } catch (caughtError) {
        clearTokens();
        setApiAccessToken(undefined);

        if (hasOAuthResponse && !cancelled) {
          notify({
            type: "error",
            message: getOAuthErrorMessage(caughtError, t),
          });
        }
      }
    }

    void bootstrapAuth();

    return () => {
      cancelled = true;
    };
  }, [notify, t]);

  useEffect(() => {
    if (!isTauri()) {
      return undefined;
    }

    let cancelled = false;
    const unlisteners: UnlistenFn[] = [];

    async function completeOAuthWindowLogin(callbackUrl: string) {
      setLoadState("loading");

      try {
        const tokens = await completeRedirectLogin(callbackUrl);

        if (cancelled || !tokens) {
          return;
        }

        saveTokens(tokens);
        await loadProfile(tokens.accessToken);

        if (!cancelled) {
          notify({
            type: "info",
            message: t("auth-oauth-success"),
          });
        }
      } catch (caughtError) {
        if (!cancelled) {
          cleanupRejectedLoginSession();
          notify({
            type: "error",
            message: getOAuthErrorMessage(caughtError, t),
          });
        }
      } finally {
        if (!cancelled) {
          setLoadState("idle");
          setOauthModalOpen(false);
        }
      }
    }

    async function subscribeOAuthWindowEvents() {
      unlisteners.push(
        await listen<OAuthCallbackPayload>("mira-oauth-callback", (event) => {
          if (!event.payload.url) {
            setLoadState("idle");
            setOauthModalOpen(false);
            return;
          }

          void completeOAuthWindowLogin(event.payload.url);
        }),
      );

      unlisteners.push(
        await listen("mira-oauth-closed", () => {
          setLoadState("idle");
          setOauthModalOpen(false);
        }),
      );
    }

    void subscribeOAuthWindowEvents();

    return () => {
      cancelled = true;

      for (const unlisten of unlisteners) {
        unlisten();
      }
    };
  }, [notify, t]);

  useEffect(() => {
    /**
     * Description
     * Intercepts titlebar close requests while signed in and opens the close dialog.
     *
     * Params
     * event - Cancelable close request event emitted by the titlebar.
     */
    function handleCloseRequest(event: Event) {
      if (!profile) {
        return;
      }

      event.preventDefault();
      setCloseDialogOpen(true);
    }

    window.addEventListener("mira:close-request", handleCloseRequest);

    return () => {
      window.removeEventListener("mira:close-request", handleCloseRequest);
    };
  }, [profile]);

  useEffect(() => {
    /**
     * Description
     * Opens the settings modal when the titlebar settings button emits a request.
     */
    function handleSettingsRequest() {
      setSettingsOpen(true);
    }

    window.addEventListener("mira:settings-request", handleSettingsRequest);

    return () => {
      window.removeEventListener("mira:settings-request", handleSettingsRequest);
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    /**
     * Description
     * Loads login provider options from the backend and updates the auth form.
     */
    async function loadLoginOptions() {
      const result = await loginOptions();

      if (!cancelled && result.data?.providers) {
        setProviders(result.data.providers);
      }
    }

    void loadLoginOptions();

    return () => {
      cancelled = true;
    };
  }, []);

  /**
   * Description
   * Handles email/password login submission, stores returned tokens, and loads the
   * signed-in profile.
   *
   * Params
   * event - Form submit event from the password login form.
   */
  async function handlePasswordLogin(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setLoadState("loading");

    try {
      await prepareNewLogin();
      const tokens = await loginWithPassword(loginName, loginPassword);
      saveTokens(tokens);
      await loadProfile(tokens.accessToken);
      notify({
        type: "info",
        message: t("auth-login-success"),
      });
    } catch (caughtError) {
      clearTokens();
      setApiAccessToken(undefined);
      notify({
        type: "error",
        message: getErrorMessage(caughtError, t("auth-action-failed")),
      });
    } finally {
      setLoadState("idle");
    }
  }

  /**
   * Description
   * Handles registration form submission and returns the user to the login tab on
   * successful account creation.
   *
   * Params
   * event - Form submit event from the registration form.
   */
  async function handleRegistration(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setLoadState("loading");

    const registrationRequest = {
      email: registerEmail.trim(),
      password: registerPassword,
      displayName: registerDisplayName.trim(),
    };

    if (!registrationRequest.displayName) {
      setLoadState("idle");
      notify({
        type: "warning",
        message: t("auth-display-name-required"),
      });
      return;
    }

    try {
      const result = await register({
        body: registrationRequest,
      });

      if (result.error) {
        throw result.error;
      }

      setAuthMode("login");
      setLoginName(registrationRequest.email);
      notify({
        type: "info",
        message: result.data?.message ?? t("auth-register-success"),
      });
    } catch (caughtError) {
      notify({
        type: "error",
        message: getErrorMessage(caughtError, t("auth-register-failed")),
      });
    } finally {
      setLoadState("idle");
    }
  }

  /**
   * Description
   * Starts the Google OAuth sign-in flow.
   */
  async function handleGoogleLogin() {
    setLoadState("loading");

    try {
      await prepareNewLogin();
      const result = await startGoogleLogin({ accentColor, locale });

      if (isTauri()) {
        if (result?.modal === false) {
          setLoadState("idle");
        } else {
          setOauthModalOpen(true);
        }
      }
    } catch (caughtError) {
      setLoadState("idle");
      setOauthModalOpen(false);
      notify({
        type: "error",
        message: getErrorMessage(caughtError, t("auth-action-failed")),
      });
    }
  }

  /**
   * Description
   * Starts the GitHub OAuth sign-in flow.
   */
  async function handleGithubLogin() {
    setLoadState("loading");

    try {
      await prepareNewLogin();
      const result = await startGithubLogin({ accentColor, locale });

      if (isTauri()) {
        if (result?.modal === false) {
          setLoadState("idle");
        } else {
          setOauthModalOpen(true);
        }
      }
    } catch (caughtError) {
      setLoadState("idle");
      setOauthModalOpen(false);
      notify({
        type: "error",
        message: getErrorMessage(caughtError, t("auth-action-failed")),
      });
    }
  }

  /**
   * Description
   * Starts the Discord OAuth sign-in flow.
   */
  async function handleDiscordLogin() {
    setLoadState("loading");

    try {
      await prepareNewLogin();
      const result = await startDiscordLogin({ accentColor, locale });

      if (isTauri()) {
        if (result?.modal === false) {
          setLoadState("idle");
        } else {
          setOauthModalOpen(true);
        }
      }
    } catch (caughtError) {
      setLoadState("idle");
      setOauthModalOpen(false);
      notify({
        type: "error",
        message: getErrorMessage(caughtError, t("auth-action-failed")),
      });
    }
  }

  /**
   * Description
   * Clears auth state, stored tokens, and open dialogs, returning the user to auth.
   */
  async function handleLogout() {
    setLoadState("loading");
    setCloseDialogOpen(false);
    setSettingsOpen(false);
    setOauthModalOpen(false);

    try {
      await releaseStoredAuthServiceSession();
      await startKeycloakLogout();
    } catch (caughtError) {
      notify({
        type: "error",
        message: getErrorMessage(caughtError, t("auth-action-failed")),
      });
    } finally {
      clearTokens();
      setApiAccessToken(undefined);
      setProfile(undefined);
      setLoginPassword("");
      setLoadState("idle");
    }
  }

  /**
   * Description
   * Closes the app window through Tauri when available, with a browser fallback.
   */
  async function handleQuit() {
    await releaseStoredAuthServiceSession();

    if (isTauri()) {
      await getCurrentWindow().close();
      return;
    }

    window.close();
  }

  useEffect(() => {
    const className = "mira-client-authenticated";

    document.documentElement.classList.toggle(className, Boolean(profile));
    document.body.classList.toggle(className, Boolean(profile));

    return () => {
      document.documentElement.classList.remove(className);
      document.body.classList.remove(className);
    };
  }, [profile]);

  useEffect(() => {
    if (!profile) {
      return;
    }

    void sendDesktopSessionHeartbeat();
    const intervalId = window.setInterval(() => {
      void sendDesktopSessionHeartbeat();
    }, 3_000);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [profile]);

  const busy = loadState === "loading";
  const clientVersion = `Client v${__CLIENT_VERSION__}`;
  const profileName = profile ? getProfileName(profile) : undefined;
  const profileLevel = profile ? getProfileLevel(profile) : 0;
  const profileTagId = profile ? getProfileTagId(profile) : undefined;
  const profileAvatarUrl = profile
    ? getProfileAvatarUrl(profile, readTokens()?.accessToken)
    : undefined;

  return (
    <main className={profile ? "app-shell app-shell-authenticated" : "app-shell"}>
      {oauthModalOpen ? <div className="oauth-modal-backdrop" aria-hidden="true" /> : null}

      {profile && profileName ? (
        <Client
          accentColor={accentColor}
          backgroundChampion={backgroundChampion}
          chatPosition={chatPosition}
          clientAnimation={clientAnimation}
          friendRequestPolicy={friendRequestPolicy}
          gameScreenMode={gameScreenMode}
          closeDialogOpen={closeDialogOpen}
          locale={locale}
          profileAvatarUrl={profileAvatarUrl}
          profileLevel={profileLevel}
          profileName={profileName}
          profilePublicId={profile.publicId}
          profileTagId={profileTagId}
          resolution={resolution}
          settingsOpen={settingsOpen}
          supportsFourKResolution={supportsFourKResolution}
          supportsTwoKResolution={supportsTwoKResolution}
          t={t}
          uiScale={uiScale}
          onAccentColorChange={setAccentColor}
          onBackgroundChampionChange={setBackgroundChampion}
          onChatPositionChange={setChatPosition}
          onClientAnimationChange={setClientAnimation}
          onFriendRequestPolicyChange={setFriendRequestPolicy}
          onGameScreenModeChange={setGameScreenMode}
          onCloseDialogClose={() => setCloseDialogOpen(false)}
          onLocaleChange={setLocale}
          onLogout={handleLogout}
          onQuit={handleQuit}
          onResolutionChange={setResolution}
          onSettingsClose={() => setSettingsOpen(false)}
          onUiScaleChange={setUiScale}
        />
      ) : (
        <section className="login-window" aria-labelledby="login-title">
          <div className="login-header">
            <div className="brand-mark">M</div>
            <div>
              <h1 id="login-title">{t("auth-title")}</h1>
              <p>{t("auth-subtitle")}</p>
            </div>
          </div>

          <div className="mode-tabs" role="tablist" aria-label="Auth mode">
            <button
              aria-selected={authMode === "login"}
              className={authMode === "login" ? "active" : ""}
              role="tab"
              type="button"
              onClick={() => setAuthMode("login")}
            >
              {t("auth-login-tab")}
            </button>
            <button
              aria-selected={authMode === "register"}
              className={authMode === "register" ? "active" : ""}
              role="tab"
              type="button"
              onClick={() => setAuthMode("register")}
            >
              {t("auth-register-tab")}
            </button>
          </div>

          {authMode === "login" ? (
            <div className="auth-stack">
              <form className="login-form" onSubmit={handlePasswordLogin}>
                <label>
                  {t("auth-username")}
                  <input
                    autoComplete="username"
                    placeholder={t("auth-username-placeholder")}
                    required
                    value={loginName}
                    onChange={(event) => setLoginName(event.target.value)}
                  />
                </label>

                <label>
                  {t("auth-password")}
                  <input
                    autoComplete="current-password"
                    placeholder={t("auth-password-placeholder")}
                    required
                    type="password"
                    value={loginPassword}
                    onChange={(event) => setLoginPassword(event.target.value)}
                  />
                </label>

                <button className="login-button" disabled={busy} type="submit">
                  <LogIn size={18} />
                  {busy ? t("auth-login-loading") : t("auth-login-button")}
                </button>

                <div className="provider-actions" aria-label="OAuth providers">
                  <button
                    aria-label={t("auth-google")}
                    className="provider-button"
                    disabled={busy || !googleEnabled}
                    title={t("auth-google")}
                    type="button"
                    onClick={handleGoogleLogin}
                  >
                    <GoogleIcon />
                  </button>

                  <button
                    aria-label={t("auth-github")}
                    className="provider-button"
                    disabled={busy || !githubEnabled}
                    title={t("auth-github")}
                    type="button"
                    onClick={handleGithubLogin}
                  >
                    <GitHubIcon />
                  </button>

                  <button
                    aria-label={t("auth-discord")}
                    className="provider-button"
                    disabled={busy || !discordEnabled}
                    title={t("auth-discord")}
                    type="button"
                    onClick={handleDiscordLogin}
                  >
                    <DiscordIcon />
                  </button>
                </div>
              </form>
            </div>
          ) : (
            <form className="login-form" onSubmit={handleRegistration}>
              <label>
                {t("auth-display-name")}
                <input
                  autoComplete="name"
                  placeholder={t("auth-display-name-placeholder")}
                  required
                  value={registerDisplayName}
                  onChange={(event) => setRegisterDisplayName(event.target.value)}
                />
              </label>

              <label>
                {t("auth-email")}
                <input
                  autoComplete="email"
                  placeholder={t("auth-username-placeholder")}
                  required
                  type="email"
                  value={registerEmail}
                  onChange={(event) => setRegisterEmail(event.target.value)}
                />
              </label>

              <label>
                {t("auth-password")}
                <input
                  autoComplete="new-password"
                  minLength={8}
                  placeholder={t("auth-new-password-placeholder")}
                  required
                  type="password"
                  value={registerPassword}
                  onChange={(event) => setRegisterPassword(event.target.value)}
                />
              </label>

              <button className="login-button" disabled={busy} type="submit">
                <UserPlus size={18} />
                {busy ? t("auth-register-loading") : t("auth-register-button")}
              </button>
            </form>
          )}

          <p className="runtime-info">{clientVersion}</p>

        </section>
      )}

      {!profile && settingsOpen ? (
        <SettingsModal
          accentColor={accentColor}
          backgroundChampion={backgroundChampion}
          chatPosition={chatPosition}
          clientAnimation={clientAnimation}
          friendRequestPolicy={friendRequestPolicy}
          gameScreenMode={gameScreenMode}
          locale={locale}
          resolution={resolution}
          supportsFourKResolution={supportsFourKResolution}
          supportsTwoKResolution={supportsTwoKResolution}
          t={t}
          uiScale={uiScale}
          vision="Vision.Auth"
          onAccentColorChange={setAccentColor}
          onBackgroundChampionChange={setBackgroundChampion}
          onChatPositionChange={setChatPosition}
          onClientAnimationChange={setClientAnimation}
          onFriendRequestPolicyChange={setFriendRequestPolicy}
          onGameScreenModeChange={setGameScreenMode}
          onClose={() => setSettingsOpen(false)}
          onLocaleChange={setLocale}
          onResolutionChange={setResolution}
          onUiScaleChange={setUiScale}
        />
      ) : null}
    </main>
  );
}

export default Authentication;
