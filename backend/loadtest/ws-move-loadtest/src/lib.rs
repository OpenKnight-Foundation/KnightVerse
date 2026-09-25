//! Load-test harness for KnightVerse's WebSocket move-submission endpoint.
//!
//! The backend unit/integration tests cover the WebSocket move path one
//! connection at a time. This harness exercises it the way production does:
//! `--games` concurrent games, each with two player sockets (the ones allowed
//! to submit moves) plus `--spectators-per-game` read-only sockets, submitting
//! moves at a configurable rate and recording the broadcast leg
//! (client send → server fan-out → peer receive) as latency percentiles plus a
//! delivery/error rate.
//!
//! Layout:
//! * [`parse_args`] — dependency-free CLI parsing (no clap, so the harness can
//!   stay outside `backend/`'s workspace).
//! * [`run`] — the load loop.
//! * [`summarize`] / [`format_report`] — percentile maths and the report.
//! * [`mock`] — an in-process, protocol-compatible mock of the endpoint used
//!   by `--self-test` so the harness can be verified without Postgres/Redis.

pub mod mock;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use uuid::Uuid;

/// JWT secret used by `--self-test` when none is supplied. The mock server
/// mints and validates with the same value, so nothing is trusted implicitly.
pub const DEFAULT_SELF_TEST_SECRET: &str = "knightverse-ws-loadtest-secret";

/// Endpoint path appended to the base URL, mirroring
/// `backend/modules/api/src/server.rs`.
pub const WS_GAME_PATH: &str = "/v1/ws/game";

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsSink = futures_util::stream::SplitSink<WsStream, Message>;
type WsSource = futures_util::stream::SplitStream<WsStream>;

pub const HELP_TEXT: &str = "\
ws-move-loadtest — load-test the KnightVerse WebSocket move-submission endpoint

USAGE:
    ws-move-loadtest [OPTIONS]

OPTIONS:
    --url <URL>                  Base WebSocket URL of the backend
                                 [default: ws://127.0.0.1:8080]
    --games <N>                  Concurrent games to open          [default: 10]
    --players-per-game <N>       Player sockets per game           [default: 2]
    --spectators-per-game <N>    Spectator sockets per game        [default: 0]
    --moves <N>                  Moves submitted per game          [default: 50]
    --rate <R>                   Moves per second per game (0 = as fast as
                                 possible)                         [default: 10]
    --token <JWT>                Reuse one access token for every connection.
                                 Default: mint one per connection from
                                 --jwt-secret / $JWT_SECRET / $JWT_SECRET_KEY.
    --jwt-secret <SECRET>        HS256 secret used to mint access tokens
    --connect-timeout-ms <MS>    WebSocket handshake timeout        [default: 5000]
    --drain-timeout-ms <MS>      How long to wait for the last broadcast
                                 of each game                        [default: 2000]
    --max-error-rate <F>         Exit non-zero when the measured error rate
                                 exceeds this fraction              [default: 0.0]
    --json <PATH>                Also write the report as JSON to PATH
    --self-test                  Run against an in-process mock of the endpoint
                                 instead of a real backend (no Postgres/Redis)
    -h, --help                   Print this help
    -V, --version                Print the harness version

WHAT IS MEASURED:
    Each game opens two player sockets (index 0 submits, index 1 measures) and
    any number of spectator sockets. Every move is tagged with a unique `san`
    marker; the harness records the time from writing the frame to the server
    until each other socket in the same game observes the broadcast, and it
    counts any observation that never arrives as a delivery miss.
";

// ---------------------------------------------------------------------------
// CLI parsing
// ---------------------------------------------------------------------------

/// Fully resolved settings for one load-test run.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadTestConfig {
    pub url: String,
    pub games: usize,
    pub players_per_game: usize,
    pub spectators_per_game: usize,
    pub moves: usize,
    pub rate_per_second: f64,
    pub connect_timeout: Duration,
    pub drain_timeout: Duration,
    pub jwt_secret: Option<String>,
    pub token: Option<String>,
}

