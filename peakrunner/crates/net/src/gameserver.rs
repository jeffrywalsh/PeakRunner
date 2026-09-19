use std::collections::VecDeque;
use std::io;
use std::net::TcpListener;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, AtomicU32, Ordering}};
use std::thread;
use std::time::{Duration, Instant};
use peakrunner_core::{sim::{Command, Match, MAX_PLAYERS, STEP}, terrain::MapId};
use crate::proto::{ClientMsg, ServerMsg, PROTOCOL};
use crate::wire::{Framed, private_bind, invalid};

pub struct GameHost {
    listener: TcpListener, name: String, map: MapId, max_players: usize, password: String,
}
pub struct GameHandle {
    stop: Arc<AtomicBool>, thread: Option<thread::JoinHandle<()>>,
    pub players: Arc<AtomicU32>,
    pub status: Arc<Mutex<crate::public::MatchStatus>>,
}
impl Drop for GameHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() { let _ = t.join(); }
    }
}
struct Peer {
    wire: Framed, slot: Option<usize>, accepted: Instant, last: Instant,
    queue: VecDeque<Command>, current: Command, last_seq: u64, tokens: f32,
}
impl GameHost {
    pub fn bind(addr: &str, name: &str, max_players: u32, map: &str) -> io::Result<Self> {
        let map = match map.to_lowercase().as_str() {
            "valley" => MapId::Valley, "raindance" => MapId::Raindance,
            _ => return Err(invalid("unknown map: use Valley or Raindance")),
        };
        if !(2..=MAX_PLAYERS as u32).contains(&max_players) { return Err(invalid("capacity must be 2–8")); }
        if name.is_empty() || name.len() > 64 || name.chars().any(char::is_control) { return Err(invalid("invalid match name")); }
        let listener = TcpListener::bind(private_bind(addr)?)?;
        listener.set_nonblocking(true)?;
        Ok(Self { listener, name: name.into(), map, max_players: max_players as usize, password: String::new() })
    }
    pub fn with_password(mut self, password: String) -> Self { self.password = password; self }
    pub fn local_addr(&self) -> std::net::SocketAddr { self.listener.local_addr().expect("bound") }
    pub fn spawn(self) -> GameHandle {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let players = Arc::new(AtomicU32::new(0));
        let count = players.clone();
        let status = Arc::new(Mutex::new(crate::public::MatchStatus::default()));
        let stats = status.clone();
        let thread = thread::spawn(move || self.run(flag, count, stats));
        GameHandle { stop, thread: Some(thread), players, status }
    }
    fn run(self, stop: Arc<AtomicBool>, count: Arc<AtomicU32>, status: Arc<Mutex<crate::public::MatchStatus>>) {
        let mut game = Match::new(self.map);
        let mut peers: Vec<Peer> = Vec::new();
        let mut id = 0u32;
        let step = Duration::from_secs_f64(STEP as f64);
        let mut next = Instant::now();
        while !stop.load(Ordering::Relaxed) {
            let now = Instant::now();
            if now < next { thread::sleep((next - now).min(Duration::from_millis(5))); continue; }
            next += step;
            // Never repay an unbounded wall-clock backlog after suspension.
            if now.duration_since(next.min(now)) > Duration::from_millis(100) { next = now + step; }
            for _ in 0..8 {
                match self.listener.accept() {
                    Ok((stream, _)) if peers.len() < 16 => {
                        if let Ok(wire) = Framed::new(stream, 2048) {
                            peers.push(Peer { wire, slot: None, accepted: now, last: now,
                                queue: VecDeque::new(), current: Command::default(), last_seq: 0, tokens: 32.0 });
                        }
                    }
                    Ok(_) => {}
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(_) => break,
                }
            }
            let mut commands = vec![None; MAX_PLAYERS];
            peers.retain_mut(|peer| {
                peer.tokens = (peer.tokens + 2.0).min(32.0);
                let result = (|| -> io::Result<()> {
                    if peer.slot.is_none() && now.duration_since(peer.accepted) > Duration::from_secs(3) {
                        return Err(invalid("handshake timeout"));
                    }
                    if now.duration_since(peer.last) > Duration::from_secs(5) { return Err(invalid("input timeout")); }
                    for msg in peer.wire.receive::<ClientMsg>()? {
                        peer.tokens -= 1.0;
                        if peer.tokens < 0.0 { return Err(invalid("message rate exceeded")); }
                        match msg {
                            ClientMsg::Hello { name, protocol, password } if peer.slot.is_none() => {
                                let name = name.trim();
                                if protocol != PROTOCOL { return Err(invalid("incompatible game version")); }
                                if password != self.password { return Err(invalid("incorrect match password")); }
                                if name.is_empty() || name.len() > 24 || name.chars().any(char::is_control) {
                                    return Err(invalid("name must be 1–24 bytes without control characters"));
                                }
                                if game.world.players.iter().filter(|p| p.net_id != 0).count() >= self.max_players {
                                    return Err(invalid("match is full"));
                                }
                                id = id.checked_add(1).ok_or_else(|| invalid("identity limit"))?;
                                let slot = game.join(id, name).ok_or_else(|| invalid("match is full"))?;
                                peer.slot = Some(slot);
                                log::info!("player_join id={id} slot={slot}");
                                peer.wire.send(&ServerMsg::Welcome { player_id: id, match_name: self.name.clone() })?;
                            }
                            ClientMsg::Input { command } if peer.slot.is_some() => {
                                if !command.valid() || command.seq <= peer.last_seq || command.seq - peer.last_seq > 1200 {
                                    return Err(invalid("invalid or out-of-order input"));
                                }
                                if peer.queue.len() >= 12 { peer.queue.pop_front(); }
                                peer.last_seq = command.seq;
                                peer.last = now;
                                peer.queue.push_back(command);
                            }
                            ClientMsg::Leave => return Err(invalid("left match")),
                            _ => return Err(invalid("unexpected message")),
                        }
                    }
                    if let Some(slot) = peer.slot {
                        // A delayed burst must not become permanent input lag or
                        // buy extra simulation steps. Sample newest held state once
                        // per server tick; acknowledgements retire skipped inputs.
                        if let Some(command) = peer.queue.pop_back() { peer.current = command; }
                        peer.queue.clear();
                        if now.duration_since(peer.last) <= Duration::from_millis(250) {
                            commands[slot] = Some(peer.current);
                        }
                    }
                    peer.wire.flush()
                })();
                if let Err(e) = result {
                    if let Some(slot) = peer.slot { log::info!("player_leave id={} reason={e}", game.world.players[slot].net_id); }
                    let _ = peer.wire.send(&ServerMsg::Reject { message: e.to_string() });
                    if let Some(slot) = peer.slot { game.leave(slot); commands[slot] = None; }
                    false
                } else { true }
            });
            let previous_phase = game.phase;
            game.step(&commands);
            if game.phase != previous_phase {
                log::info!("match_phase round={} phase={:?} score={:?}", game.round, game.phase, game.world.score);
            }
            if game.tick % 3 == 0 {
                let msg = ServerMsg::Snapshot { state: game.snapshot() };
                peers.retain_mut(|peer| {
                    if peer.slot.is_none() { return true; }
                    if peer.wire.send(&msg).is_err() {
                        game.leave(peer.slot.unwrap()); false
                    } else { true }
                });
            }
            count.store(game.world.players.iter().filter(|p| p.net_id != 0).count() as u32, Ordering::Relaxed);
            if game.tick % 3 == 0 {
                *status.lock().expect("match status") = crate::public::MatchStatus {
                    name: self.name.clone(), map: format!("{:?}", self.map),
                    players: count.load(Ordering::Relaxed), max_players: self.max_players as u32,
                    tick: game.tick, round: game.round, phase: format!("{:?}", game.phase),
                    score: game.world.score, time_left: game.world.time_left,
                    password_required: !self.password.is_empty(),
                };
            }
        }
    }
}

