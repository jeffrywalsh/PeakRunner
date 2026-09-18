use serde::{Deserialize, Serialize};

pub const PROTOCOL: &str = "peakrunner-1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerAdvert {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub players: u32,
    pub max_players: u32,
    pub map: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DirRequest {
    Register {
        name: String,
        host: String,
        port: u16,
        players: u32,
        max_players: u32,
        map: String,
    },
    Heartbeat {
        id: String,
        players: u32,
    },
    List,
    Unregister {
        id: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DirResponse {
    Registered {
        id: String,
    },
    Servers {
        servers: Vec<ServerAdvert>,
    },
    Ok,
    Error {
        message: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ClientMsg {
    Hello { name: String, protocol: String },
    Pose {
        x: f32,
        y: f32,
        z: f32,
        yaw: f32,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ServerMsg {
    Welcome {
        player_id: u32,
        match_name: String,
        map: String,
    },
    Players {
        players: Vec<String>,
    },
    Poses {
        poses: Vec<Pose>,
    },
    Reject {
        message: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pose {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
}
