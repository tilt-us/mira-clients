# Mira Clients

Client workspace for the Mira desktop launcher, installer, Bevy game client, and
dedicated game server.

## Download

The website downloads the Mira Installer only. The installer downloads the
desktop client and Bevy game-client binary. The desktop client owns game
content and verifies it before it starts the game.

Current download pointers:

| Environment | Latest manifest |
| --- | --- |
| Development | `https://downloads.tilt-us.com/dev/latest.json` |
| Staging | `https://downloads.tilt-us.com/staging/latest.json` |
| Production | `https://downloads.tilt-us.com/latest.json` |

`latest.json` points to the installer, runtime, and content manifests. The
website consumes `installer/manifest.json`; the installer consumes the runtime
manifest; the desktop client consumes the content manifest. Garage contains
only current stable object names. Versioned artifacts and release history
remain on [GitHub Releases](https://github.com/tilt-us/mira-clients/releases).

## Download Publishing

The release workflow creates a current content archive from `assets/` and
uploads it with the installer and runtime artifacts to the `downloads.tilt-us.com`
Garage bucket. It uploads artifacts first, manifests second, and `latest.json`
last. The workflow requires these CI secrets and does not embed them in any
application:

- `GARAGE_ENDPOINT`
- `GARAGE_ACCESS_KEY_ID`
- `GARAGE_SECRET_ACCESS_KEY`
- `GARAGE_REGION`
- `GARAGE_BUCKET`

## Development Start

Install the desktop client dependencies first:

```bash
cd apps/mira-client
npm install
```

Start the desktop client against local services:

```bash
npm run local:desktop
```

Start the desktop client against the dev API:

```bash
npm run dev:desktop
```

Start the Bevy game client directly in development preview mode with the visible
Lira model:

```bash
cargo run -p mira-game-client -- --dev-preview
```

`--dev-preview` is only meant for development builds. It starts the local map
and mechanics preview with Lira spawned locally, without connecting to a match
server.

## CI Access to Private Dependencies

The `mira-game-client` CI job reads `tilt-us/mira-game-api` through a GitHub
App. Configure the following organization-level values and grant them to each
repository that consumes the dependency:

- Secret `MIRA_CI_APP_CLIENT_ID`: the GitHub App client ID.
- Secret `MIRA_CI_APP_PRIVATE_KEY`: the complete PEM private key for the app.

Install the app for `tilt-us/mira-game-api` with access limited to that
repository and `Contents: Read` permission. The workflow creates a short-lived
token scoped to that repository for each job.

## Keycloak URL Parameters

Keycloak can be themed with URL parameters:

- `accent`: Accent color as a hex value, with or without a URL-encoded `#`, for example `accent=%23f2c45b` or `accent=f2c45b`.
- `fontColor`: Text color on top of the accent color. Allowed values are `white` and `black`.
- `lang`: Login page language. Allowed values are `german` and `english`.
- `kc_locale` / `ui_locales`: Keycloak/OIDC locale hints, for example `de` or `en`.
- `hl`: Google language hint, for example `de` or `en`. This must be allowed as a forwarded query parameter in the Keycloak Google provider.
