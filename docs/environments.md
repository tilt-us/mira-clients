# Client Environments

Mira client builds select one public deployment environment with `MIRA_ENV`:

| `MIRA_ENV` | Website | Services API | Keycloak issuer |
| --- | --- | --- | --- |
| `dev` | `https://dev.tilt-us.com` | `https://dev-api.tilt-us.com` | `https://dev-api.tilt-us.com/keycloak/realms/mira` |
| `staging` | `https://staging.tilt-us.com` | `https://staging-api.tilt-us.com` | `https://staging-api.tilt-us.com/keycloak/realms/mira` |
| `prod` | `https://tilt-us.com` | `https://api.tilt-us.com` | `https://api.tilt-us.com/keycloak/realms/mira` |

The URLs are defined once in `mira-environments.json`. Service endpoints such
as `/auth`, `/live`, `/match`, `/game`, and `/chat` are derived from the selected
services API URL.

Download pointers are selected independently from `MIRA_ENV` and never fall
back to production:

| `MIRA_ENV` | Download `latest.json` |
| --- | --- |
| `dev` | `https://downloads.tilt-us.com/dev/latest.json` |
| `staging` | `https://downloads.tilt-us.com/staging/latest.json` |
| `prod` | `https://downloads.tilt-us.com/latest.json` |

The installer reads the runtime and content manifests, then installs the
desktop client, game executable, and `assets/ui` at the installation root. The
desktop client reads the same content manifest and atomically installs or
updates `assets/game`; it blocks game launch until that directory is current.

Local debug builds default to `MIRA_ENV=dev`. Release builds require an explicit
valid value and fail for a missing or invalid `MIRA_ENV`.

## Build Examples

Build the Bevy game client from the repository root:

```bash
MIRA_ENV=dev cargo build -p mira-game-client
MIRA_ENV=staging cargo build --release -p mira-game-client
MIRA_ENV=prod cargo build --release -p mira-game-client
```

Build the desktop client from `apps/mira-client`:

```bash
MIRA_ENV=dev npm run tauri -- build --bundles deb --ci
MIRA_ENV=staging npm run tauri -- build --bundles deb --ci
MIRA_ENV=prod npm run tauri -- build --bundles deb --ci
```
