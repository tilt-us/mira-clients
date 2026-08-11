import React from "react";
import ReactDOM from "react-dom/client";
import { loadRuntimeConfig } from "./runtimeConfig";
import { initializeUiAssets } from "./uiAssets";

async function bootstrap() {
  await loadRuntimeConfig();
  await initializeUiAssets();
  const { default: App } = await import("./App");

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

bootstrap().catch((error) => {
  console.error(error);

  const root = document.getElementById("root");
  if (root) {
    root.textContent =
      error instanceof Error
        ? `Client-Konfiguration konnte nicht geladen werden: ${error.message}`
        : "Client-Konfiguration konnte nicht geladen werden.";
  }
});
