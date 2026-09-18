use std::collections::HashMap;
use std::io::{BufReader, ErrorKind};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::proto::{DirRequest, DirResponse, ServerAdvert};
use crate::wire::{read_msg, resolve, write_msg};

const STALE: Duration = Duration::from_secs(8);

struct Listing {
    advert: ServerAdvert,
    seen: Instant,
}

struct State {
    next: u64,
    servers: HashMap<String, Listing>,
}

pub struct DirectoryHandle {
    pub addr: std::net::SocketAddr,
    stop: Arc<std::sync::atomic::AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for DirectoryHandle {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_millis(200));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn serve(bind: &str) -> std::io::Result<DirectoryHandle> {
    let listener = TcpListener::bind(bind)?;
    listener.set_nonblocking(true)?;
    let addr = listener.local_addr()?;
    let state = Arc::new(Mutex::new(State {
        next: 1,
        servers: HashMap::new(),
    }));
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let thread = thread::Builder::new()
        .name("peakrunner-directory".into())
        .spawn(move || accept_loop(listener, state, stop_flag))
        .expect("directory thread");
    log::info!("directory listening on {addr}");
    println!("PeakRunner directory listening on {addr}");
    Ok(DirectoryHandle {
        addr,
        stop,
        thread: Some(thread),
    })
}

fn accept_loop(listener: TcpListener, state: Arc<Mutex<State>>, stop: Arc<std::sync::atomic::AtomicBool>) {
    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let state = Arc::clone(&state);
                thread::spawn(move || {
                    if let Err(err) = handle(stream, state) {
                        log::debug!("directory client: {err}");
                    }
                });
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(25));
            }
            Err(err) => {
                log::warn!("directory accept: {err}");
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

fn handle(stream: TcpStream, state: Arc<Mutex<State>>) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_nodelay(true)?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let request: DirRequest = read_msg(&mut reader)?;
    let response = {
        let mut state = state.lock().expect("directory lock");
        sweep(&mut state);
        match request {
            DirRequest::Register {
                name,
                host,
                port,
                players,
                max_players,
                map,
            } => {
                let id = format!("rift-{}", state.next);
                state.next += 1;
                state.servers.insert(
                    id.clone(),
                    Listing {
                        advert: ServerAdvert {
                            id: id.clone(),
                            name,
                            host,
                            port,
                            players,
                            max_players,
                            map,
                        },
                        seen: Instant::now(),
                    },
                );
                log::info!("listed {id}");
                DirResponse::Registered { id }
            }
            DirRequest::Heartbeat { id, players } => {
                if let Some(listing) = state.servers.get_mut(&id) {
                    listing.seen = Instant::now();
                    listing.advert.players = players;
                    DirResponse::Ok
                } else {
                    DirResponse::Error {
                        message: "unknown server".into(),
                    }
                }
            }
            DirRequest::List => DirResponse::Servers {
                servers: state.servers.values().map(|s| s.advert.clone()).collect(),
            },
            DirRequest::Unregister { id } => {
                state.servers.remove(&id);
                DirResponse::Ok
            }
        }
    };
    let mut stream = reader.into_inner();
    write_msg(&mut stream, &response)
}

fn sweep(state: &mut State) {
    let now = Instant::now();
    state.servers.retain(|_, listing| now.duration_since(listing.seen) < STALE);
}

pub fn register(
    directory: &str,
    name: &str,
    host: &str,
    port: u16,
    players: u32,
    max_players: u32,
    map: &str,
) -> std::io::Result<String> {
    let response = exchange(
        directory,
        &DirRequest::Register {
            name: name.into(),
            host: host.into(),
            port,
            players,
            max_players,
            map: map.into(),
        },
    )?;
    match response {
        DirResponse::Registered { id } => Ok(id),
        DirResponse::Error { message } => Err(std::io::Error::other(message)),
        _ => Err(std::io::Error::other("unexpected directory reply")),
    }
}

pub fn heartbeat(directory: &str, id: &str, players: u32) -> std::io::Result<()> {
    let response = exchange(
        directory,
        &DirRequest::Heartbeat {
            id: id.into(),
            players,
        },
    )?;
    match response {
        DirResponse::Ok => Ok(()),
        DirResponse::Error { message } => Err(std::io::Error::other(message)),
        _ => Err(std::io::Error::other("unexpected directory reply")),
    }
}

#[allow(dead_code)]
pub fn unregister(directory: &str, id: &str) -> std::io::Result<()> {
    let _ = exchange(directory, &DirRequest::Unregister { id: id.into() })?;
    Ok(())
}

pub fn list(directory: &str) -> std::io::Result<Vec<ServerAdvert>> {
    match exchange(directory, &DirRequest::List)? {
        DirResponse::Servers { servers } => Ok(servers),
        DirResponse::Error { message } => Err(std::io::Error::other(message)),
        _ => Err(std::io::Error::other("unexpected directory reply")),
    }
}

fn exchange(directory: &str, request: &DirRequest) -> std::io::Result<DirResponse> {
    let mut stream = TcpStream::connect_timeout(&resolve(directory)?, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_nodelay(true)?;
    write_msg(&mut stream, request)?;
    let mut reader = BufReader::new(stream);
    read_msg(&mut reader)
}