impl Default for LoadTestConfig {
    fn default() -> Self {
        Self {
            url: "ws://127.0.0.1:8080".to_string(),
            games: 10,
            players_per_game: 2,
            spectators_per_game: 0,
            moves: 50,
            rate_per_second: 10.0,
            connect_timeout: Duration::from_millis(5000),
            drain_timeout: Duration::from_millis(2000),
            jwt_secret: None,
            token: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CliOptions {
    pub config: LoadTestConfig,
    pub json_path: Option<PathBuf>,
    pub self_test: bool,
    pub max_error_rate: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Run(Box<CliOptions>),
    Help,
    Version,
}

fn parse_usize(flag: &str, value: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|_| format!("{} expects a non-negative integer, got '{}'", flag, value))
}

fn parse_f64(flag: &str, value: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .map_err(|_| format!("{} expects a number, got '{}'", flag, value))
}

/// Parse CLI arguments (without the program name).
///
/// Unknown flags are rejected rather than ignored so a typo in a CI invocation
/// surfaces immediately instead of silently benchmarking the defaults.
pub fn parse_args(args: &[String]) -> Result<Command, String> {
    let mut config = LoadTestConfig::default();
    let mut json_path = None;
    let mut self_test = false;
    let mut max_error_rate = 0.0_f64;

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].clone();
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if arg.starts_with("--") => (f.to_string(), Some(v.to_string())),
            _ => (arg.clone(), None),
        };

        let mut next_value = |flag: &str| -> Result<String, String> {
            if let Some(v) = inline.clone() {
                return Ok(v);
            }
            i += 1;
            args.get(i)
                .cloned()
                .ok_or_else(|| format!("{} requires a value", flag))
        };

        match flag.as_str() {
            "-h" | "--help" => return Ok(Command::Help),
            "-V" | "--version" => return Ok(Command::Version),
            "--url" => config.url = next_value("--url")?,
            "--games" => config.games = parse_usize("--games", &next_value("--games")?)?,
            "--players-per-game" | "--players" => {
                config.players_per_game =
                    parse_usize("--players-per-game", &next_value("--players-per-game")?)?
            }
            "--spectators-per-game" | "--spectators" => {
                config.spectators_per_game = parse_usize(
                    "--spectators-per-game",
                    &next_value("--spectators-per-game")?,
                )?
            }
            "--moves" => config.moves = parse_usize("--moves", &next_value("--moves")?)?,
            "--rate" => {
                config.rate_per_second = parse_f64("--rate", &next_value("--rate")?)?;
                if config.rate_per_second < 0.0 {
                    return Err("--rate must not be negative".to_string());
                }
            }
            "--token" => config.token = Some(next_value("--token")?),
            "--jwt-secret" => config.jwt_secret = Some(next_value("--jwt-secret")?),
            "--connect-timeout-ms" => {
                let ms = parse_usize("--connect-timeout-ms", &next_value("--connect-timeout-ms")?)?;
                config.connect_timeout = Duration::from_millis(ms as u64);
            }
            "--drain-timeout-ms" => {
                let ms = parse_usize("--drain-timeout-ms", &next_value("--drain-timeout-ms")?)?;
                config.drain_timeout = Duration::from_millis(ms as u64);
            }
            "--max-error-rate" => {
                max_error_rate = parse_f64("--max-error-rate", &next_value("--max-error-rate")?)?;
            }
            "--json" => json_path = Some(PathBuf::from(next_value("--json")?)),
            "--self-test" => self_test = true,
            other => return Err(format!("unknown argument '{}'", other)),
        }

        i += 1;
    }

    if config.games == 0 {
        return Err("--games must be at least 1".to_string());
    }
    if config.players_per_game == 0 {
        return Err("--players-per-game must be at least 1".to_string());
    }
    if config.players_per_game < 2 && config.spectators_per_game == 0 && config.moves > 0 {
        return Err(
            "--players-per-game must be at least 2 (one sender, one observer) unless \
             --spectators-per-game is set"
                .to_string(),
        );
    }

    if config.jwt_secret.is_none() && config.token.is_none() {
        config.jwt_secret = std::env::var("JWT_SECRET")
            .or_else(|_| std::env::var("JWT_SECRET_KEY"))
            .ok()
            .filter(|s| !s.trim().is_empty());
    }

    Ok(Command::Run(Box::new(CliOptions {
        config,
        json_path,
        self_test,
        max_error_rate,
    })))
}

// ---------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LatencyStats {
    pub count: usize,
    pub min_ms: f64,
    pub mean_ms: f64,
    pub p50_ms: f64,
    pub p90_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
}

