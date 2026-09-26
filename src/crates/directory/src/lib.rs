//! Standalone discovery service. No simulation, gameplay protocol, or client dependency.
//!
//! Two kinds of listing:
//! - The configured backend (`dellcon-north-spine`), always polled.
//! - Announced servers: a hosted server POSTs `{"url":"quic://host:port"}` to
//!   `/announce` every ANNOUNCE_EVERY seconds. The directory lists it only if
//!   the claimed host resolves to the address the request came from and a
//!   certificate-validated status query to that URL succeeds; afterwards it
//!   polls the server itself like the backend. Entries lapse when announces
//!   stop (ANNOUNCE_TTL) or status goes stale. Announces are rate-limited per
//!   address and capped (MAX_ANNOUNCED, MAX_PER_ADDRESS).
pub mod lan;
use std::{collections::HashMap, io, net::{IpAddr, SocketAddr}, sync::{Arc, Mutex}, time::{Duration, Instant}};
use axum::{Router, Json, extract::{ConnectInfo, DefaultBodyLimit, State, Path}, http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response}, routing::{get, post}};
use peakrunner_discovery::{Announce, MatchStatus, ServerAdvert, PublicDirectory, PROTOCOL, announce_id,
    quic::{endpoint_url, query_status}};

const BACKEND_ID: &str = "dellcon-north-spine";
const FRESH: Duration = Duration::from_secs(12);
/// Hosted servers announce this often; entries lapse after ANNOUNCE_TTL.
pub const ANNOUNCE_EVERY: Duration = Duration::from_secs(20);
const ANNOUNCE_TTL: Duration = Duration::from_secs(65);
const MIN_ANNOUNCE_GAP: Duration = Duration::from_secs(4);
const MAX_ANNOUNCED: usize = 32;
const MAX_PER_ADDRESS: usize = 4;

struct Announced { owner: IpAddr, announced: Instant, status: Option<(MatchStatus, Instant)> }

#[derive(Clone)]
struct Directory {
    status: Arc<Mutex<Option<(MatchStatus, Instant)>>>,
    advertised: String,
    announced: Arc<Mutex<HashMap<String, Announced>>>,
    last_announce: Arc<Mutex<HashMap<IpAddr, Instant>>>,
    trusted: bool,
}

impl Directory {
    fn new(advertised: String, trusted: bool) -> Self {
        Self { status: Arc::new(Mutex::new(None)), advertised, announced: Arc::default(), last_announce: Arc::default(), trusted }
    }
}

fn live(directory: &Directory) -> Option<MatchStatus> {
    directory.status.lock().unwrap().as_ref()
        .filter(|(_, seen)| seen.elapsed() < FRESH).map(|(s, _)| s.clone())
}

fn advert(id: String, url: &str, s: MatchStatus) -> ServerAdvert {
    ServerAdvert { id, name: s.name, map: s.map, host: url.into(),
        port: endpoint_url(url).ok().and_then(|u| u.port()).unwrap_or(7777),
        players: s.players, max_players: s.max_players }
}

fn announced_live(directory: &Directory) -> Vec<(String, String, MatchStatus)> {
    let map = directory.announced.lock().unwrap();
    let mut out: Vec<_> = map.iter().filter_map(|(url, a)| {
        let (s, seen) = a.status.as_ref()?;
        if seen.elapsed() >= FRESH || a.announced.elapsed() >= ANNOUNCE_TTL { return None; }
        Some((announce_id(url)?, url.clone(), s.clone()))
    }).collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response.headers_mut().insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    response
}

async fn health(State(directory): State<Directory>) -> Response {
    match live(&directory) { Some(s) => Json(s).into_response(), None => StatusCode::SERVICE_UNAVAILABLE.into_response() }
}

async fn listing(State(directory): State<Directory>) -> Response {
    let mut servers: Vec<ServerAdvert> = live(&directory)
        .map(|s| advert(BACKEND_ID.into(), &directory.advertised, s)).into_iter().collect();
    servers.extend(announced_live(&directory).into_iter().map(|(id, url, s)| advert(id, &url, s)));
    no_store(Json(PublicDirectory { protocol: PROTOCOL.into(), servers }).into_response())
}

async fn details(Path(id): Path<String>, State(directory): State<Directory>) -> Response {
    let found = if id == BACKEND_ID { Some(live(&directory)) }
        else { announced_live(&directory).into_iter().find(|(i, _, _)| *i == id).map(|(_, _, s)| Some(s)) };
    no_store(match found {
        None => StatusCode::NOT_FOUND.into_response(),
        Some(Some(status)) => Json(status).into_response(),
        Some(None) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    })
}

/// The address an announce came from: Cloudflare's client IP header when the
/// directory sits behind the tunnel (trusted mode), else the socket peer.
fn requester(directory: &Directory, headers: &HeaderMap, peer: SocketAddr) -> Option<IpAddr> {
    if directory.trusted {
        headers.get("cf-connecting-ip")?.to_str().ok()?.trim().parse().ok()
    } else { Some(peer.ip()) }
}

