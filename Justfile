set shell := ["bash", "-uc"]

default:
    @just --list

dev port="5000" control_port="6000" player_id="1001" champion="lira" team="light" screen="window":
    #!/usr/bin/env bash
    set -euo pipefail

    cargo run -p mira-game-server -- --port {{port}} --control-port {{control_port}} &
    server_pid=$!

    cleanup() {
        kill "$server_pid" 2>/dev/null || true
        wait "$server_pid" 2>/dev/null || true
    }
    trap cleanup EXIT INT TERM

    sleep 1

    cargo run -p mira-game-client -- \
        --dev-preview \
        --match-id local-dev \
        --player-public-id {{player_id}} \
        --champion {{champion}} \
        --team {{team}} \
        --server-host 127.0.0.1 \
        --port {{port}} \
        --server-control-base-url http://127.0.0.1:{{control_port}} \
        --stage Local \
        --screen {{screen}}

dev-server port="5000" control_port="6000":
    cargo run -p mira-game-server -- --port {{port}} --control-port {{control_port}}

dev-client port="5000" control_port="6000" player_id="1001" champion="lira" team="light" screen="window":
    cargo run -p mira-game-client -- \
        --dev-preview \
        --match-id local-dev \
        --player-public-id {{player_id}} \
        --champion {{champion}} \
        --team {{team}} \
        --server-host 127.0.0.1 \
        --port {{port}} \
        --server-control-base-url http://127.0.0.1:{{control_port}} \
        --stage Local \
        --screen {{screen}}

offline-preview champion="lira" team="light" screen="window":
    cargo run -p mira-game-client -- \
        --offline-preview \
        --champion {{champion}} \
        --team {{team}} \
        --screen {{screen}}