/// Linear-interpolated percentile over an already-sorted slice.
pub fn percentile(sorted: &[f64], pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let rank = (pct / 100.0).clamp(0.0, 1.0) * (sorted.len() as f64 - 1.0);
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    if lower == upper {
        sorted[lower]
    } else {
        let weight = rank - lower as f64;
        sorted[lower] + (sorted[upper] - sorted[lower]) * weight
    }
}

pub fn summarize(mut samples: Vec<f64>) -> LatencyStats {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if samples.is_empty() {
        return LatencyStats::default();
    }
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    LatencyStats {
        count: samples.len(),
        min_ms: samples[0],
        mean_ms: mean,
        p50_ms: percentile(&samples, 50.0),
        p90_ms: percentile(&samples, 90.0),
        p95_ms: percentile(&samples, 95.0),
        p99_ms: percentile(&samples, 99.0),
        max_ms: *samples.last().unwrap_or(&0.0),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ErrorBreakdown {
    /// Sockets that never completed the WebSocket handshake.
    pub connection_failures: u64,
    /// Games that could not reach two live player sockets.
    pub insufficient_connections: u64,
    /// Moves the harness could not write to the socket.
    pub send_failures: u64,
    /// Broadcasts a peer or spectator never observed inside the drain window.
    pub delivery_misses: u64,
    /// `Error` frames sent back by the server.
    pub server_error_frames: u64,
    /// Inbound frames that were not valid JSON.
    pub parse_failures: u64,
}

impl ErrorBreakdown {
    pub fn total(&self) -> u64 {
        self.connection_failures
            + self.insufficient_connections
            + self.send_failures
            + self.delivery_misses
            + self.server_error_frames
            + self.parse_failures
    }

    fn merge(&mut self, other: &ErrorBreakdown) {
        self.connection_failures += other.connection_failures;
        self.insufficient_connections += other.insufficient_connections;
        self.send_failures += other.send_failures;
        self.delivery_misses += other.delivery_misses;
        self.server_error_frames += other.server_error_frames;
        self.parse_failures += other.parse_failures;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub run_id: String,
    pub url: String,
    pub games: usize,
    pub players_per_game: usize,
    pub spectators_per_game: usize,
    pub moves_per_game: usize,
    pub rate_per_game_per_sec: f64,
    pub duration_secs: f64,
    pub moves_sent: u64,
    pub moves_per_sec_achieved: f64,
    pub expected_deliveries: u64,
    pub received_deliveries: u64,
    pub delivery_ratio: f64,
    pub operations_total: u64,
    pub errors_total: u64,
    pub error_rate: f64,
    pub connect_latency_ms: LatencyStats,
    pub opponent_broadcast_latency_ms: LatencyStats,
    pub spectator_broadcast_latency_ms: LatencyStats,
    pub errors: ErrorBreakdown,
    pub error_details: Vec<String>,
}

impl Report {
    pub fn exceeds_error_budget(&self, max_error_rate: f64) -> bool {
        self.error_rate > max_error_rate
    }
}

// ---------------------------------------------------------------------------
// Load loop
// ---------------------------------------------------------------------------

/// Marker used to surface a server-sent `Error` frame through the observation
/// channel without colliding with a move marker.
const SERVER_ERROR_MARKER: &str = "__server_error__";

#[derive(Debug)]
struct Arrival {
    conn_index: usize,
    marker: String,
    at: Instant,
}

#[derive(Debug, Default)]
struct GameOutcome {
    connect_samples: Vec<f64>,
    opponent_samples: Vec<f64>,
    spectator_samples: Vec<f64>,
    moves_sent: u64,
    expected_deliveries: u64,
    received_deliveries: u64,
    errors: ErrorBreakdown,
    error_details: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Claims {
    sub: String,
    user_id: i32,
    player_id: String,
    username: String,
    exp: usize,
    iat: usize,
    jti: String,
    token_type: &'static str,
}

fn mint_token(secret: &str, user_id: i32, player_id: &str) -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        user_id,
        player_id: player_id.to_string(),
        username: format!("loadtest-{}", user_id),
        exp: now + 3600,
        iat: now,
        jti: Uuid::new_v4().to_string(),
        // Matches `security::jwt::TokenType` (serde rename_all = "lowercase"),
        // which `ws_route` enforces before accepting the socket.
        token_type: "access",
    };
    jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| format!("failed to mint access token: {e}"))
}

fn token_for(
    config: &LoadTestConfig,
    game_index: usize,
    conn_index: usize,
) -> Result<String, String> {
    if let Some(token) = &config.token {
        return Ok(token.clone());
    }
    let secret = config
        .jwt_secret
        .as_deref()
        .ok_or_else(|| "no access token: pass --token or --jwt-secret".to_string())?;
    let user_id = (game_index * 1000 + conn_index) as i32 + 1;
    mint_token(secret, user_id, &Uuid::new_v4().to_string())
}

async fn connect(
    config: &LoadTestConfig,
    game_id: &str,
    token: &str,
    is_spectator: bool,
) -> Result<(WsSink, WsSource, f64), String> {
    let mut url = format!(
        "{}{}/{}",
        config.url.trim_end_matches('/'),
        WS_GAME_PATH,
        game_id
    );
    if is_spectator {
        url.push_str("?role=spectator");
    }

    let mut request = url
        .clone()
        .into_client_request()
        .map_err(|e| format!("invalid URL '{}': {}", url, e))?;
    let header = HeaderValue::from_str(&format!("Bearer {}", token))
        .map_err(|e| format!("invalid token header: {}", e))?;
    request.headers_mut().insert("Authorization", header);

    let started = Instant::now();
    let (stream, _response) = tokio::time::timeout(config.connect_timeout, connect_async(request))
        .await
        .map_err(|_| format!("handshake timed out after {:?}", config.connect_timeout))?
        .map_err(|e| format!("handshake failed: {}", e))?;
    let connect_ms = started.elapsed().as_secs_f64() * 1000.0;

    let (sink, source) = stream.split();
    Ok((sink, source, connect_ms))
}

/// Extract the `san` marker of an inbound `Move` frame, if it is one.
pub fn move_marker(text: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    if value.get("type")?.as_str()? != "Move" {
        return None;
    }
    value
        .get("payload")?
        .get("san")?
        .as_str()
        .map(str::to_string)
}

async fn read_loop(
    mut source: WsSource,
    conn_index: usize,
    tx: mpsc::UnboundedSender<Arrival>,
    parse_failures: Arc<AtomicU64>,
) {
    while let Some(message) = source.next().await {
        let Ok(message) = message else { return };
        let text = match message {
            Message::Text(text) => text,
            Message::Close(_) => return,
            _ => continue,
        };

        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            parse_failures.fetch_add(1, Ordering::Relaxed);
            continue;
        };

        let marker = match value.get("type").and_then(|t| t.as_str()) {
            Some("Move") => value
                .get("payload")
                .and_then(|payload| payload.get("san"))
                .and_then(|san| san.as_str())
                .map(str::to_string),
            Some("Error") => Some(SERVER_ERROR_MARKER.to_string()),
            _ => None,
        };

        if let Some(marker) = marker {
            if tx
                .send(Arrival {
                    conn_index,
                    marker,
                    at: Instant::now(),
                })
                .is_err()
            {
                return;
            }
        }
    }
}

