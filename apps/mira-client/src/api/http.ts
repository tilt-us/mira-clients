import { isTauri } from "@tauri-apps/api/core";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";

export function getApiTransport() {
  if (runsInTauriLikeShell()) {
    return "tauri-http";
  }

  return "browser-fetch";
}

export function getClientDeviceType() {
  return runsInTauriLikeShell() ? "Desktop" : "Web";
}

function runsInTauriLikeShell() {
  const location = window.location;

  return (
    isTauri() ||
    "isTauri" in globalThis ||
    "__TAURI_INTERNALS__" in globalThis ||
    "__TAURI__" in globalThis ||
    location.protocol === "tauri:" ||
    location.hostname === "tauri.localhost" ||
    location.origin === "null"
  );
}

export const apiFetch: typeof fetch = (input, init) => {
  if (runsInTauriLikeShell()) {
    return tauriFetchWithCleanRequest(input, init);
  }

  return fetch(input, init);
};

async function tauriFetchWithCleanRequest(input: RequestInfo | URL, init?: RequestInit) {
  if (input instanceof Request) {
    const headers = cleanHeaders(input.headers, init?.headers);
    headers.set("origin", "");
    const body = await readRequestBody(input);

    return tauriFetch(input.url, {
      ...init,
      body,
      headers: Object.fromEntries(headers.entries()),
      method: init?.method ?? input.method,
      signal: init?.signal ?? input.signal,
    });
  }

  const headers = cleanHeaders(init?.headers);
  headers.set("origin", "");

  return tauriFetch(input, {
    ...init,
    headers: Object.fromEntries(headers.entries()),
  });
}

async function readRequestBody(request: Request) {
  if (request.method === "GET" || request.method === "HEAD") {
    return undefined;
  }

  const body = await request.clone().arrayBuffer();

  if (body.byteLength === 0) {
    return undefined;
  }

  return body;
}

function cleanHeaders(...headersList: Array<HeadersInit | undefined>) {
  const headers = new Headers();

  for (const headersInit of headersList) {
    if (!headersInit) {
      continue;
    }

    new Headers(headersInit).forEach((value, key) => {
      headers.set(key, value);
    });
  }

  for (const header of [
    "origin",
    "referer",
    "sec-fetch-dest",
    "sec-fetch-mode",
    "sec-fetch-site",
    "sec-fetch-user",
  ]) {
    headers.delete(header);
  }

  return headers;
}