fn same_ip(a: IpAddr, b: IpAddr) -> bool { a.to_canonical() == b.to_canonical() }

/// Admission before any network work: the URL shape, the per-address rate
/// and the caps. Ok(true) for a new entry, Ok(false) to refresh one.
fn admit(directory: &Directory, url: &str, from: IpAddr, now: Instant) -> Result<bool, StatusCode> {
    if url == directory.advertised { return Err(StatusCode::NO_CONTENT); }
    let u = endpoint_url(url).map_err(|_| StatusCode::BAD_REQUEST)?;
    if announce_id(url).is_none() || u.port().is_some_and(|p| p < 1024) { return Err(StatusCode::BAD_REQUEST); }
    {
        let mut last = directory.last_announce.lock().unwrap();
        if last.get(&from).is_some_and(|t| now.duration_since(*t) < MIN_ANNOUNCE_GAP) { return Err(StatusCode::TOO_MANY_REQUESTS); }
        last.retain(|_, t| now.duration_since(*t) < ANNOUNCE_TTL);
        last.insert(from, now);
    }
    let mut map = directory.announced.lock().unwrap();
    map.retain(|_, a| now.duration_since(a.announced) < ANNOUNCE_TTL);
    if let Some(a) = map.get_mut(url) {
        if !same_ip(a.owner, from) { return Err(StatusCode::FORBIDDEN); }
        a.announced = now;
        return Ok(false);
    }
    if map.len() >= MAX_ANNOUNCED { return Err(StatusCode::SERVICE_UNAVAILABLE); }
    if map.values().filter(|a| same_ip(a.owner, from)).count() >= MAX_PER_ADDRESS { return Err(StatusCode::TOO_MANY_REQUESTS); }
    Ok(true)
}

async fn announce(State(directory): State<Directory>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap,
                  Json(body): Json<Announce>) -> Response {
    let Some(from) = requester(&directory, &headers, peer) else { return StatusCode::BAD_REQUEST.into_response() };
    let url = body.url.trim().to_string();
    let fresh = match admit(&directory, &url, from, Instant::now()) {
        Ok(fresh) => fresh,
        Err(code) => return no_store(code.into_response()),
    };
    if !fresh { return no_store(StatusCode::NO_CONTENT.into_response()); }
    // The claimed host must be the announcer's own address.
    let u = endpoint_url(&url).unwrap();
    let host = u.host_str().unwrap().trim_matches(['[', ']']).to_string();
    let port = u.port().unwrap_or(7777);
    let resolves = tokio::net::lookup_host((host.as_str(), port)).await
        .map(|addrs| addrs.into_iter().any(|a| same_ip(a.ip(), from))).unwrap_or(false);
    if !resolves { return no_store(StatusCode::FORBIDDEN.into_response()); }
    // And it must answer a certificate-validated status query there.
    let status = match query_status(&url).await { Ok(s) => s, Err(_) => return no_store(StatusCode::UNPROCESSABLE_ENTITY.into_response()) };
    let mut map = directory.announced.lock().unwrap();
    if map.len() >= MAX_ANNOUNCED && !map.contains_key(&url) { return no_store(StatusCode::SERVICE_UNAVAILABLE.into_response()); }
    let now = Instant::now();
    map.insert(url, Announced { owner: from, announced: now, status: Some((status, now)) });
    log::info!("listed announced server {host}:{port}");
    no_store(StatusCode::CREATED.into_response())
}

pub async fn serve(bind: &str, backend: String, advertised: String) -> io::Result<()> {
    serve_with(bind, backend, advertised, false).await
}

