# Mira Clients

Client workspace for the Mira desktop launcher, installer, Bevy game client, and
dedicated game server.

## Download

Linux installer:

- https://api.tilt-us.com/downloads/mira/game-sources/installer/releases/v1.0.0/mira-installer-1.0.0-linux-Mira-Installer.AppImage

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

## Keycloak URL Parameters

Keycloak can be themed with URL parameters:

- `accent`: Accent color as a hex value, with or without a URL-encoded `#`, for example `accent=%23f2c45b` or `accent=f2c45b`.
- `fontColor`: Text color on top of the accent color. Allowed values are `white` and `black`.
- `lang`: Login page language. Allowed values are `german` and `english`.
- `kc_locale` / `ui_locales`: Keycloak/OIDC locale hints, for example `de` or `en`.
- `hl`: Google language hint, for example `de` or `en`. This must be allowed as a forwarded query parameter in the Keycloak Google provider.
