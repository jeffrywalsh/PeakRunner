use std::io;
use std::sync::{Arc, Mutex, mpsc::{self, SyncSender}, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::{Duration, Instant};
use peakrunner_core::sim::{Command, Snapshot};
use crate::{directory, proto::{ClientMsg, ServerAdvert, ServerMsg, PROTOCOL}, transport::Transport};

#[derive(Clone, Debug, Default)]
pub struct Lobby {
    pub connected: bool, pub player_id: u32, pub match_name: String, pub map: String,
    pub players: Vec<String>, pub snapshot: Option<Snapshot>, pub error: Option<String>,
}
pub struct Session {
    lobby: Arc<Mutex<Lobby>>, outbound: SyncSender<Command>, stop: Arc<AtomicBool>,
}
impl Drop for Session { fn drop(&mut self) { self.stop.store(true, Ordering::Relaxed); } }
impl Session {
    pub fn lobby(&self) -> Lobby { self.lobby.lock().expect("lobby").clone() }
    pub fn leave(self) { self.stop.store(true, Ordering::Relaxed); }
    pub fn send_input(&self, command: Command) -> bool { self.outbound.try_send(command).is_ok() }
}
pub fn browse(addr: &str) -> io::Result<Vec<ServerAdvert>> {
    if addr.starts_with("https://") { return peakrunner_discovery::http::browse_https(addr); }
    if addr.contains("://") { return Err(io::Error::other("public directories require https://")); }
    directory::list(addr)
}
pub fn connect(host: &str, port: u16, name: &str) -> io::Result<Session> { connect_private(host, port, name, "") }
pub fn connect_private(host: &str, port: u16, name: &str, password: &str) -> io::Result<Session> {
    let lobby = Arc::new(Mutex::new(Lobby::default()));
    let (outbound, rx) = mpsc::sync_channel(32);
    let stop = Arc::new(AtomicBool::new(false));
    let state = lobby.clone();
    let flag = stop.clone();
    let address = if host.contains("://") { host.to_string() } else { match host.trim_matches(['[', ']']).parse::<std::net::IpAddr>() {
        Ok(ip) => std::net::SocketAddr::new(ip, port).to_string(),
        Err(_) => format!("{host}:{port}"),
    }};
    let hello = ClientMsg::Hello { name: name.into(), protocol: PROTOCOL.into(), password: password.into() };
    thread::Builder::new().name("peakrunner-session".into()).spawn(move || {
        let result = (|| -> io::Result<()> {
            if address.starts_with("quic://") {
                return tokio::runtime::Builder::new_current_thread().enable_all().build()?
                    .block_on(crate::quic::run_client(&address, hello, rx, state.clone(), flag.clone()));
            }
            let mut wire = Transport::connect(&address)?;
            wire.send(&hello)?;
            let mut last = Instant::now();
            loop {
                if flag.load(Ordering::Relaxed) { let _ = wire.send(&ClientMsg::Leave); return Ok(()); }
                for _ in 0..8 {
                    match rx.try_recv() {
                        Ok(command) => wire.send(&ClientMsg::Input { command })?,
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
                    }
                }
                for msg in wire.receive::<ServerMsg>()? {
                    last = Instant::now();
                    let mut lobby = state.lock().expect("lobby");
                    match msg {
                        ServerMsg::Welcome { player_id, match_name } => {
                            lobby.player_id = player_id; lobby.match_name = match_name;
                        }
                        ServerMsg::Snapshot { state } => {
                            if state.players.len() != peakrunner_core::sim::MAX_PLAYERS
                                || state.acks.len() != state.players.len()
                                || !state.players.iter().any(|p| p.net_id == lobby.player_id) {
                                return Err(io::Error::other("invalid server snapshot"));
                            }
                            lobby.map = format!("{:?}", state.map);
                            lobby.players = state.players.iter().filter(|p| p.net_id != 0).map(|p| p.name.clone()).collect();
                            lobby.snapshot = Some(state);
                            lobby.connected = true;
                        }
                        ServerMsg::Reject { message } => return Err(io::Error::other(message)),
                        ServerMsg::Status { .. } => return Err(io::Error::other("unexpected server status")),
                    }
                }
                if last.elapsed() > Duration::from_secs(5) { return Err(io::Error::other("server timed out")); }
                wire.flush()?;
                thread::sleep(Duration::from_millis(2));
            }
        })();
        let mut lobby = state.lock().expect("lobby");
        lobby.connected = false;
        if let Err(e) = result {
            lobby.error = Some(match e.kind() {
                io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::ConnectionAborted | io::ErrorKind::BrokenPipe =>
                    "The host closed the match or the connection was lost.".into(),
                io::ErrorKind::ConnectionRefused => "No match server is listening at that address.".into(),
                _ => e.to_string(),
            });
            lobby.players.clear();
            lobby.snapshot = None;
        }
    })?;
    Ok(Session { lobby, outbound, stop })
}
