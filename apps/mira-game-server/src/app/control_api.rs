use super::match_manifest::ServerMatchManifest;
use serde::Serialize;
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug, Clone, Copy)]
pub struct ServerControlApiSettings {
    pub listen_addr: SocketAddr,
}

impl Default for ServerControlApiSettings {
    fn default() -> Self {
        Self {
            listen_addr: SocketAddr::new(IpAddr::from([127, 0, 0, 1]), 6000),
        }
    }
}

#[derive(Clone)]
struct ControlState {
    match_id: Option<String>,
    allowed_players: HashSet<u64>,
    ready_players: Arc<Mutex<HashSet<u64>>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DisplayReadyResponse {
    match_id: String,
    player_public_id: u64,
    display_ready: bool,
    ready_players: usize,
    total_players: usize,
    ready_player_ids: Vec<u64>,
    all_clients_ready: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoadingScreenResponse {
    match_id: String,
    ready_players: usize,
    total_players: usize,
    ready_player_ids: Vec<u64>,
    can_close: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorResponse {
    error: String,
}

pub fn spawn(settings: ServerControlApiSettings, manifest: ServerMatchManifest) {
    let state = ControlState {
        match_id: manifest.match_id.clone(),
        allowed_players: manifest.player_ids().into_iter().collect(),
        ready_players: Arc::new(Mutex::new(HashSet::new())),
    };

    thread::spawn(move || {
        let listener = match TcpListener::bind(settings.listen_addr) {
            Ok(listener) => listener,
            Err(error) => {
                eprintln!(
                    "Failed to bind game-server control API at {}: {}",
                    settings.listen_addr, error
                );
                return;
            }
        };

        println!(
            "Game-server control API listening at http://{}",
            settings.listen_addr
        );

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => handle_connection(stream, &state),
                Err(error) => eprintln!("Game-server control API connection failed: {error}"),
            }
        }
    });
}

fn handle_connection(mut stream: TcpStream, state: &ControlState) {
    let Ok(request) = read_request(&stream) else {
        write_json(
            &mut stream,
            400,
            &ErrorResponse {
                error: "Invalid HTTP request.".to_string(),
            },
        );
        return;
    };

    let response = route(&request.method, &request.path, state);
    match response {
        RouteResponse::Json(status, body) => write_raw_json(&mut stream, status, &body),
        RouteResponse::NoContent => write_no_content(&mut stream),
    }
}

fn route(method: &str, path: &str, state: &ControlState) -> RouteResponse {
    if method == "OPTIONS" {
        return RouteResponse::NoContent;
    }

    let segments = path
        .split('?')
        .next()
        .unwrap_or(path)
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    if segments.len() == 6
        && method == "POST"
        && segments[0] == "api"
        && segments[1] == "matches"
        && segments[3] == "players"
        && segments[5] == "display-ready"
    {
        let match_id = segments[2];
        let player_public_id = match segments[4].parse::<u64>() {
            Ok(player_public_id) => player_public_id,
            Err(_) => return json_error(400, "Invalid player public id."),
        };
        return mark_display_ready(match_id, player_public_id, state);
    }

    if segments.len() == 4
        && method == "GET"
        && segments[0] == "api"
        && segments[1] == "matches"
        && segments[3] == "loading-screen"
    {
        return loading_screen(segments[2], state);
    }

    json_error(404, "Endpoint was not found.")
}

fn mark_display_ready(
    match_id: &str,
    player_public_id: u64,
    state: &ControlState,
) -> RouteResponse {
    if !match_matches(match_id, state) {
        return json_error(404, "Match was not found on this server.");
    }
    if !state.allowed_players.is_empty() && !state.allowed_players.contains(&player_public_id) {
        return json_error(403, "Player is not allowed on this match server.");
    }

    let mut ready_players = state
        .ready_players
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    ready_players.insert(player_public_id);
    let ready_count = ready_players.len();
    let ready_player_ids = ready_player_ids(&ready_players);
    let total_players = total_players(state, ready_count);
    let response = DisplayReadyResponse {
        match_id: match_id.to_string(),
        player_public_id,
        display_ready: true,
        ready_players: ready_count,
        total_players,
        ready_player_ids,
        all_clients_ready: ready_count >= total_players,
    };
    json_response(200, &response)
}

fn loading_screen(match_id: &str, state: &ControlState) -> RouteResponse {
    if !match_matches(match_id, state) {
        return json_error(404, "Match was not found on this server.");
    }

    let ready_players = state
        .ready_players
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let ready_count = ready_players.len();
    let ready_player_ids = ready_player_ids(&ready_players);
    let total_players = total_players(state, ready_count);
    let response = LoadingScreenResponse {
        match_id: match_id.to_string(),
        ready_players: ready_count,
        total_players,
        ready_player_ids,
        can_close: ready_count >= total_players,
    };
    json_response(200, &response)
}

fn ready_player_ids(ready_players: &HashSet<u64>) -> Vec<u64> {
    let mut player_ids = ready_players.iter().copied().collect::<Vec<_>>();
    player_ids.sort_unstable();
    player_ids
}

fn match_matches(match_id: &str, state: &ControlState) -> bool {
    state
        .match_id
        .as_deref()
        .map(|expected| expected == match_id)
        .unwrap_or(true)
}

fn total_players(state: &ControlState, ready_count: usize) -> usize {
    if state.allowed_players.is_empty() {
        ready_count.max(1)
    } else {
        state.allowed_players.len()
    }
}

fn json_response<T: Serialize>(status: u16, body: &T) -> RouteResponse {
    RouteResponse::Json(
        status,
        serde_json::to_string(body)
            .unwrap_or_else(|_| "{\"error\":\"Serialization failed.\"}".to_string()),
    )
}

fn json_error(status: u16, error: &str) -> RouteResponse {
    json_response(
        status,
        &ErrorResponse {
            error: error.to_string(),
        },
    )
}

fn read_request(stream: &TcpStream) -> Result<HttpRequest, std::io::Error> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();
    Ok(HttpRequest { method, path })
}

