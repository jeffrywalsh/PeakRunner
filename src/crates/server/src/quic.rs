//! Server-only QUIC admission, input validation and authoritative snapshots.
use std::{collections::HashMap, io, net::{IpAddr, SocketAddr, TcpStream}, sync::{Arc, Mutex}, time::{Duration, Instant}};
use quinn::{Connection, Endpoint};
use tokio::{sync::Semaphore, time::timeout};
use crate::{GameHost, proto::{ClientMsg, ServerMsg}, wire::{Framed, invalid}};
use peakrunner_discovery::quic::{ALPN, transport, read_control, send_control};
use peakrunner_protocol::packets::{decode_inputs, chunks};
#[cfg(test)]
use peakrunner_discovery::quic::{client_config, endpoint_url};
#[cfg(test)]
use peakrunner_protocol::packets::{Inputs, Reassembly, inputs};
#[cfg(test)]
use peakrunner_core::sim::Command;
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
struct Admission { active: usize, tokens: f32, seen: Instant }
type Admissions = Arc<Mutex<HashMap<IpAddr, Admission>>>;
type Pings = Arc<Mutex<HashMap<u32, u32>>>;
struct PingEntry { pings: Pings, id: Option<u32> }
impl Drop for PingEntry {
    fn drop(&mut self) { if let Some(id) = self.id { self.pings.lock().unwrap().remove(&id); } }
}
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
    // Eight players behind one NAT still leave room for directory status and
    // one pending admission. The authoritative match itself remains eight slots.
    if p.active >= 10 || p.tokens < 1.0 { return None; }
    p.tokens -= 1.0; p.active += 1;
    Some(Permit { peers: peers.clone(), ip })
}
pub async fn serve(endpoint: Endpoint, backend: SocketAddr, status: Arc<Mutex<peakrunner_discovery::MatchStatus>>) {
    let slots = Arc::new(Semaphore::new(16));
    let peers: Admissions = Arc::new(Mutex::new(HashMap::new()));
    let pings: Pings = Arc::new(Mutex::new(HashMap::new()));
    while let Some(incoming) = endpoint.accept().await {
        if !incoming.remote_address_validated() { let _ = incoming.retry(); continue; }
        let Ok(slot) = slots.clone().try_acquire_owned() else { incoming.refuse(); continue; };
        let Some(admission) = admit(&peers, incoming.remote_address().ip()) else { incoming.refuse(); continue; };
        let status = status.clone();
        let pings = pings.clone();
        tokio::spawn(async move {
            let _slot = slot; let _admission = admission;
            if let Ok(Ok(conn)) = timeout(Duration::from_secs(4), incoming).await {
                if let Err(e) = server_connection(&conn, backend, status, pings).await { log::debug!("QUIC session closed: {e}"); }
                conn.close(0u32.into(), b"session ended");
            }
        });
    }
}
async fn next_control(mut recv: quinn::RecvStream) -> (quinn::RecvStream, io::Result<ClientMsg>) {
    let message = read_control(&mut recv).await;
    (recv, message)
}
async fn server_connection(conn: &Connection, backend: SocketAddr, status: Arc<Mutex<peakrunner_discovery::MatchStatus>>, pings: Pings) -> io::Result<()> {
    let mut ping = PingEntry { pings, id: None };
    let empty_datagram_space = conn.datagram_send_buffer_space();
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
    let mut chat_tokens = 8.0f32; let mut chat_refill = Instant::now();
    let control = next_control(recv); tokio::pin!(control);
    loop {
        tokio::select! {
            (stream, message) = &mut control => {
                chat_tokens = (chat_tokens + chat_refill.elapsed().as_secs_f32() * 2.0).min(8.0);
                chat_refill = Instant::now(); chat_tokens -= 1.0;
                if chat_tokens < 0.0 { return Err(invalid("chat rate limit")); }
                match message? {
                    ClientMsg::Chat { text } if text.len() <= peakrunner_core::feed::MAX_BYTES => wire.send(&ClientMsg::Chat { text })?,
                    ClientMsg::TeamChat { text } if text.len() <= peakrunner_core::feed::MAX_BYTES => wire.send(&ClientMsg::TeamChat { text })?,
                    ClientMsg::Rename { name } if name.len() <= peakrunner_core::names::MAX_LEN => wire.send(&ClientMsg::Rename { name })?,
                    ClientMsg::Leave => return Ok(()),
                    _ => return Err(invalid("unexpected control message")),
                }
                control.set(next_control(stream));
            },
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
                        ServerMsg::Snapshot { mut state } => {
                            if let Some(id) = ping.id {
                                let mut values = ping.pings.lock().unwrap();
                                values.insert(id, conn.rtt().as_millis().min(u32::MAX as u128) as u32);
                                state.pings = state.players.iter().filter_map(|p|
                                    values.get(&p.net_id).map(|ms| (p.net_id, *ms))).collect();
                            }
                            // Finish the one pending snapshot instead of mixing
                            // fragments from multiple ticks or queuing old frames.
                            // Under congestion, skip intermediate snapshots; once
                            // drained, send the newest state on the next 20Hz tick.
                            if conn.datagram_send_buffer_space() < empty_datagram_space { continue; }
                            for packet in chunks(&state)? { conn.send_datagram(packet.into()).map_err(io::Error::other)?; }
                        }
                        control => {
                            if let ServerMsg::Welcome { player_id, .. } = &control { ping.id = Some(*player_id); }
                            timeout(Duration::from_secs(1), send_control(&mut send, &control)).await.map_err(io::Error::other)??;
                        }
                    }
                }
            }
        }
    }
}
pub async fn run_server(bind: SocketAddr, cert: &[u8], key: &[u8]) -> io::Result<()> {
    let cfg = server_config(cert, key)?;
    let endpoint = Endpoint::server(cfg, bind)?;
    let map = std::env::var("PEAKRUNNER_MATCH_MAP").unwrap_or_else(|_| "Raindance".into());
    let name = std::env::var("PEAKRUNNER_MATCH_NAME").unwrap_or_else(|_| "Springdale Central".into());
    let mut host = GameHost::bind("127.0.0.1:0", &name, 8, &map)?
        .with_password(std::env::var("PEAKRUNNER_MATCH_PASSWORD").unwrap_or_default());
    if let Some(rotation) = std::env::var_os("PEAKRUNNER_MATCH_ROTATION") {
        let rotation = rotation.into_string().map_err(|_| invalid("rotation must be UTF-8 JSON"))?;
        host = host.with_rotation(&rotation)?;
    }
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
    #[test] fn a_full_shared_nat_leaves_room_for_status_without_unbounded_admission() {
        let peers: Admissions = Arc::new(Mutex::new(HashMap::new()));
        let ip = "127.0.0.1".parse().unwrap();
        let players: Vec<_> = (0..8).map(|_| admit(&peers, ip).unwrap()).collect();
        let status = admit(&peers, ip).expect("full match blocked directory status");
        let pending = admit(&peers, ip).unwrap();
        assert!(admit(&peers, ip).is_none());
        drop(status); drop(pending); drop(players);
        assert_eq!(peers.lock().unwrap()[&ip].active, 0);
    }
    #[test] fn compressed_snapshot_limits_cover_eight_player_weapon_load() {
        let mut game = peakrunner_core::sim::Match::new(peakrunner_core::terrain::MapId::Valley);
        for id in 1..=8 { game.join(id, &format!("Player{id}")).unwrap(); }
        game.phase = peakrunner_core::sim::Phase::Playing;
        for n in 0..peakrunner_core::feed::CAPACITY {
            peakrunner_core::feed::push(&mut game.world.feed, peakrunner_core::feed::Entry::Chat {
                sender:format!("Player{n}"),text:(0..160).map(|i| char::from(b'!' + ((i*17+n*7)%90) as u8)).collect(),
            });
        }
        let mut largest = 0;
        for seq in 1..=900 {
            let commands: Vec<_> = (0..8).map(|i| Some(Command { seq, yaw:i as f32,
                fire:true, jet:true, jump:true, weapon:((seq/150)%3) as u8, ..Default::default() })).collect();
            game.step(&commands);
            if seq%3 == 0 {
                let packets = chunks(&game.snapshot()).unwrap();
                largest = largest.max(packets.len());
                assert!(packets.iter().map(Vec::len).sum::<usize>() < 16 * 1024);
            }
        }
        eprintln!("largest compressed eight-player snapshot: {largest} fragments");
        let mut malicious = 1u64.to_le_bytes().to_vec();
        malicious.extend_from_slice(&[0,1]); malicious.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(Reassembly::default().accept(&malicious).is_err());
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn encrypted_udp_verifies_identity_and_delivers_authoritative_snapshots() {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let server = Endpoint::server(server_config(cert.cert.pem().as_bytes(), cert.key_pair.serialize_pem().as_bytes()).unwrap(), "127.0.0.1:0".parse().unwrap()).unwrap();
        let address = server.local_addr().unwrap();
        let host = GameHost::bind("127.0.0.1:0", "QUIC test", 8, "Valley").unwrap();
        let backend = host.local_addr(); let game = host.spawn();
        timeout(Duration::from_secs(1), async {
            while game.status.lock().unwrap().tick == 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }).await.expect("simulation did not publish its first status");
        let task = tokio::spawn(serve(server.clone(), backend, game.status.clone()));
        let mut client = Endpoint::client("127.0.0.1:0".parse().unwrap()).unwrap();
        client.set_default_client_config(client_config(None).unwrap());
        assert!(timeout(Duration::from_secs(3), client.connect(address, "localhost").unwrap()).await.unwrap().is_err(), "untrusted certificate accepted");
        client.set_default_client_config(client_config(Some(cert.cert.der().clone())).unwrap());
        assert!(timeout(Duration::from_secs(3), client.connect(address, "wrong.example").unwrap()).await.unwrap().is_err(), "wrong hostname accepted");
        // The directory speaks only the discovery contract, not gameplay messages.
        let status_conn = client.connect(address, "localhost").unwrap().await.unwrap();
        let (mut status_send, mut status_recv) = status_conn.open_bi().await.unwrap();
        send_control(&mut status_send, &peakrunner_discovery::StatusRequest::Status).await.unwrap();
        let peakrunner_discovery::StatusReply::Status { status } =
            timeout(Duration::from_secs(3), read_control(&mut status_recv)).await.unwrap().unwrap();
        assert_eq!(status.name, "QUIC test");
        assert_eq!(status.map, "Valley - CTF");
        assert_eq!(status.players, 0);
        status_conn.close(0u32.into(), b"status test complete");
        let conn = timeout(Duration::from_secs(3), client.connect(address, "localhost").unwrap()).await.unwrap().unwrap();
        let (mut send, mut recv) = conn.open_bi().await.unwrap();
        send_control(&mut send, &ClientMsg::Hello { name: "UDP test".into(), protocol: crate::proto::game_protocol(), password: String::new() }).await.unwrap();
        let welcome = timeout(Duration::from_secs(3), read_control::<ServerMsg>(&mut recv)).await.unwrap().unwrap();
        let ServerMsg::Welcome { player_id, .. } = welcome else { panic!("join rejected") };
        let mut assembly = Reassembly::default();
        let state = timeout(Duration::from_secs(3), async {
            loop { if let Some(s) = assembly.accept(&conn.read_datagram().await.unwrap()).unwrap() { break s; } }
        }).await.unwrap();
        assert!(state.players.iter().any(|p| p.net_id == player_id));
        assert!(state.pings.iter().any(|(id,_)| *id == player_id), "server RTT missing");
        send_control(&mut send, &ClientMsg::Rename { name: "UDP Pilot 42".into() }).await.unwrap();
        timeout(Duration::from_secs(3), async {
            loop {
                if let Some(s) = assembly.accept(&conn.read_datagram().await.unwrap()).unwrap() {
                    if s.players.iter().any(|p| p.net_id == player_id && p.name == "UDP Pilot 42") { break; }
                }
            }
        }).await.expect("reliable rename missing from snapshots");
        // Multiple reliable control frames must not close the UDP session or
        // lose framing when snapshots arrive between partial stream reads.
        send_control(&mut send, &ClientMsg::Chat { text: "Ready for a match".into() }).await.unwrap();
        timeout(Duration::from_secs(3), async {
            loop {
                if let Some(s) = assembly.accept(&conn.read_datagram().await.unwrap()).unwrap() {
                    if s.feed.iter().any(|e| matches!(e, peakrunner_core::feed::Entry::Chat { sender, text } if sender == "UDP Pilot 42" && text == "Ready for a match")) { break; }
                }
            }
        }).await.expect("reliable chat missing from authoritative snapshots");
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
        assert!(reassembly.pending_frames() <= 4);
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
