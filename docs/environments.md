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

The installer reads `runtimeManifestUrl` from this pointer. The desktop client
reads `contentManifestUrl`, installs the verified content archive atomically in
its application-data directory, and blocks game launch unless
`assets/index.html` is current.

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
