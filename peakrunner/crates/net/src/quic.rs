//! Direct encrypted UDP. Inputs and bounded, independently replaceable snapshots
//! use QUIC datagrams. Only admission/control uses a reliable stream.
use std::{collections::{BTreeMap, HashMap, VecDeque}, io, net::{IpAddr, SocketAddr, TcpStream},
    sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}, mpsc}, time::{Duration, Instant}};
use peakrunner_core::sim::{Command, Snapshot, MAX_PLAYERS};
use serde::{Deserialize, Serialize};
use quinn::{Connection, Endpoint};
use tokio::{sync::Semaphore, time::timeout};
use crate::{client::Lobby, proto::{ClientMsg, ServerMsg}, wire::{Framed, invalid}, GameHost};

const ALPN: &[u8] = b"peakrunner/3";
const CHUNK: usize = 1050;
const MAX_PARTS: usize = 64;
const MAX_SNAPSHOT: usize = CHUNK * MAX_PARTS;

#[derive(Serialize, Deserialize)]
struct Inputs { count: u8, frames: [Command; 3] }
fn inputs(history: &VecDeque<Command>) -> Vec<u8> {
    let mut data = Inputs { count: history.len() as u8, frames: [Command::default(); 3] };
    for (i, command) in history.iter().enumerate() { data.frames[i] = *command; }
    postcard::to_allocvec(&data).expect("fixed input packet")
}
fn decode_inputs(bytes: &[u8]) -> io::Result<Vec<Command>> {
    if bytes.len() > 256 { return Err(invalid("input packet too large")); }
    let data: Inputs = postcard::from_bytes(bytes).map_err(|_| invalid("invalid input packet"))?;
    if !(1..=3).contains(&data.count) { return Err(invalid("invalid input count")); }
    let frames = &data.frames[..data.count as usize];
    if frames.iter().any(|c| !c.valid()) || frames.windows(2).any(|w| w[0].seq >= w[1].seq) {
        return Err(invalid("invalid input sequence"));
    }
    Ok(frames.to_vec())
}
fn chunks(snapshot: &Snapshot) -> io::Result<Vec<Vec<u8>>> {
    let bytes = postcard::to_allocvec(snapshot).map_err(io::Error::other)?;
    if bytes.len() > MAX_SNAPSHOT { return Err(invalid("snapshot limit exceeded")); }
    let count = bytes.len().div_ceil(CHUNK);
    Ok(bytes.chunks(CHUNK).enumerate().map(|(i, chunk)| {
        let mut packet = Vec::with_capacity(chunk.len() + 10);
        packet.extend_from_slice(&snapshot.tick.to_le_bytes());
        packet.push(i as u8); packet.push(count as u8); packet.extend_from_slice(chunk); packet
    }).collect())
}
struct Assembly { born: Instant, parts: Vec<Option<Vec<u8>>> }
#[derive(Default)]
struct Reassembly { frames: BTreeMap<u64, Assembly>, completed: u64 }
impl Reassembly {
    fn accept(&mut self, packet: &[u8]) -> io::Result<Option<Snapshot>> {
        if packet.len() < 11 || packet.len() > CHUNK + 10 { return Err(invalid("invalid snapshot fragment")); }
        let tick = u64::from_le_bytes(packet[..8].try_into().unwrap());
        let index = packet[8] as usize; let count = packet[9] as usize;
        if count == 0 || count > MAX_PARTS || index >= count { return Err(invalid("invalid fragment count")); }
        if tick <= self.completed { return Ok(None); }
        self.frames.retain(|_, a| a.born.elapsed() < Duration::from_millis(250));
        if !self.frames.contains_key(&tick) && self.frames.len() >= 4 {
            if tick < *self.frames.first_key_value().unwrap().0 { return Ok(None); }
            self.frames.pop_first();
        }
        let frame = self.frames.entry(tick).or_insert_with(|| Assembly { born: Instant::now(), parts: vec![None; count] });
        if frame.parts.len() != count { return Err(invalid("inconsistent snapshot fragments")); }
        frame.parts[index] = Some(packet[10..].to_vec());
        if frame.parts.iter().any(Option::is_none) { return Ok(None); }
        let mut bytes = Vec::new();
        for part in &frame.parts { bytes.extend_from_slice(part.as_ref().unwrap()); }
        let snapshot: Snapshot = postcard::from_bytes(&bytes).map_err(|_| invalid("invalid snapshot data"))?;
        if snapshot.tick != tick || snapshot.players.len() != MAX_PLAYERS || snapshot.acks.len() != MAX_PLAYERS {
            return Err(invalid("invalid snapshot identity"));
        }
        self.completed = tick; self.frames.retain(|key, _| *key > tick);
        Ok(Some(snapshot))
    }
}

