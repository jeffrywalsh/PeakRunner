//! Public HTTP/WSS edge. TLS terminates at Cloudflare; cloudflared encrypts the
//! journey to dellcon. Plain HTTP is confined to the dedicated Docker network.
use std::{collections::HashMap, io::{self, Read}, net::{IpAddr, SocketAddr, TcpStream},
    sync::{Arc, Mutex}, time::{Duration, Instant}};
use axum::{Router, Json, extract::{State, ConnectInfo, WebSocketUpgrade, ws::{Message, WebSocket}},
    http::{HeaderMap, StatusCode, header}, response::{IntoResponse, Response}, routing::get};
use serde::{Serialize, Deserialize};
use tokio::{sync::Semaphore, time::timeout};
use crate::{GameHost, proto::{ClientMsg, ServerMsg, ServerAdvert, PROTOCOL}, wire::{Framed, invalid}};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MatchStatus {
    pub name: String, pub map: String, pub players: u32, pub max_players: u32,
    pub tick: u64, pub round: u32, pub phase: String, pub score: [u32; 2],
    pub time_left: f32, pub password_required: bool,
}
#[derive(Serialize, Deserialize)]
pub struct PublicDirectory { pub protocol: String, pub servers: Vec<ServerAdvert> }

pub fn http_get(url: &str, https_only: bool) -> io::Result<Vec<u8>> {
    let config = ureq::Agent::config_builder().https_only(https_only)
        .max_redirects(0).timeout_global(Some(Duration::from_secs(4))).build();
    let mut response = ureq::Agent::new_with_config(config).get(url).call().map_err(io::Error::other)?;
    let mut bytes = Vec::new();
    response.body_mut().as_reader().take(262_145).read_to_end(&mut bytes)?;
    if bytes.len() > 262_144 { return Err(invalid("directory response too large")); }
    Ok(bytes)
}
pub fn browse_https(url: &str) -> io::Result<Vec<ServerAdvert>> {
    let data: PublicDirectory = serde_json::from_slice(&http_get(url, true)?).map_err(io::Error::other)?;
    if data.protocol != PROTOCOL || data.servers.len() > 128 { return Err(invalid("incompatible directory")); }
    for s in &data.servers {
        if !s.host.starts_with("quic://") || crate::quic::endpoint_url(&s.host).is_err()
            || s.name.is_empty() || s.name.len() > 64 || s.name.chars().any(char::is_control)
            || s.players > s.max_players || !(2..=8).contains(&s.max_players) {
            return Err(invalid("unsafe directory listing"));
        }
    }
    Ok(data.servers)
}

