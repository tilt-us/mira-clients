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
`assets/fonts/Roboto-Bold.ttf`.

## Commands

```bash
npm install
npm run generate:api
npm run build
npm run tauri dev
npm run dev:desktop
npm run local:desktop
npm run prod:desktop
```

`npm run dev:desktop` starts the Tauri app with the built React UI and the
`api.tilt-us.com` desktop config, without starting a Vite web server. Use
`npm run local:desktop` for the same desktop start against localhost services.
Use `npm run prod:desktop` for the Tauri `deb` release bundle with the
production desktop config.

## Backend API

The desktop client reads runtime service addresses from `mira-client.toml` in
development. Adjust this file when the services are exposed through Docker on a
different host or port:

```toml
[services]
api_base_url = "http://localhost:8080"
live_api_base_url = "http://localhost:8082"
matchmaking_api_base_url = "http://localhost:8083"

[keycloak]
base_url = "http://localhost:8081"
realm = "mira"
client_id = "mira-bevy"
password_client_id = "mira-e2e"
```

The desktop client looks for the config in this order: `MIRA_CLIENT_CONFIG`, next
to the app executable, the current working directory, the app config directory,
and finally the repository root in development. Release bundles do not include a
TOML config file; production desktop builds use compiled defaults:

```toml
[services]
api_base_url = "https://api.tilt-us.com/auth"
live_api_base_url = "https://api.tilt-us.com/live"
matchmaking_api_base_url = "https://api.tilt-us.com/match"

[keycloak]
base_url = "https://api.tilt-us.com/keycloak"
```

The matching Keycloak service settings are:

```bash
KEYCLOAK_HOSTNAME=https://api.tilt-us.com/keycloak
KEYCLOAK_ISSUER_URI=https://api.tilt-us.com/keycloak/realms/mira
```

For browser/Vite development you can still override the addresses with:

```bash
VITE_API_BASE_URL=http://localhost:8080
VITE_LIVE_API_BASE_URL=http://localhost:8082
VITE_MATCHMAKING_API_BASE_URL=http://localhost:8083
VITE_KEYCLOAK_BASE_URL=http://localhost:8081
VITE_KEYCLOAK_REALM=mira
VITE_KEYCLOAK_CLIENT_ID=mira-bevy
VITE_KEYCLOAK_PASSWORD_CLIENT_ID=mira-e2e
```

OpenAPI client code is generated into `src/api/generated`:

```bash
npm run generate:api
```

By default, generation reads and merges:

- `https://api.tilt-us.com/auth/v3/api-docs`
- `https://api.tilt-us.com/live/v3/api-docs`
- `https://api.tilt-us.com/match/v3/api-docs`

The services must expose those endpoints, for example with Springdoc OpenAPI.
If the backends are running somewhere else, override the input URLs:

```bash
OPENAPI_INPUTS=http://localhost:8080/v3/api-docs,http://localhost:8082/v3/api-docs,http://localhost:8083/v3/api-docs npm run generate:api
```

To generate from a single OpenAPI document, use the legacy input override:

```bash
OPENAPI_INPUT=http://localhost:8080/v3/api-docs npm run generate:api
```

Import generated endpoints through `src/api/client.ts` so the configured base
URL is applied in one place.

Email/password login uses Keycloak's password grant with
`VITE_KEYCLOAK_PASSWORD_CLIENT_ID`. Google login uses
`VITE_KEYCLOAK_CLIENT_ID` with the authorization-code flow, PKCE, and
`kc_idp_hint=google`. The authorization-code client must allow the Tauri dev
redirect URL, for example `http://localhost:1420/*`. The password client must
have Direct Access Grants enabled.

## Linux Prerequisites

Tauri needs the native WebKitGTK development packages. If `npm run tauri dev` or
`cargo check --manifest-path src-tauri/Cargo.toml` fails with missing
`webkit2gtk-4.1` or `javascriptcoregtk-4.1`, install the Tauri Linux
prerequisites for your distribution first:

https://v2.tauri.app/start/prerequisites/

## Workspace Note

`src-tauri` is excluded from the root Cargo workspace. That keeps the existing
game/server `cargo check` from requiring desktop WebKit system libraries.
