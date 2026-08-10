# Development

This project uses `just` for common local development commands.

## Prerequisites

Install `just` once if it is not available:

```bash
cargo install just
```

The development recipes fetch the private dedicated-server repository through
SSH. Configure GitHub SSH access before running them. The checkout is cached at
`~/.cache/mira-game-server` by default; set `MIRA_GAME_SERVER_DIRECTORY` to use
another checkout or `MIRA_GAME_SERVER_REVISION` to select a different revision.
`just dev` builds an updated server before applying its 60-second readiness
timeout.

## Start Local Game Development

Start a local dedicated game server and a client connected to it:

```bash
just dev
```

The client is started with local development launch parameters and without an
auth token. This keeps gameplay validation on the dedicated server while
avoiding the production backend/auth flow.

Defaults:

```text
server UDP port: 5000
server control API port: 6000
player id: 1001
champion: lira
team: light
screen: window
```

Override parameters when needed:

```bash
just dev 5001 6001 1002 ignara dark window
```

The local recipes use the checked-in prototype champion catalog by default, so
they do not require the Java game service on port `8084`. To validate against
that service instead, start it and override the catalog source:

```bash
MIRA_DEVELOPMENT_CHAMPION_CATALOG=api just dev
```

## Split Server And Client

Run only the server:

```bash
just dev-server
```

Run only a client against an already running local server:

```bash
just dev-client
```

This is useful for testing server-authoritative player-vs-player interactions
with two local clients:

```bash
just dev-server
just dev-client 5000 6000 1001 lira light window
just dev-client 5000 6000 1002 ignara dark window
```

## Offline Preview

Run the old single-client preview without connecting to a server:

```bash
just offline-preview
```

Use this only for fast local visual checks. Server-authoritative behavior such
as damage, auto-attack combos, cooldown validation, range checks, and team checks
must be tested through `just dev` or split server/client commands.
