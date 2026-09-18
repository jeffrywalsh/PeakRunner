use std::io::{BufReader, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::directory;
use crate::proto::{ClientMsg, ServerMsg, PROTOCOL};
use crate::wire::{read_msg, write_msg};

struct Guest {
    id: u32,
    name: String,
    tx: Sender<String>,
    pose: Option<crate::proto::Pose>,
}

struct Table {
    next: u32,
    host: String,
    guests: Vec<Guest>,
}

struct DirectoryLink {
    addr: String,
    id: Mutex<Option<String>>,
}

pub struct GameHost {
    listener: TcpListener,
    name: String,
    map: String,
    max_players: u32,
}

impl GameHost {
    pub fn bind(addr: &str, name: &str, max_players: u32, map: &str) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        Ok(Self {
            listener,
            name: name.into(),
            map: map.into(),
            max_players,
        })
    }

    pub fn local_addr(&self) -> std::net::SocketAddr {
        self.listener.local_addr().expect("local addr")
    }

    pub fn spawn(self) {
        let name = self.name.clone();
        let map = self.map.clone();
        let max_players = self.max_players;
        thread::Builder::new()
            .name("peakrunner-server".into())
            .spawn(move || {
                if let Err(err) = accept_loop(
                    self.listener,
                    name,
                    map,
                    max_players,
                    "Host".into(),
                    Arc::new(AtomicU32::new(1)),
                    None,
                ) {
                    log::error!("game server stopped: {err}");
                }
            })
            .expect("server thread");
    }
}

pub fn run_from_args() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let mut directory = "192.168.1.64:7780".to_string();
    let mut port: u16 = 7781;
    let mut name = "Open rift".to_string();
    let mut advertise = lan_ip();
    let mut map = "Valley".to_string();
    let mut host_name = "Host".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--directory" => directory = args.next().unwrap_or(directory),
            "--port" => port = args.next().and_then(|v| v.parse().ok()).unwrap_or(port),
            "--name" => name = args.next().unwrap_or(name),
            "--advertise" => advertise = args.next().unwrap_or(advertise),
            "--map" => map = args.next().unwrap_or(map),
            "--host" => host_name = args.next().unwrap_or(host_name),
            other => {
                eprintln!("unknown argument {other}");
                eprintln!(
                    "usage: peakrunner-server [--directory HOST:PORT] [--port N] [--name NAME] [--host NAME] [--advertise HOST] [--map NAME]"
                );
                std::process::exit(2);
            }
        }
    }

    let host = match GameHost::bind(&format!("0.0.0.0:{port}"), &name, 8, &map) {
        Ok(host) => host,
        Err(err) => {
            eprintln!("could not listen on {port}: {err}");
            std::process::exit(1);
        }
    };
    let bound = host.local_addr().port();
    println!("PeakRunner server \"{name}\" listening on 0.0.0.0:{bound}");
    println!("advertising {advertise}:{bound} to directory {directory}");
    let players = Arc::new(AtomicU32::new(1));
    let link = Arc::new(DirectoryLink {
        addr: directory.clone(),
        id: Mutex::new(None),
    });
    let directory_for_beat = directory.clone();
    let name_for_beat = name.clone();
    let map_for_beat = map.clone();
    let players_for_beat = Arc::clone(&players);
    let link_for_beat = Arc::clone(&link);
    thread::spawn(move || {
        advertise_loop(
            directory_for_beat,
            name_for_beat,
            advertise,
            bound,
            map_for_beat,
            players_for_beat,
            link_for_beat,
        )
    });
    println!("host {host_name} is seated; others can join");
    if let Err(err) = accept_loop(host.listener, name, map, 8, host_name, players, Some(link)) {
        eprintln!("server stopped: {err}");
        std::process::exit(1);
    }
}

fn advertise_loop(
    directory: String,
    name: String,
    host: String,
    port: u16,
    map: String,
    players: Arc<AtomicU32>,
    link: Arc<DirectoryLink>,
) {
    let mut id: Option<String> = None;
    loop {
        let players = players.load(Ordering::Relaxed);
        let result = if let Some(id) = id.as_deref() {
            directory::heartbeat(&directory, id, players).map(|_| id.to_string())
        } else {
            directory::register(&directory, &name, &host, port, players, 8, &map)
        };
        match result {
            Ok(current) => {
                if id.as_deref() != Some(current.as_str()) {
                    println!("listed on directory as {current} with {players} connected");
                }
                id = Some(current.clone());
                *link.id.lock().expect("directory id") = Some(current);
            }
            Err(err) => {
                eprintln!("directory: {err}");
                id = None;
                *link.id.lock().expect("directory id") = None;
            }
        }
        thread::sleep(Duration::from_secs(2));
    }
}

fn accept_loop(
    listener: TcpListener,
    name: String,
    map: String,
    max_players: u32,
    host_name: String,
    players: Arc<AtomicU32>,
    link: Option<Arc<DirectoryLink>>,
) -> std::io::Result<()> {
    let table = Arc::new(Mutex::new(Table {
        next: 2,
        host: host_name,
        guests: Vec::new(),
    }));
    players.store(1, Ordering::Relaxed);
    for conn in listener.incoming() {
        let stream = match conn {
            Ok(stream) => stream,
            Err(err) => {
                log::warn!("accept: {err}");
                continue;
            }
        };
        let table = Arc::clone(&table);
        let players = Arc::clone(&players);
        let link = link.clone();
        let name = name.clone();
        let map = map.clone();
        thread::spawn(move || {
            if let Err(err) = greet(stream, table, players, link, name, map, max_players) {
                log::debug!("guest left: {err}");
            }
        });
    }
    Ok(())
}