async fn run_game(game_index: usize, config: &LoadTestConfig) -> GameOutcome {
    let mut outcome = GameOutcome::default();
    let game_id = Uuid::new_v4().to_string();
    let total_connections = config.players_per_game + config.spectators_per_game;
    let (tx, mut rx) = mpsc::unbounded_channel::<Arrival>();
    let parse_failures = Arc::new(AtomicU64::new(0));

    let mut writers: Vec<(usize, WsSink)> = Vec::new();
    let mut readers: Vec<tokio::task::JoinHandle<()>> = Vec::new();
    let mut player_connections = 0usize;
    let mut spectator_connections = 0usize;

    for conn_index in 0..total_connections {
        let is_spectator = conn_index >= config.players_per_game;
        let token = match token_for(config, game_index, conn_index) {
            Ok(token) => token,
            Err(err) => {
                outcome.errors.connection_failures += 1;
                outcome.error_details.push(err);
                continue;
            }
        };

        match connect(config, &game_id, &token, is_spectator).await {
            Ok((sink, source, connect_ms)) => {
                outcome.connect_samples.push(connect_ms);
                if is_spectator {
                    spectator_connections += 1;
                } else {
                    player_connections += 1;
                }
                readers.push(tokio::spawn(read_loop(
                    source,
                    conn_index,
                    tx.clone(),
                    parse_failures.clone(),
                )));
                writers.push((conn_index, sink));
            }
            Err(err) => {
                outcome.errors.connection_failures += 1;
                outcome.error_details.push(format!(
                    "game {} connection {}: {}",
                    game_id, conn_index, err
                ));
            }
        }
    }

    if player_connections < 2 {
        outcome.errors.insufficient_connections += 1;
        outcome.error_details.push(format!(
            "game {}: only {} player socket(s) connected, need 2",
            game_id, player_connections
        ));
        for reader in readers {
            reader.abort();
        }
        return outcome;
    }

    let sender_pos = match writers
        .iter()
        .position(|(index, _)| *index < config.players_per_game)
    {
        Some(pos) => pos,
        None => {
            outcome.errors.insufficient_connections += 1;
            for reader in readers {
                reader.abort();
            }
            return outcome;
        }
    };
    let sender_conn_index = writers[sender_pos].0;

    let interval = if config.rate_per_second > 0.0 {
        Duration::from_secs_f64(1.0 / config.rate_per_second)
    } else {
        Duration::ZERO
    };

    let mut send_times: HashMap<String, Instant> = HashMap::new();
    for move_index in 0..config.moves {
        if !interval.is_zero() && move_index > 0 {
            tokio::time::sleep(interval).await;
        }

        let marker = format!("lt-{}-{}", game_index, move_index);
        let frame = serde_json::json!({
            "type": "Move",
            "payload": {
                "from": "e2",
                "to": "e4",
                "san": marker,
                "fen": "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1"
            }
        })
        .to_string();

        send_times.insert(marker.clone(), Instant::now());
        if writers[sender_pos]
            .1
            .send(Message::Text(frame))
            .await
            .is_err()
        {
            outcome.errors.send_failures += 1;
            send_times.remove(&marker);
            break;
        }
        outcome.moves_sent += 1;
    }

    let peers = player_connections.saturating_sub(1) + spectator_connections;
    outcome.expected_deliveries = outcome.moves_sent * peers as u64;

    // Wait for the fan-out of the last move. Every socket in the game sees the
    // broadcast (LobbyState fans out to all recipients, the sender included),
    // so the sender's own echo is observed and ignored for the latency stats.
    let deadline = Instant::now() + config.drain_timeout;
    let mut observed: HashMap<(usize, String), Instant> = HashMap::new();
    let mut relevant = 0u64;
    let mut server_errors = 0u64;
    while relevant < outcome.expected_deliveries {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(arrival)) => {
                if arrival.marker == SERVER_ERROR_MARKER {
                    server_errors += 1;
                    continue;
                }
                let key = (arrival.conn_index, arrival.marker.clone());
                let is_sender_echo = arrival.conn_index == sender_conn_index;
                let is_tracked_move = send_times.contains_key(&arrival.marker);
                if observed.insert(key, arrival.at).is_none() && !is_sender_echo && is_tracked_move
                {
                    relevant += 1;
                }
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }

    for ((conn_index, marker), at) in &observed {
        if *conn_index == sender_conn_index {
            continue;
        }
        let Some(sent_at) = send_times.get(marker) else {
            continue;
        };
        let latency_ms = at.duration_since(*sent_at).as_secs_f64() * 1000.0;
        if *conn_index < config.players_per_game {
            outcome.opponent_samples.push(latency_ms);
        } else {
            outcome.spectator_samples.push(latency_ms);
        }
    }
    outcome.received_deliveries =
        (outcome.opponent_samples.len() + outcome.spectator_samples.len()) as u64;
    outcome.errors.delivery_misses = outcome
        .expected_deliveries
        .saturating_sub(outcome.received_deliveries);
    outcome.errors.server_error_frames = server_errors;
    outcome.errors.parse_failures = parse_failures.load(Ordering::Relaxed);

    for (_, sink) in writers.iter_mut() {
        let _ = sink.close().await;
    }
    for reader in readers {
        reader.abort();
    }

    outcome
}