pub fn endpoint_url(address: &str) -> io::Result<url::Url> {
    let url = url::Url::parse(address).map_err(|_| invalid("invalid QUIC URL"))?;
    if url.scheme() != "quic" || url.host_str().is_none() || !url.username().is_empty()
        || url.password().is_some() || url.query().is_some() || url.fragment().is_some()
        || !["", "/"].contains(&url.path()) { return Err(invalid("use quic://hostname:7777")); }
    Ok(url)
}
fn transport() -> quinn::TransportConfig {
    let mut cfg = quinn::TransportConfig::default();
    cfg.max_concurrent_bidi_streams(1u32.into()).max_concurrent_uni_streams(0u32.into())
        .stream_receive_window(8192u32.into()).receive_window(16384u32.into())
        .datagram_receive_buffer_size(Some(256 * 1024)).datagram_send_buffer_size(128 * 1024)
        .max_idle_timeout(Some(Duration::from_secs(10).try_into().unwrap()))
        .keep_alive_interval(Some(Duration::from_secs(1)));
    cfg
}
fn client_config(extra_root: Option<rustls::pki_types::CertificateDer<'static>>) -> io::Result<quinn::ClientConfig> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    if let Some(root) = extra_root { roots.add(root).map_err(io::Error::other)?; }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut tls = rustls::ClientConfig::builder_with_provider(provider).with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(io::Error::other)?.with_root_certificates(roots).with_no_client_auth();
    tls.alpn_protocols = vec![ALPN.to_vec()];
    let crypto = quinn::crypto::rustls::QuicClientConfig::try_from(tls).map_err(io::Error::other)?;
    let mut cfg = quinn::ClientConfig::new(Arc::new(crypto)); cfg.transport_config(Arc::new(transport())); Ok(cfg)
}
pub fn server_config(cert: &[u8], key: &[u8]) -> io::Result<quinn::ServerConfig> {
    let certs = rustls_pemfile::certs(&mut io::Cursor::new(cert)).collect::<Result<Vec<_>, _>>()?;
    let key = rustls_pemfile::private_key(&mut io::Cursor::new(key))?.ok_or_else(|| invalid("missing TLS key"))?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut tls = rustls::ServerConfig::builder_with_provider(provider).with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(io::Error::other)?.with_no_client_auth().with_single_cert(certs, key).map_err(io::Error::other)?;
    tls.alpn_protocols = vec![ALPN.to_vec()];
    // No 0-RTT: joining and inputs must not be accepted as replayable early data.
    tls.max_early_data_size = 0;
    let crypto = quinn::crypto::rustls::QuicServerConfig::try_from(tls).map_err(io::Error::other)?;
    let mut cfg = quinn::ServerConfig::with_crypto(Arc::new(crypto));
    cfg.transport_config(Arc::new(transport())).migration(false).max_incoming(32);
    Ok(cfg)
}
async fn send_control<T: Serialize>(stream: &mut quinn::SendStream, data: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec(data).map_err(io::Error::other)?;
    if bytes.len() > 4096 { return Err(invalid("control message too large")); }
    stream.write_all(&(bytes.len() as u32).to_le_bytes()).await.map_err(io::Error::other)?;
    stream.write_all(&bytes).await.map_err(io::Error::other)
}
async fn read_control<T: serde::de::DeserializeOwned>(stream: &mut quinn::RecvStream) -> io::Result<T> {
    let mut len = [0u8; 4]; stream.read_exact(&mut len).await.map_err(io::Error::other)?;
    let len = u32::from_le_bytes(len) as usize;
    if len > 4096 { return Err(invalid("control message too large")); }
    let mut bytes = vec![0; len]; stream.read_exact(&mut bytes).await.map_err(io::Error::other)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}