fn greet(
    stream: TcpStream,
    table: Arc<Mutex<Table>>,
    players: Arc<AtomicU32>,
    link: Option<Arc<DirectoryLink>>,
    match_name: String,
    map: String,
    max_players: u32,
) -> std::io::Result<()> {
    stream.set_nodelay(true)?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let hello: ClientMsg = read_msg(&mut reader)?;
    let ClientMsg::Hello { name, protocol } = hello else {
        let mut stream = reader.into_inner();
        write_msg(
            &mut stream,
            &ServerMsg::Reject {
                message: "expected hello".into(),
            },
        )?;
        return Ok(());
    };
    if protocol != PROTOCOL {
        let mut stream = reader.into_inner();
        write_msg(
            &mut stream,
            &ServerMsg::Reject {
                message: format!("protocol {protocol} is not {PROTOCOL}"),
            },
        )?;
        return Ok(());
    }
    let (tx, rx) = mpsc::channel::<String>();
    let id = {
        let mut table = table.lock().expect("guest table");
        if 1 + table.guests.len() as u32 >= max_players {
            drop(table);
            let mut stream = reader.into_inner();
            write_msg(
                &mut stream,
                &ServerMsg::Reject {
                    message: "rift is full".into(),
                },
            )?;
            return Ok(());
        }
        let id = table.next;
        table.next += 1;
        table.guests.push(Guest {
            id,
            name: name.clone(),
            tx: tx.clone(),
            pose: None,
        });
        id
    };
    let mut writer = stream.try_clone()?;
    thread::spawn(move || {
        while let Ok(line) = rx.recv() {
            if writer.write_all(line.as_bytes()).is_err() || writer.flush().is_err() {
                break;
            }
        }
    });
    send_one(
        &tx,
        &ServerMsg::Welcome {
            player_id: id,
            match_name,
            map,
        },
    );
    broadcast_players(&table, &players, link.as_deref());
    broadcast_poses(&table, &players, link.as_deref());
    let read = read_guest(&mut reader, id, &table, &players, link.as_deref());
    {
        let mut table = table.lock().expect("guest table");
        table.guests.retain(|guest| guest.id != id);
    }
    broadcast_players(&table, &players, link.as_deref());
    read
}

fn send_one(tx: &Sender<String>, msg: &ServerMsg) {
    if let Ok(mut line) = serde_json::to_string(msg) {
        line.push('\n');
        let _ = tx.send(line);
    }
}

fn broadcast_players(table: &Arc<Mutex<Table>>, count: &AtomicU32, link: Option<&DirectoryLink>) {
    let table = table.lock().expect("guest table");
    let mut players = vec![table.host.clone()];
    players.extend(table.guests.iter().map(|guest| guest.name.clone()));
    let connected = players.len() as u32;
    count.store(connected, Ordering::Relaxed);
    let msg = ServerMsg::Players { players };
    let Ok(mut line) = serde_json::to_string(&msg) else {
        return;
    };
    line.push('\n');
    for guest in &table.guests {
        let _ = guest.tx.send(line.clone());
    }
    drop(table);
    if let Some(link) = link {
        if let Some(id) = link.id.lock().expect("directory id").clone() {
            let _ = directory::heartbeat(&link.addr, &id, connected);
        }
    }
}

fn read_guest(
    reader: &mut BufReader<TcpStream>,
    id: u32,
    table: &Arc<Mutex<Table>>,
    players: &AtomicU32,
    link: Option<&DirectoryLink>,
) -> std::io::Result<()> {
    use std::io::BufRead;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            return Ok(());
        }
        let Ok(msg) = serde_json::from_str::<ClientMsg>(line.trim()) else {
            continue;
        };
        if let ClientMsg::Pose { x, y, z, yaw } = msg {
            let name = {
                let mut table = table.lock().expect("guest table");
                let Some(guest) = table.guests.iter_mut().find(|guest| guest.id == id) else {
                    continue;
                };
                guest.pose = Some(crate::proto::Pose {
                    name: guest.name.clone(),
                    x,
                    y,
                    z,
                    yaw,
                });
                guest.name.clone()
            };
            let _ = name;
            broadcast_poses(table, players, link);
        }
    }
}

fn broadcast_poses(table: &Arc<Mutex<Table>>, count: &AtomicU32, link: Option<&DirectoryLink>) {
    let table = table.lock().expect("guest table");
    let mut poses = vec![host_pose(&table.host)];
    for guest in &table.guests {
        if let Some(pose) = &guest.pose {
            poses.push(pose.clone());
        }
    }
    let msg = ServerMsg::Poses { poses };
    let Ok(mut line) = serde_json::to_string(&msg) else {
        return;
    };
    line.push('\n');
    for guest in &table.guests {
        let _ = guest.tx.send(line.clone());
    }
    let _ = (count, link);
}

fn host_pose(name: &str) -> crate::proto::Pose {
    crate::proto::Pose {
        name: name.into(),
        x: 128.0,
        y: 30.0,
        z: 232.0,
        yaw: 0.0,
    }
}

fn lan_ip() -> String {
    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(socket) => socket,
        Err(_) => return "127.0.0.1".into(),
    };
    if socket.connect("8.8.8.8:80").is_err() {
        return "127.0.0.1".into();
    }
    socket
        .local_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".into())
}