struct Limit { active: usize, tokens: f32, seen: Instant }
#[derive(Default)]
struct Limits { ips: HashMap<IpAddr, Limit> }
impl Limits {
    fn enter(&mut self, ip: IpAddr) -> bool {
        self.ips.retain(|_, v| v.active > 0 || v.seen.elapsed() < Duration::from_secs(300));
        if self.ips.len() >= 4096 && !self.ips.contains_key(&ip) { return false; }
        let now = Instant::now();
        let l = self.ips.entry(ip).or_insert(Limit { active: 0, tokens: 12.0, seen: now });
        l.tokens = (l.tokens + l.seen.elapsed().as_secs_f32() / 3.0).min(12.0);
        l.seen = now;
        if l.active >= 8 || l.tokens < 1.0 { return false; }
        l.tokens -= 1.0; l.active += 1; true
    }
}
struct Admission { limits: Arc<Mutex<Limits>>, ip: IpAddr }
impl Drop for Admission { fn drop(&mut self) {
    if let Some(l) = self.limits.lock().unwrap().ips.get_mut(&self.ip) { l.active = l.active.saturating_sub(1); }
} }
#[derive(Clone)]
struct Edge {
    backend: Option<SocketAddr>, trusted_proxy: bool,
    slots: Arc<Semaphore>, limits: Arc<Mutex<Limits>>,
    status: Arc<Mutex<Option<(MatchStatus, Instant)>>>, advertised: String,
}
fn client_ip(headers: &HeaderMap, peer: IpAddr, trusted_proxy: bool) -> Option<IpAddr> {
    if !trusted_proxy { return Some(peer); }
    // This mode must only be reachable from cloudflared on the isolated network.
    if headers.get("x-forwarded-proto")?.to_str().ok()? != "https" { return None; }
    headers.get("cf-connecting-ip")?.to_str().ok()?.parse().ok()
}
async fn upgrade(State(edge): State<Edge>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, ws: WebSocketUpgrade) -> Response {
    let Some(ip) = client_ip(&headers, peer.ip(), edge.trusted_proxy) else { return StatusCode::FORBIDDEN.into_response(); };
    if let Some(origin) = headers.get(header::ORIGIN) {
        if !["https://peakrunner.net", "https://www.peakrunner.net"].iter().any(|s| origin == *s) {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    let Ok(permit) = edge.slots.clone().try_acquire_owned() else { return StatusCode::SERVICE_UNAVAILABLE.into_response(); };
    if !edge.limits.lock().unwrap().enter(ip) { return StatusCode::TOO_MANY_REQUESTS.into_response(); }
    let admission = Admission { limits: edge.limits.clone(), ip };
    ws.max_message_size(2048).max_frame_size(2048).write_buffer_size(0).max_write_buffer_size(524_288)
        .on_upgrade(move |socket| async move {
            let _permit = permit; let _admission = admission;
            if let Some(addr) = edge.backend { bridge(socket, addr).await; }
        })
}
async fn bridge(mut socket: WebSocket, addr: SocketAddr) {
    let Ok(Ok(stream)) = tokio::task::spawn_blocking(move || TcpStream::connect_timeout(&addr, Duration::from_secs(1))).await else { return; };
    let Ok(mut wire) = Framed::new(stream, 262_144) else { return; };
    let mut poll = tokio::time::interval(Duration::from_millis(2));
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last = Instant::now();
    let mut tokens = 16.0f32;
    let mut refill = Instant::now();
    loop {
        tokio::select! {
            incoming = socket.recv() => {
                tokens = (tokens + refill.elapsed().as_secs_f32() * 120.0).min(16.0);
                refill = Instant::now(); tokens -= 1.0;
                if tokens < 0.0 { break; }
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        let Ok(message) = serde_json::from_str::<ClientMsg>(&text) else { break; };
                        if wire.send(&message).is_err() { break; }
                        last = Instant::now();
                    }
                    Some(Ok(Message::Pong(_))) => {},
                    Some(Ok(Message::Ping(bytes))) => { if !matches!(timeout(Duration::from_secs(1), socket.send(Message::Pong(bytes))).await, Ok(Ok(()))) { break; } },
                    _ => break,
                }
            }
            _ = poll.tick() => {
                if last.elapsed() > Duration::from_secs(5) || wire.flush().is_err() { break; }
                let Ok(messages) = wire.receive::<ServerMsg>() else { break; };
                for msg in messages {
                    let Ok(text) = serde_json::to_string(&msg) else { return; };
                    if !matches!(timeout(Duration::from_secs(1), socket.send(Message::Text(text.into()))).await, Ok(Ok(()))) { return; }
                }
            }
        }
    }
    let _ = timeout(Duration::from_millis(200), socket.send(Message::Close(None))).await;
}
fn live(edge: &Edge) -> Option<MatchStatus> {
    edge.status.lock().unwrap().as_ref().filter(|(_, seen)| seen.elapsed() < Duration::from_secs(12)).map(|(s, _)| s.clone())
}
async fn status(State(edge): State<Edge>) -> Response {
    match live(&edge) { Some(status) => Json(status).into_response(), None => StatusCode::SERVICE_UNAVAILABLE.into_response() }
}
async fn listing(State(edge): State<Edge>) -> Response {
    let servers = live(&edge).map(|s| vec![ServerAdvert { id: "dellcon-north-spine".into(),
        name: s.name, map: s.map, host: edge.advertised.clone(), port: crate::quic::endpoint_url(&edge.advertised).ok().and_then(|u| u.port()).unwrap_or(7777),
        players: s.players, max_players: s.max_players }]).unwrap_or_default();
    let mut response = Json(PublicDirectory { protocol: PROTOCOL.into(), servers }).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response.headers_mut().insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    response
}