pub fn run_from_args() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let mut bind = "127.0.0.1".to_string();
    let mut port = 7781u16;
    let mut name = "Open rift".to_string();
    let mut map = "Valley".to_string();
    let mut directory: Option<String> = None;
    let mut advertise: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let Some(value) = args.next() else { eprintln!("missing value for {arg}"); return; };
        match arg.as_str() {
            "--bind" => bind = value, "--port" => port = value.parse().expect("port"),
            "--name" => name = value, "--map" => map = value,
            "--directory" => directory = Some(value), "--advertise" => advertise = Some(value),
            _ => { eprintln!("unknown option {arg}; use --bind, --port, --name, --map, --directory, --advertise"); return; }
        }
    }
    let password = std::env::var("PEAKRUNNER_MATCH_PASSWORD").unwrap_or_default();
    let bind_ip: std::net::IpAddr = bind.parse().expect("--bind must be a specific private IP");
    let host = GameHost::bind(&std::net::SocketAddr::new(bind_ip, port).to_string(), &name, 8, &map)
        .expect("private game server bind").with_password(password);
    println!("PeakRunner: {name}, {map}, {}, 8 player CTF; private LAN/VPN transport", host.local_addr());
    let handle = host.spawn();
    let mut lease: Option<crate::directory::Lease> = None;
    loop {
        if let Some(dir) = directory.as_deref() {
            let players = handle.players.load(Ordering::Relaxed);
            let result = if let Some(l) = &lease {
                crate::directory::heartbeat(dir, l, players).map(|_| ())
            } else {
                crate::directory::register(dir, &name, advertise.as_deref().unwrap_or(&bind), port, players, 8, &map)
                    .map(|l| { lease = Some(l); })
            };
            if let Err(e) = result { log::warn!("directory: {e}"); lease = None; }
        }
        thread::sleep(Duration::from_secs(2));
    }
}