pub async fn serve_with(bind: &str, backend: String, advertised: String, trusted: bool) -> io::Result<()> {
    endpoint_url(&advertised)?;
    endpoint_url(&backend)?;
    let listener = tokio::net::TcpListener::bind(bind).await?;
    let directory = Directory::new(advertised, trusted);
    let output = directory.status.clone();
    let announced = directory.announced.clone();
    let poller = tokio::spawn(async move {
        loop {
            if let Ok(status) = query_status(&backend).await {
                *output.lock().unwrap() = Some((status, Instant::now()));
            }
            let urls: Vec<String> = {
                let mut map = announced.lock().unwrap();
                map.retain(|_, a| a.announced.elapsed() < ANNOUNCE_TTL);
                map.keys().cloned().collect()
            };
            let mut set = tokio::task::JoinSet::new();
            for url in urls { set.spawn(async move { let s = query_status(&url).await; (url, s) }); }
            while let Some(Ok((url, result))) = set.join_next().await {
                if let (Ok(status), Some(a)) = (result, announced.lock().unwrap().get_mut(&url)) {
                    a.status = Some((status, Instant::now()));
                }
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
    let app = Router::new().route("/", get(listing)).route("/servers", get(listing))
        .route("/servers/{id}", get(details))
        .route("/announce", post(announce)).layer(DefaultBodyLimit::max(1024))
        .route("/healthz", get(health)).with_state(directory);
    log::info!("Directory listening on {}", listener.local_addr()?);
    let result = axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).with_graceful_shutdown(async {
        #[cfg(unix)] {
            let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("SIGTERM");
            tokio::select! { _ = term.recv() => {}, _ = tokio::signal::ctrl_c() => {} }
        }
        #[cfg(not(unix))] { let _ = tokio::signal::ctrl_c().await; }
    }).await;
    poller.abort();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn listings_expire_and_never_expose_stale_matches() {
        let d = Directory::new("quic://play.peakrunner.net:7777".into(), false);
        assert_eq!(health(State(d.clone())).await.status(), StatusCode::SERVICE_UNAVAILABLE);
        *d.status.lock().unwrap() = Some((MatchStatus { name: "North Spine".into(), map: "Raindance".into(), max_players: 8, ..Default::default() }, Instant::now()));
        let response = listing(State(d.clone())).await;
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let bytes = axum::body::to_bytes(response.into_body(), 4096).await.unwrap();
        let catalog: PublicDirectory = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(catalog.servers.len(), 1);
        assert_eq!(catalog.servers[0].map, "Raindance");
        assert_eq!(details(Path("unknown".into()),State(d.clone())).await.status(),StatusCode::NOT_FOUND);
        assert_eq!(details(Path("dellcon-north-spine".into()),State(d.clone())).await.status(),StatusCode::OK);
        d.status.lock().unwrap().as_mut().unwrap().1 = Instant::now() - Duration::from_secs(13);
        assert!(live(&d).is_none());
        assert_eq!(details(Path("dellcon-north-spine".into()),State(d.clone())).await.status(),StatusCode::SERVICE_UNAVAILABLE);
        let bytes = axum::body::to_bytes(listing(State(d)).await.into_body(), 4096).await.unwrap();
        assert!(serde_json::from_slice::<PublicDirectory>(&bytes).unwrap().servers.is_empty());
    }

    #[tokio::test]
    async fn announced_servers_are_listed_by_port_and_lapse() {
        let d = Directory::new("quic://play.peakrunner.net:7777".into(), false);
        let url = "quic://play.peakrunner.net:7778".to_string();
        let status = MatchStatus { name: "Football".into(), map: "Longfield - Football".into(), max_players: 8, ..Default::default() };
        d.announced.lock().unwrap().insert(url.clone(), Announced { owner: "198.51.100.7".parse().unwrap(),
            announced: Instant::now(), status: Some((status, Instant::now())) });
        let bytes = axum::body::to_bytes(listing(State(d.clone())).await.into_body(), 4096).await.unwrap();
        let catalog: PublicDirectory = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(catalog.servers.len(), 1, "the backend is down; the announced one is up");
        assert_eq!((catalog.servers[0].id.as_str(), catalog.servers[0].port), ("play-peakrunner-net-7778", 7778));
        assert_eq!(details(Path("play-peakrunner-net-7778".into()), State(d.clone())).await.status(), StatusCode::OK);
        // Announces stop: it lapses.
        d.announced.lock().unwrap().get_mut(&url).unwrap().announced = Instant::now() - ANNOUNCE_TTL;
        assert!(announced_live(&d).is_empty());
    }

    #[test]
    fn announces_are_rate_limited_capped_and_owned() {
        let d = Directory::new("quic://play.peakrunner.net:7777".into(), false);
        let a: IpAddr = "198.51.100.7".parse().unwrap();
        let b: IpAddr = "203.0.113.9".parse().unwrap();
        let t0 = Instant::now();
        assert_eq!(admit(&d, "quic://play.peakrunner.net:7777", a, t0), Err(StatusCode::NO_CONTENT), "the backend is already listed");
        assert_eq!(admit(&d, "https://x.example:7778", a, t0), Err(StatusCode::BAD_REQUEST));
        assert_eq!(admit(&d, "quic://x.example:80", a, t0), Err(StatusCode::BAD_REQUEST), "privileged ports refused");
        assert_eq!(admit(&d, "quic://x.example:7778", a, t0), Ok(true));
        assert_eq!(admit(&d, "quic://x.example:7779", a, t0), Err(StatusCode::TOO_MANY_REQUESTS), "too soon");
        d.announced.lock().unwrap().insert("quic://x.example:7778".into(), Announced { owner: a, announced: t0, status: None });
        let t1 = t0 + MIN_ANNOUNCE_GAP;
        assert_eq!(admit(&d, "quic://x.example:7778", a, t1), Ok(false), "a heartbeat refreshes");
        assert_eq!(admit(&d, "quic://x.example:7778", b, t1), Err(StatusCode::FORBIDDEN), "someone else can't take it over");
        for port in 7780..7783 {
            d.announced.lock().unwrap().insert(format!("quic://x.example:{port}"), Announced { owner: a, announced: t1, status: None });
        }
        let t2 = t1 + MIN_ANNOUNCE_GAP;
        assert_eq!(admit(&d, "quic://x.example:7790", a, t2), Err(StatusCode::TOO_MANY_REQUESTS), "per-address cap");
    }
}
