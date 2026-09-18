use std::io::BufReader;
use std::net::TcpStream;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::directory;
use crate::proto::{ClientMsg, ServerAdvert, ServerMsg, PROTOCOL};
use crate::wire::{read_msg, resolve, write_msg};

#[derive(Clone, Debug)]
pub struct Lobby {
    pub connected: bool,
    pub match_name: String,
    pub map: String,
    pub players: Vec<String>,
    pub poses: Vec<crate::proto::Pose>,
    pub error: Option<String>,
}

impl Default for Lobby {
    fn default() -> Self {
        Self {
            connected: false,
            match_name: String::new(),
            map: String::new(),
            players: Vec::new(),
            poses: Vec::new(),
            error: None,
        }
    }
}

pub struct Session {
    lobby: Arc<Mutex<Lobby>>,
    outbound: Sender<ClientMsg>,
}

impl Session {
    pub fn lobby(&self) -> Lobby {
        self.lobby.lock().expect("lobby").clone()
    }

    pub fn leave(self) {
        let _ = self.outbound.send(ClientMsg::Pose {
            x: f32::NAN,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
        });
    }

    pub fn send_pose(&self, x: f32, y: f32, z: f32, yaw: f32) {
        let _ = self.outbound.send(ClientMsg::Pose { x, y, z, yaw });
    }
}

pub fn browse(directory_addr: &str) -> std::io::Result<Vec<ServerAdvert>> {
    directory::list(directory_addr)
}

pub fn connect(host: &str, port: u16, name: &str) -> std::io::Result<Session> {
    let stream = TcpStream::connect_timeout(&resolve(&format!("{host}:{port}"))?, Duration::from_secs(3))?;
    stream.set_nodelay(true)?;
    let lobby = Arc::new(Mutex::new(Lobby::default()));
    let (tx, rx) = mpsc::channel();
    let lobby_thread = Arc::clone(&lobby);
    let name = name.to_string();
    thread::Builder::new()
        .name("peakrunner-session".into())
        .spawn(move || session_loop(stream, name, lobby_thread, rx))
        .expect("session thread");
    Ok(Session {
        lobby,
        outbound: tx,
    })
}

fn session_loop(
    mut stream: TcpStream,
    name: String,
    lobby: Arc<Mutex<Lobby>>,
    outbound: mpsc::Receiver<ClientMsg>,
) {
    let hello = ClientMsg::Hello {
        name,
        protocol: PROTOCOL.into(),
    };
    if let Err(err) = write_msg(&mut stream, &hello) {
        fail(&lobby, err.to_string());
        return;
    }
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut writer = stream;
    loop {
        while let Ok(msg) = outbound.try_recv() {
            if matches!(msg, ClientMsg::Pose { x, .. } if x.is_nan()) {
                return;
            }
            if write_msg(&mut writer, &msg).is_err() {
                fail(&lobby, "lost the rift".into());
                return;
            }
        }
        let _ = reader.get_ref().set_read_timeout(Some(Duration::from_millis(200)));
        match read_msg::<ServerMsg>(&mut reader) {
            Ok(ServerMsg::Welcome {
                match_name, map, ..
            }) => {
                let mut lobby = lobby.lock().expect("lobby");
                lobby.connected = true;
                lobby.match_name = match_name;
                lobby.map = map;
                lobby.error = None;
            }
            Ok(ServerMsg::Players { players }) => {
                lobby.lock().expect("lobby").players = players;
            }
            Ok(ServerMsg::Poses { poses }) => {
                lobby.lock().expect("lobby").poses = poses;
            }
            Ok(ServerMsg::Reject { message }) => {
                fail(&lobby, message);
                break;
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock || err.kind() == std::io::ErrorKind::TimedOut => {
                continue;
            }
            Err(err) => {
                fail(&lobby, err.to_string());
                break;
            }
        }
    }
    let mut lobby = lobby.lock().expect("lobby");
    lobby.connected = false;
}

fn fail(lobby: &Mutex<Lobby>, message: String) {
    let mut lobby = lobby.lock().expect("lobby");
    lobby.connected = false;
    lobby.error = Some(message);
}