pub async fn serve(bind: &str, directory: bool, trusted_proxy: bool, backend_url: String, advertised: String) -> io::Result<()> {
    crate::quic::endpoint_url(&advertised)?;
    let edge = Edge { backend: None, trusted_proxy, slots: Arc::new(Semaphore::new(16)),
        limits: Arc::new(Mutex::new(Limits::default())), status: Arc::new(Mutex::new(None)), advertised };
    let mut game_handle = None;
    let mut edge = edge;
    if directory {
        let output = edge.status.clone();
        tokio::spawn(async move {
            loop {
                if let Ok(status) = crate::quic::query_status(&backend_url).await {
                    *output.lock().unwrap() = Some((status, Instant::now()));
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });
    } else {
        let host = GameHost::bind("127.0.0.1:0", "North Spine", 8, "Valley")?
            .with_password(std::env::var("PEAKRUNNER_MATCH_PASSWORD").unwrap_or_default());
        edge.backend = Some(host.local_addr());
        let handle = host.spawn();
        let input = handle.status.clone(); let output = edge.status.clone();
        tokio::spawn(async move {
            let mut tick = 0;
            loop {
                let status = input.lock().unwrap().clone();
                if status.tick > tick { tick = status.tick; *output.lock().unwrap() = Some((status, Instant::now())); }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });
        game_handle = Some(handle);
    }
    let app = if directory { Router::new().route("/", get(listing)).route("/servers", get(listing)) }
        else { Router::new().route("/match", get(upgrade)).route("/status", get(status)) };
    let app = app.route("/healthz", get(status)).with_state(edge);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    log::info!("{} HTTP edge listening on {}", if directory { "directory" } else { "match" }, listener.local_addr()?);
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).with_graceful_shutdown(async {
        #[cfg(unix)] {
            let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("SIGTERM");
            tokio::select! { _ = term.recv() => {}, _ = tokio::signal::ctrl_c() => {} }
        }
        #[cfg(not(unix))] { let _ = tokio::signal::ctrl_c().await; }
    }).await?;
    drop(game_handle);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn bounded_admission_and_proxy_trust() {
        let ip = "127.0.0.1".parse().unwrap(); let mut limits = Limits::default();
        for _ in 0..8 { assert!(limits.enter(ip)); } assert!(!limits.enter(ip));
        let mut headers = HeaderMap::new();
        assert_eq!(client_ip(&headers, ip, false), Some(ip));
        assert_eq!(client_ip(&headers, ip, true), None);
        headers.insert("cf-connecting-ip", "1.2.3.4".parse().unwrap());
        headers.insert("x-forwarded-proto", "https".parse().unwrap());
        assert_eq!(client_ip(&headers, ip, true), Some("1.2.3.4".parse().unwrap()));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn websocket_match_rejects_forgery_and_releases_disconnected_slots() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap(); drop(listener);
        let server = tokio::spawn(async move { serve(&addr.to_string(), false, false,
            String::new(), "quic://play.peakrunner.net:7777".into()).await });
        tokio::time::sleep(Duration::from_millis(100)).await;
        let endpoint = format!("ws://{addr}/match");
        let a = crate::connect(&endpoint, 0, "A").unwrap();
        let b = crate::connect(&endpoint, 0, "B").unwrap();
        for _ in 0..200 {
            if a.lobby().players.len() == 2 && b.lobby().connected { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(a.lobby().connected && b.lobby().connected);
        assert_ne!(a.lobby().player_id, b.lobby().player_id);
        // Malformed and oversized frames must close without affecting valid peers.
        for payload in [r#"{"op":"pose","position":[999,999,999]}"#.to_string(), "x".repeat(4096)] {
            let endpoint = endpoint.clone();
            tokio::task::spawn_blocking(move || {
                let (mut bad, _) = tungstenite::connect(&endpoint).unwrap();
                if let tungstenite::stream::MaybeTlsStream::Plain(s) = bad.get_mut() { s.set_read_timeout(Some(Duration::from_secs(2))).unwrap(); }
                let _ = bad.send(tungstenite::Message::Text(payload.into()));
                assert!(matches!(bad.read(), Err(_) | Ok(tungstenite::Message::Close(_))));
            }).await.unwrap();
        }
        assert!(a.lobby().connected);
        drop(b);
        for _ in 0..200 {
            if a.lobby().players.len() == 1 { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(a.lobby().players, vec!["A"]);
        drop(a); server.abort(); let _ = server.await;
    }
}