fn apply(lobby: &Arc<Mutex<Lobby>>, message: ServerMsg) -> io::Result<()> {
    let mut l = lobby.lock().unwrap();
    match message {
        ServerMsg::Welcome { player_id, match_name } => { l.player_id = player_id; l.match_name = match_name; },
        ServerMsg::Reject { message } => return Err(io::Error::other(message)),
        ServerMsg::Status { .. } => return Err(invalid("unexpected status")),
        ServerMsg::Snapshot { state } => {
            if !state.players.iter().any(|p| p.net_id == l.player_id) { return Err(invalid("player missing from snapshot")); }
            if l.snapshot.as_ref().is_some_and(|s| s.tick >= state.tick) { return Ok(()); }
            l.map = format!("{:?}", state.map);
            l.players = state.players.iter().filter(|p| p.net_id != 0).map(|p| p.name.clone()).collect();
            l.snapshot = Some(state); l.connected = true;
        }
    }
    Ok(())
}
async fn connect_endpoint(address: &str) -> io::Result<(Endpoint, Connection)> {
    let url = endpoint_url(address)?;
    let host = url.host_str().unwrap().trim_matches(['[', ']']);
    // For pre-DNS deployment checks only. Hostname/certificate verification is
    // unchanged, even when a diagnostic explicitly selects the destination IP.
    let addr = if let Ok(ip) = std::env::var("PEAKRUNNER_QUIC_TEST_IP") {
        SocketAddr::new(ip.parse().map_err(|_| invalid("invalid diagnostic IP"))?, url.port().unwrap_or(7777))
    } else { tokio::net::lookup_host((host, url.port().unwrap_or(7777))).await?
        .next().ok_or_else(|| invalid("server address missing"))? };
    let mut endpoint = Endpoint::client(if addr.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" }.parse().unwrap())?;
    endpoint.set_default_client_config(client_config(None)?);
    let connecting = endpoint.connect(addr, host).map_err(io::Error::other)?;
    let conn = timeout(Duration::from_secs(6), connecting).await.map_err(|_| invalid("UDP connection timed out; check network/firewall"))?
        .map_err(|_| invalid("encrypted UDP connection failed; check server availability and certificate"))?;
    Ok((endpoint, conn))
}
pub(crate) async fn run_client(address: &str, hello: ClientMsg, outbound: mpsc::Receiver<Command>,
    lobby: Arc<Mutex<Lobby>>, stop: Arc<AtomicBool>) -> io::Result<()> {
    let (endpoint, conn) = connect_endpoint(address).await?;
    let result = client_connection(&conn, hello, outbound, lobby, stop).await;
    conn.close(0u32.into(), b"client left"); endpoint.wait_idle().await;
    result
}
pub async fn query_status(address: &str) -> io::Result<crate::public::MatchStatus> {
    let (endpoint, conn) = connect_endpoint(address).await?;
    let result = timeout(Duration::from_secs(3), async {
        let (mut send, mut recv) = conn.open_bi().await.map_err(io::Error::other)?;
        send_control(&mut send, &ClientMsg::Status).await?;
        match read_control::<ServerMsg>(&mut recv).await? {
            ServerMsg::Status { status } => Ok(status), _ => Err(invalid("unexpected status reply")),
        }
    }).await.map_err(io::Error::other)?;
    conn.close(0u32.into(), b"status complete"); endpoint.wait_idle().await; result
}
async fn client_connection(conn: &Connection, hello: ClientMsg, outbound: mpsc::Receiver<Command>,
    lobby: Arc<Mutex<Lobby>>, stop: Arc<AtomicBool>) -> io::Result<()> {
    let (mut send, mut recv) = conn.open_bi().await.map_err(io::Error::other)?;
    timeout(Duration::from_secs(3), send_control(&mut send, &hello)).await.map_err(io::Error::other)??;
    let welcome = timeout(Duration::from_secs(4), read_control::<ServerMsg>(&mut recv)).await.map_err(io::Error::other)??;
    apply(&lobby, welcome)?;
    let mut assembly = Reassembly::default(); let mut history = VecDeque::new();
    let mut last_snapshot = Instant::now();
    let mut poll = tokio::time::interval(Duration::from_millis(2));
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // Keep this future pinned: cancel/restarting a partial stream read loses framing.
    let control = read_control::<ServerMsg>(&mut recv); tokio::pin!(control);
    loop {
        tokio::select! {
            data = conn.read_datagram() => {
                if let Some(state) = assembly.accept(&data.map_err(io::Error::other)?)? {
                    apply(&lobby, ServerMsg::Snapshot { state })?; last_snapshot = Instant::now();
                }
            }
            message = &mut control => { apply(&lobby, message?)?; return Err(invalid("unexpected server control")); }
            _ = poll.tick() => {
                if stop.load(Ordering::Relaxed) { return Ok(()); }
                if last_snapshot.elapsed() > Duration::from_secs(5) { return Err(invalid("server snapshots timed out")); }
                for _ in 0..8 {
                    match outbound.try_recv() {
                        Ok(command) => {
                            history.push_back(command); while history.len() > 3 { history.pop_front(); }
                            conn.send_datagram(inputs(&history).into()).map_err(io::Error::other)?;
                        }
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
                    }
                }
            }
        }
    }
}

struct Admission { active: usize, tokens: f32, seen: Instant }
type Admissions = Arc<Mutex<HashMap<IpAddr, Admission>>>;
struct Permit { peers: Admissions, ip: IpAddr }
impl Drop for Permit { fn drop(&mut self) {
    if let Some(p) = self.peers.lock().unwrap().get_mut(&self.ip) { p.active = p.active.saturating_sub(1); }
} }
fn admit(peers: &Admissions, ip: IpAddr) -> Option<Permit> {
    let mut map = peers.lock().unwrap();
    map.retain(|_, p| p.active > 0 || p.seen.elapsed() < Duration::from_secs(300));
    if map.len() >= 4096 && !map.contains_key(&ip) { return None; }
    let p = map.entry(ip).or_insert(Admission { active: 0, tokens: 12.0, seen: Instant::now() });
    p.tokens = (p.tokens + p.seen.elapsed().as_secs_f32() / 3.0).min(12.0); p.seen = Instant::now();
    if p.active >= 8 || p.tokens < 1.0 { return None; }
    p.tokens -= 1.0; p.active += 1;
    Some(Permit { peers: peers.clone(), ip })
}
pub async fn serve(endpoint: Endpoint, backend: SocketAddr, status: Arc<Mutex<crate::public::MatchStatus>>) {
    let slots = Arc::new(Semaphore::new(16));
    let peers: Admissions = Arc::new(Mutex::new(HashMap::new()));
    while let Some(incoming) = endpoint.accept().await {
        if !incoming.remote_address_validated() { let _ = incoming.retry(); continue; }
        let Ok(slot) = slots.clone().try_acquire_owned() else { incoming.refuse(); continue; };
        let Some(admission) = admit(&peers, incoming.remote_address().ip()) else { incoming.refuse(); continue; };
        let status = status.clone();
        tokio::spawn(async move {
            let _slot = slot; let _admission = admission;
            if let Ok(Ok(conn)) = timeout(Duration::from_secs(4), incoming).await {
                if let Err(e) = server_connection(&conn, backend, status).await { log::debug!("QUIC session closed: {e}"); }
                conn.close(0u32.into(), b"session ended");
            }
        });
    }
}
async fn server_connection(conn: &Connection, backend: SocketAddr, status: Arc<Mutex<crate::public::MatchStatus>>) -> io::Result<()> {
    let (mut send, mut recv) = timeout(Duration::from_secs(3), conn.accept_bi()).await.map_err(io::Error::other)?.map_err(io::Error::other)?;
    let hello = timeout(Duration::from_secs(3), read_control::<ClientMsg>(&mut recv)).await.map_err(io::Error::other)??;
    if matches!(hello, ClientMsg::Status) {
        let status = status.lock().unwrap().clone();
        timeout(Duration::from_secs(1), send_control(&mut send, &ServerMsg::Status { status })).await.map_err(io::Error::other)??;
        send.finish().map_err(io::Error::other)?;
        let _ = timeout(Duration::from_secs(1), send.stopped()).await;
        return Ok(());
    }
    if !matches!(hello, ClientMsg::Hello { .. }) { return Err(invalid("expected hello")); }
    let stream = tokio::task::spawn_blocking(move || TcpStream::connect_timeout(&backend, Duration::from_secs(1))).await.map_err(io::Error::other)??;
    let mut wire = Framed::new(stream, 262_144)?; wire.send(&hello)?;
    let mut poll = tokio::time::interval(Duration::from_millis(2)); poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_seq = 0; let mut last_input = Instant::now();
    let mut tokens = 32.0f32; let mut refill = Instant::now();
    let control = read_control::<ClientMsg>(&mut recv); tokio::pin!(control);
    loop {
        tokio::select! {
            _ = &mut control => return Ok(()),
            data = conn.read_datagram() => {
                tokens = (tokens + refill.elapsed().as_secs_f32() * 120.0).min(32.0); refill = Instant::now(); tokens -= 1.0;
                if tokens < 0.0 { return Err(invalid("input rate limit")); }
                let commands = decode_inputs(&data.map_err(io::Error::other)?)?;
                for command in commands {
                    if command.seq <= last_seq { continue; }
                    if command.seq - last_seq > 1200 { return Err(invalid("input sequence jump")); }
                    wire.send(&ClientMsg::Input { command })?; last_seq = command.seq; last_input = Instant::now();
                }
            }
            _ = poll.tick() => {
                if last_input.elapsed() > Duration::from_secs(5) { return Err(invalid("input timeout")); }
                wire.flush()?;
                for message in wire.receive::<ServerMsg>()? {
                    match message {
                        ServerMsg::Snapshot { state } => {
                            for packet in chunks(&state)? { conn.send_datagram(packet.into()).map_err(io::Error::other)?; }
                        }
                        control => timeout(Duration::from_secs(1), send_control(&mut send, &control)).await.map_err(io::Error::other)??,
                    }
                }
            }
        }
    }
}
pub async fn run_server(bind: SocketAddr, cert: &[u8], key: &[u8]) -> io::Result<()> {
    let cfg = server_config(cert, key)?;
    let endpoint = Endpoint::server(cfg, bind)?;
    let host = GameHost::bind("127.0.0.1:0", "North Spine", 8, "Valley")?
        .with_password(std::env::var("PEAKRUNNER_MATCH_PASSWORD").unwrap_or_default());
    let backend = host.local_addr(); let game = host.spawn();
    let status = game.status.clone();
    let public_status = status.clone();
    let health = axum::Router::new().route("/status", axum::routing::get(move || {
        let status = status.clone(); async move { axum::Json(status.lock().unwrap().clone()) }
    }));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    let health_task = tokio::spawn(async move { let _ = axum::serve(listener, health).await; });
    log::info!("Encrypted UDP match listening on {bind}");
    let stop = endpoint.clone();
    tokio::select! {
        _ = serve(endpoint, backend, public_status) => {},
        _ = shutdown() => stop.close(0u32.into(), b"server restarting"),
    }
    health_task.abort(); drop(game); Ok(())
}
async fn shutdown() {
    #[cfg(unix)] { let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
        tokio::select! { _ = term.recv() => {}, _ = tokio::signal::ctrl_c() => {} } }
    #[cfg(not(unix))] { let _ = tokio::signal::ctrl_c().await; }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn encrypted_udp_verifies_identity_and_delivers_authoritative_snapshots() {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let server = Endpoint::server(server_config(cert.cert.pem().as_bytes(), cert.key_pair.serialize_pem().as_bytes()).unwrap(), "127.0.0.1:0".parse().unwrap()).unwrap();
        let address = server.local_addr().unwrap();
        let host = GameHost::bind("127.0.0.1:0", "QUIC test", 8, "Valley").unwrap();
        let backend = host.local_addr(); let game = host.spawn();
        let task = tokio::spawn(serve(server.clone(), backend, game.status.clone()));
        let mut client = Endpoint::client("127.0.0.1:0".parse().unwrap()).unwrap();
        client.set_default_client_config(client_config(None).unwrap());
        assert!(timeout(Duration::from_secs(3), client.connect(address, "localhost").unwrap()).await.unwrap().is_err(), "untrusted certificate accepted");
        client.set_default_client_config(client_config(Some(cert.cert.der().clone())).unwrap());
        assert!(timeout(Duration::from_secs(3), client.connect(address, "wrong.example").unwrap()).await.unwrap().is_err(), "wrong hostname accepted");
        let conn = timeout(Duration::from_secs(3), client.connect(address, "localhost").unwrap()).await.unwrap().unwrap();
        let (mut send, mut recv) = conn.open_bi().await.unwrap();
        send_control(&mut send, &ClientMsg::Hello { name: "UDP test".into(), protocol: crate::proto::PROTOCOL.into(), password: String::new() }).await.unwrap();
        let welcome = timeout(Duration::from_secs(3), read_control::<ServerMsg>(&mut recv)).await.unwrap().unwrap();
        let ServerMsg::Welcome { player_id, .. } = welcome else { panic!("join rejected") };
        let mut assembly = Reassembly::default();
        let state = timeout(Duration::from_secs(3), async {
            loop { if let Some(s) = assembly.accept(&conn.read_datagram().await.unwrap()).unwrap() { break s; } }
        }).await.unwrap();
        assert!(state.players.iter().any(|p| p.net_id == player_id));
        conn.send_datagram(vec![0u8; 300].into()).unwrap();
        timeout(Duration::from_secs(3), conn.closed()).await.expect("malformed input was not disconnected");
        server.close(0u32.into(), b"test complete"); client.close(0u32.into(), b"test complete");
        task.await.unwrap(); drop(game);
    }
    #[test] fn lost_reordered_duplicate_and_oversized_datagrams_are_bounded() {
        let mut game = peakrunner_core::sim::Match::new(peakrunner_core::terrain::MapId::Valley);
        game.join(1, "One"); game.join(2, "Two"); game.step(&[]);
        let first = chunks(&game.snapshot()).unwrap();
        let mut reassembly = Reassembly::default();
        // Lose the first fragment; a newer snapshot must still complete.
        for part in first.iter().skip(1) { assert!(reassembly.accept(part).unwrap().is_none()); }
        game.step(&[]); let second = chunks(&game.snapshot()).unwrap();
        let mut decoded = None;
        for part in second.iter().rev() { if let Some(s) = reassembly.accept(part).unwrap() { decoded = Some(s); } }
        assert_eq!(decoded.unwrap().tick, game.tick);
        for part in &first { assert!(reassembly.accept(part).unwrap().is_none()); }
        for part in &second { assert!(reassembly.accept(part).unwrap().is_none()); }
        assert!(reassembly.accept(&[0; 2000]).is_err());
        for _ in 0..12 { game.step(&[]); let parts = chunks(&game.snapshot()).unwrap(); reassembly.accept(&parts[0]).unwrap(); }
        assert!(reassembly.frames.len() <= 4);
    }
    #[test] fn input_redundancy_preserves_sequence_and_rejects_forgery() {
        let history = (7..10).map(|seq| Command { seq, ..Command::default() }).collect();
        let bytes = inputs(&history); assert!(bytes.len() <= 256);
        assert_eq!(decode_inputs(&bytes).unwrap().iter().map(|c| c.seq).collect::<Vec<_>>(), vec![7,8,9]);
        assert!(decode_inputs(&[0; 300]).is_err());
        let mut bad = Inputs { count: 1, frames: [Command::default(); 3] };
        bad.frames[0] = Command { seq: 1, move_x: 2.0, ..Command::default() };
        assert!(decode_inputs(&postcard::to_allocvec(&bad).unwrap()).is_err());
        assert!(endpoint_url("quic://play.peakrunner.net:7777").is_ok());
        assert!(endpoint_url("quic://user:pass@play.peakrunner.net:7777").is_err());
    }
}