/// Run the load test and return the aggregated report.
pub async fn run(config: LoadTestConfig) -> Report {
    let started = Instant::now();
    let run_id = Uuid::new_v4().simple().to_string();

    let mut tasks = Vec::with_capacity(config.games);
    for game_index in 0..config.games {
        let game_config = config.clone();
        tasks.push(tokio::spawn(async move {
            run_game(game_index, &game_config).await
        }));
    }

    let mut connect_samples: Vec<f64> = Vec::new();
    let mut opponent_samples: Vec<f64> = Vec::new();
    let mut spectator_samples: Vec<f64> = Vec::new();
    let mut moves_sent = 0u64;
    let mut expected_deliveries = 0u64;
    let mut received_deliveries = 0u64;
    let mut errors = ErrorBreakdown::default();
    let mut error_details: Vec<String> = Vec::new();

    for task in tasks {
        match task.await {
            Ok(outcome) => {
                connect_samples.extend(outcome.connect_samples);
                opponent_samples.extend(outcome.opponent_samples);
                spectator_samples.extend(outcome.spectator_samples);
                moves_sent += outcome.moves_sent;
                expected_deliveries += outcome.expected_deliveries;
                received_deliveries += outcome.received_deliveries;
                errors.merge(&outcome.errors);
                for detail in outcome.error_details {
                    if error_details.len() < 20 {
                        error_details.push(detail);
                    }
                }
            }
            Err(join_error) => {
                errors.insufficient_connections += 1;
                error_details.push(format!("game task failed: {}", join_error));
            }
        }
    }

    let duration_secs = started.elapsed().as_secs_f64();
    let operations_total = moves_sent + expected_deliveries;
    let errors_total = errors.total();
    let error_rate = if operations_total == 0 {
        if errors_total == 0 {
            0.0
        } else {
            1.0
        }
    } else {
        errors_total as f64 / operations_total as f64
    };
    let delivery_ratio = if expected_deliveries == 0 {
        1.0
    } else {
        received_deliveries as f64 / expected_deliveries as f64
    };

    Report {
        run_id,
        url: config.url.clone(),
        games: config.games,
        players_per_game: config.players_per_game,
        spectators_per_game: config.spectators_per_game,
        moves_per_game: config.moves,
        rate_per_game_per_sec: config.rate_per_second,
        duration_secs,
        moves_sent,
        moves_per_sec_achieved: if duration_secs > 0.0 {
            moves_sent as f64 / duration_secs
        } else {
            0.0
        },
        expected_deliveries,
        received_deliveries,
        delivery_ratio,
        operations_total,
        errors_total,
        error_rate,
        connect_latency_ms: summarize(connect_samples),
        opponent_broadcast_latency_ms: summarize(opponent_samples),
        spectator_broadcast_latency_ms: summarize(spectator_samples),
        errors,
        error_details,
    }
}

