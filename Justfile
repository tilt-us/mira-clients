set shell := ["bash", "-uc"]

default:
    @just --list

wait-for-server control_port="6000" server_pid="":
    #!/usr/bin/env bash
    set -euo pipefail

    server_ready_url="http://127.0.0.1:{{ control_port }}/ready"
    server_ready_timeout_seconds=60
    server_ready_poll_interval_seconds=0.2
    server_ready_connect_timeout_seconds=1
    server_ready_request_timeout_seconds=2
    server_ready_deadline=$((SECONDS + server_ready_timeout_seconds))

    until curl --fail --silent --connect-timeout "$server_ready_connect_timeout_seconds" --max-time "$server_ready_request_timeout_seconds" --output /dev/null "$server_ready_url" 2>/dev/null; do
        if [[ -n "{{ server_pid }}" ]] && ! kill -0 "{{ server_pid }}" 2>/dev/null; then
            echo "Game server exited before becoming ready." >&2
            exit 1
        fi
        if (( SECONDS >= server_ready_deadline )); then
            echo "Game server did not become ready within ${server_ready_timeout_seconds}s." >&2
            exit 1
        fi
        sleep "$server_ready_poll_interval_seconds"
    done

dev port="5000" control_port="6000" player_id="1001" champion="lira" team="light" screen="window":
    #!/usr/bin/env bash
    set -euo pipefail

    MIRA_DEVELOPMENT_CHAMPION_CATALOG="${MIRA_DEVELOPMENT_CHAMPION_CATALOG:-embedded}" \
        cargo run -p mira-game-server -- --port {{ port }} --control-port {{ control_port }} &
    server_pid=$!

    cleanup() {
        kill "$server_pid" 2>/dev/null || true
        wait "$server_pid" 2>/dev/null || true
    }
    trap cleanup EXIT INT TERM

    if ! just wait-for-server {{ control_port }} "$server_pid"; then
        wait "$server_pid" || true
        exit 1
    fi

    cargo run -p mira-game-client -- \
        --dev-preview \
        --match-id local-dev \
        --player-public-id {{ player_id }} \
        --champion {{ champion }} \
        --team {{ team }} \
        --server-host 127.0.0.1 \
        --port {{ port }} \
        --server-control-base-url http://127.0.0.1:{{ control_port }} \
        --stage Local \
        --screen {{ screen }}

dev-server port="5000" control_port="6000":
    MIRA_DEVELOPMENT_CHAMPION_CATALOG="${MIRA_DEVELOPMENT_CHAMPION_CATALOG:-embedded}" \
        cargo run -p mira-game-server -- --port {{ port }} --control-port {{ control_port }}

dev-client port="5000" control_port="6000" player_id="1001" champion="lira" team="light" screen="window":
    #!/usr/bin/env bash
    set -euo pipefail

    just wait-for-server {{ control_port }}

    cargo run -p mira-game-client -- \
        --dev-preview \
        --match-id local-dev \
        --player-public-id {{ player_id }} \
        --champion {{ champion }} \
        --team {{ team }} \
        --server-host 127.0.0.1 \
        --port {{ port }} \
        --server-control-base-url http://127.0.0.1:{{ control_port }} \
        --stage Local \
        --screen {{ screen }}

offline-preview champion="lira" team="light" screen="window":
    cargo run -p mira-game-client -- \
        --offline-preview \
        --champion {{ champion }} \
        --team {{ team }} \
        --screen {{ screen }}