fn write_json<T: Serialize>(stream: &mut TcpStream, status: u16, body: &T) {
    let body = serde_json::to_string(body)
        .unwrap_or_else(|_| "{\"error\":\"Serialization failed.\"}".to_string());
    write_raw_json(stream, status, &body);
}

fn write_raw_json(stream: &mut TcpStream, status: u16, body: &str) {
    let status_text = status_text(status);
    let _ = write!(
        stream,
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET,POST,OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type,Authorization\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
}

fn write_no_content(stream: &mut TcpStream) {
    let _ = write!(
        stream,
        "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET,POST,OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type,Authorization\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
}

fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        _ => "Error",
    }
}

struct HttpRequest {
    method: String,
    path: String,
}

enum RouteResponse {
    Json(u16, String),
    NoContent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_manifest_player_ready() {
        let state = ControlState {
            match_id: Some("match-1".to_string()),
            allowed_players: HashSet::from([1, 2]),
            ready_players: Arc::new(Mutex::new(HashSet::new())),
        };

        let response = route(
            "POST",
            "/api/matches/match-1/players/1/display-ready",
            &state,
        );

        assert!(matches!(response, RouteResponse::Json(200, _)));
        assert_eq!(state.ready_players.lock().unwrap().len(), 1);
    }

    #[test]
    fn rejects_player_outside_manifest() {
        let state = ControlState {
            match_id: Some("match-1".to_string()),
            allowed_players: HashSet::from([1]),
            ready_players: Arc::new(Mutex::new(HashSet::new())),
        };

        let response = route(
            "POST",
            "/api/matches/match-1/players/2/display-ready",
            &state,
        );

        assert!(matches!(response, RouteResponse::Json(403, _)));
    }
}