// ---------------------------------------------------------------------------
// Reporting
// ---------------------------------------------------------------------------

fn latency_table_row(out: &mut String, label: &str, stats: &LatencyStats) {
    let _ = writeln!(
        out,
        "  {:<28} {:>6} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>8.2}",
        label, stats.count, stats.p50_ms, stats.p90_ms, stats.p95_ms, stats.p99_ms, stats.max_ms
    );
}

/// Render the human-readable report. Kept free of `println!` so it can be
/// asserted on in tests.
pub fn format_report(report: &Report) -> String {
    let mut out = String::new();
    let total_sockets = report.games * (report.players_per_game + report.spectators_per_game);

    let _ = writeln!(out, "KnightVerse WebSocket move-submission load test");
    let _ = writeln!(out, "  run id           {}", report.run_id);
    let _ = writeln!(out, "  target           {}", report.url);
    let _ = writeln!(
        out,
        "  topology         {} games x [{} player + {} spectator] = {} sockets",
        report.games, report.players_per_game, report.spectators_per_game, total_sockets
    );
    let _ = writeln!(
        out,
        "  moves per game   {} (requested rate {} /s/game)",
        report.moves_per_game, report.rate_per_game_per_sec
    );
    let _ = writeln!(
        out,
        "  duration         {:.2} s ({} moves sent, {:.1} moves/s achieved)",
        report.duration_secs, report.moves_sent, report.moves_per_sec_achieved
    );
    out.push('\n');

    let _ = writeln!(out, "Latency (ms)");
    let _ = writeln!(
        out,
        "  {:<28} {:>6} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "stage", "n", "p50", "p90", "p95", "p99", "max"
    );
    latency_table_row(&mut out, "connect (handshake)", &report.connect_latency_ms);
    latency_table_row(
        &mut out,
        "move -> opponent peer",
        &report.opponent_broadcast_latency_ms,
    );
    latency_table_row(
        &mut out,
        "move -> spectator",
        &report.spectator_broadcast_latency_ms,
    );
    out.push('\n');

    let _ = writeln!(out, "Deliveries");
    let _ = writeln!(out, "  expected         {}", report.expected_deliveries);
    let _ = writeln!(
        out,
        "  received         {} ({:.2} %)",
        report.received_deliveries,
        report.delivery_ratio * 100.0
    );
    let _ = writeln!(
        out,
        "  error rate       {:.4} % ({} errors over {} operations)",
        report.error_rate * 100.0,
        report.errors_total,
        report.operations_total
    );
    let _ = writeln!(
        out,
        "  breakdown        connection_failures={} insufficient_connections={} send_failures={} \
delivery_misses={} server_error_frames={} parse_failures={}",
        report.errors.connection_failures,
        report.errors.insufficient_connections,
        report.errors.send_failures,
        report.errors.delivery_misses,
        report.errors.server_error_frames,
        report.errors.parse_failures
    );

    if !report.error_details.is_empty() {
        out.push('\n');
        let _ = writeln!(out, "First errors");
        for detail in &report.error_details {
            let _ = writeln!(out, "  - {}", detail);
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn percentile_interpolates_between_samples() {
        let samples = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(percentile(&samples, 0.0), 1.0);
        assert_eq!(percentile(&samples, 50.0), 2.5);
        assert_eq!(percentile(&samples, 100.0), 4.0);
        assert_eq!(percentile(&[], 95.0), 0.0);
        assert_eq!(percentile(&[7.0], 95.0), 7.0);
    }

    #[test]
    fn summarize_reports_percentiles_and_edges() {
        let stats = summarize(vec![5.0, 1.0, 4.0, 2.0, 3.0]);
        assert_eq!(stats.count, 5);
        assert_eq!(stats.min_ms, 1.0);
        assert_eq!(stats.max_ms, 5.0);
        assert_eq!(stats.mean_ms, 3.0);
        assert_eq!(stats.p50_ms, 3.0);
        assert!(stats.p95_ms >= stats.p90_ms);
        assert!(stats.p99_ms >= stats.p95_ms);

        assert_eq!(summarize(vec![]).count, 0);
    }

    #[test]
    fn parse_args_uses_documented_defaults() {
        let command = parse_args(&[]).unwrap();
        let Command::Run(options) = command else {
            panic!("expected Run");
        };
        let config = options.config;
        assert_eq!(config.url, "ws://127.0.0.1:8080");
        assert_eq!(config.games, 10);
        assert_eq!(config.players_per_game, 2);
        assert_eq!(config.moves, 50);
        assert_eq!(config.rate_per_second, 10.0);
        assert!(!options.self_test);
    }

    #[test]
    fn parse_args_accepts_flags_and_inline_values() {
        let command = parse_args(&args(&[
            "--url=ws://example.test:9000",
            "--games",
            "3",
            "--spectators",
            "4",
            "--moves=7",
            "--rate",
            "25.5",
            "--token",
            "abc.def.ghi",
            "--json",
            "/tmp/report.json",
            "--max-error-rate",
            "0.5",
        ]))
        .unwrap();
        let Command::Run(options) = command else {
            panic!("expected Run");
        };
        assert_eq!(options.config.url, "ws://example.test:9000");
        assert_eq!(options.config.games, 3);
        assert_eq!(options.config.spectators_per_game, 4);
        assert_eq!(options.config.moves, 7);
        assert_eq!(options.config.rate_per_second, 25.5);
        assert_eq!(options.config.token.as_deref(), Some("abc.def.ghi"));
        assert_eq!(
            options.json_path.as_deref(),
            Some(std::path::Path::new("/tmp/report.json"))
        );
        assert_eq!(options.max_error_rate, 0.5);
    }

    #[test]
    fn parse_args_rejects_unknown_or_invalid_input() {
        assert!(parse_args(&args(&["--nope"])).is_err());
        assert!(parse_args(&args(&["--games", "many"])).is_err());
        assert!(parse_args(&args(&["--rate"])).is_err());
        assert!(parse_args(&args(&["--games", "0"])).is_err());
        assert!(parse_args(&args(&["--players-per-game", "1"])).is_err());
        assert!(parse_args(&args(&["--rate", "-1"])).is_err());
    }

    #[test]
    fn parse_args_help_and_version() {
        assert_eq!(parse_args(&args(&["--help"])).unwrap(), Command::Help);
        assert_eq!(parse_args(&args(&["-h"])).unwrap(), Command::Help);
        assert_eq!(parse_args(&args(&["-V"])).unwrap(), Command::Version);
    }

    #[test]
    fn move_marker_only_matches_move_frames() {
        let frame = r#"{"type":"Move","payload":{"from":"e2","to":"e4","san":"lt-0-1","fen":"x"},"version":"1.0"}"#;
        assert_eq!(move_marker(frame).as_deref(), Some("lt-0-1"));
        assert_eq!(
            move_marker(r#"{"type":"Clock","payload":{"white":1}}"#),
            None
        );
        assert_eq!(move_marker("not json"), None);
    }

    #[test]
    fn error_rate_is_errors_over_operations() {
        let mut report = sample_report();
        report.moves_sent = 100;
        report.expected_deliveries = 100;
        report.errors.delivery_misses = 2;
        report.operations_total = 200;
        report.errors_total = 2;
        report.error_rate = 0.01;
        assert!(report.exceeds_error_budget(0.0));
        assert!(!report.exceeds_error_budget(0.01));
    }

    fn sample_report() -> Report {
        Report {
            run_id: "abc".to_string(),
            url: "ws://127.0.0.1:8080".to_string(),
            games: 1,
            players_per_game: 2,
            spectators_per_game: 1,
            moves_per_game: 5,
            rate_per_game_per_sec: 10.0,
            duration_secs: 0.5,
            moves_sent: 5,
            moves_per_sec_achieved: 10.0,
            expected_deliveries: 10,
            received_deliveries: 10,
            delivery_ratio: 1.0,
            operations_total: 15,
            errors_total: 0,
            error_rate: 0.0,
            connect_latency_ms: summarize(vec![1.0, 2.0]),
            opponent_broadcast_latency_ms: summarize(vec![0.5]),
            spectator_broadcast_latency_ms: summarize(vec![0.7]),
            errors: ErrorBreakdown::default(),
            error_details: vec![],
        }
    }

    #[test]
    fn report_renders_every_section_and_round_trips_as_json() {
        let report = sample_report();
        let text = format_report(&report);
        assert!(text.contains("Latency (ms)"));
        assert!(text.contains("connect (handshake)"));
        assert!(text.contains("move -> opponent peer"));
        assert!(text.contains("move -> spectator"));
        assert!(text.contains("Deliveries"));
        assert!(text.contains("error rate"));

        let json = serde_json::to_string(&report).unwrap();
        let parsed: Report = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, report);
    }

    /// End-to-end check: the harness drives the in-process mock endpoint and
    /// measures a clean run (no connection failures, no delivery misses).
    #[tokio::test]
    async fn self_test_run_reports_no_errors() {
        let server = mock::spawn(DEFAULT_SELF_TEST_SECRET.to_string())
            .await
            .expect("mock server should start");
        let config = LoadTestConfig {
            url: server.url.clone(),
            games: 2,
            players_per_game: 2,
            spectators_per_game: 1,
            moves: 5,
            rate_per_second: 0.0,
            drain_timeout: Duration::from_millis(1000),
            jwt_secret: Some(DEFAULT_SELF_TEST_SECRET.to_string()),
            ..LoadTestConfig::default()
        };

        let report = run(config).await;

        assert_eq!(report.moves_sent, 10);
        assert_eq!(report.expected_deliveries, 20);
        assert_eq!(report.received_deliveries, 20);
        assert_eq!(report.errors_total, 0, "{:?}", report.error_details);
        assert_eq!(report.connect_latency_ms.count, 6);
        assert_eq!(report.opponent_broadcast_latency_ms.count, 10);
        assert_eq!(report.spectator_broadcast_latency_ms.count, 10);
        assert!(report.opponent_broadcast_latency_ms.max_ms >= 0.0);
    }

    /// A bad token must be refused by the endpoint, which the harness counts as
    /// a connection failure rather than a silent zero-delivery success.
    #[tokio::test]
    async fn self_test_reports_rejected_handshakes() {
        let server = mock::spawn("the-real-secret".to_string())
            .await
            .expect("mock server should start");
        let config = LoadTestConfig {
            url: server.url.clone(),
            games: 1,
            players_per_game: 2,
            spectators_per_game: 0,
            moves: 3,
            rate_per_second: 0.0,
            jwt_secret: Some("a-different-secret".to_string()),
            ..LoadTestConfig::default()
        };

        let report = run(config).await;

        assert_eq!(report.moves_sent, 0);
        assert!(report.errors.insufficient_connections >= 1);
        assert!(report.errors_total > 0);
    }
}
