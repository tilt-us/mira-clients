# Mira Client

Desktop launcher and lobby client for Mira.

## Stack

- Tauri 2
- React 19
- TypeScript
- Vite 8

## Game Client Rendering

The Bevy game client uses `bevy_fontmesh` for overhead 3D text such as player
names and level digits on health bars. Bevy 0.18 provides UI text and `Text2d`,
but no built-in 3D mesh text; health-bar labels must be real 3D child entities
so they stay attached to the bar transform. The font asset is Roboto Bold at
`assets/ui/fonts/Roboto-Bold.ttf`.

## Commands

```bash
npm install
npm run generate:api
npm run dev:desktop
npm run staging:desktop
npm run prod:desktop
```

`npm run dev:desktop` and `npm run staging:desktop` start Tauri against the
official DEV and STAGING services respectively. `npm run prod:desktop` creates
the production bundle. See [the environment documentation](../../docs/environments.md)
for the available values and direct build commands.

## Backend API

Backend, website, and Keycloak URLs are selected centrally through `MIRA_ENV`.
Individual service URL overrides are not supported; this keeps desktop, browser,
and game-client builds on the same environment.

OpenAPI client code is generated into `src/api/generated`:

```bash
npm run generate:api
```

`npm run generate:api` reads and merges the selected environment's auth, live,
matchmaking, and chat OpenAPI documents. Set `MIRA_ENV` before generating when
the target is staging or production.

The services must expose those endpoints, for example with Springdoc OpenAPI.
If the backends are running somewhere else, override the input URLs:

```bash
OPENAPI_INPUTS=http://localhost:8080/v3/api-docs,http://localhost:8082/v3/api-docs,http://localhost:8083/v3/api-docs,http://localhost:8085/v3/api-docs npm run generate:api
```

Import generated endpoints through `src/api/client.ts` so the configured base
URL is applied in one place.

Email/password login uses Keycloak's password grant with
`VITE_KEYCLOAK_PASSWORD_CLIENT_ID`. Official desktop DEV/STAGING/PROD builds
use the public native OAuth client `mira-bevy`. Google, GitHub, and Discord login use
`VITE_KEYCLOAK_CLIENT_ID` with the authorization-code flow, PKCE, and the
provider hints `kc_idp_hint=google`, `kc_idp_hint=github`, and
`kc_idp_hint=discord`. The authorization-code client must allow the native
loopback redirect base `http://127.0.0.1`; each native login uses an
OS-assigned port and requests `http://127.0.0.1:<port>` without a trailing
slash. The password client must
have Direct Access Grants enabled. Identity provider callbacks are derived from
the selected environment's Keycloak issuer URL.

For `mira-bevy`, configure exactly `http://127.0.0.1` as the Keycloak **Valid
Redirect URI** for DEV, STAGING, and PROD. Do not use a wildcard, a port list,
or a Vite development URL for native OAuth.

## Linux Prerequisites

Tauri needs the native WebKitGTK development packages. If `npm run tauri dev` or
`cargo check --manifest-path src-tauri/Cargo.toml` fails with missing
`webkit2gtk-4.1` or `javascriptcoregtk-4.1`, install the Tauri Linux
prerequisites for your distribution first:

https://v2.tauri.app/start/prerequisites/

## Workspace Note

`src-tauri` is excluded from the root Cargo workspace. That keeps the existing
game/server `cargo check` from requiring desktop WebKit system libraries.
