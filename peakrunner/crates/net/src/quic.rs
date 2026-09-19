//! Client-only encrypted UDP session.
use std::{collections::VecDeque, io, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}, mpsc}, time::{Duration, Instant}};
use peakrunner_core::sim::Command;
use quinn::Connection;
use tokio::time::timeout;
use crate::{client::Lobby, proto::{ClientMsg, ServerMsg}};
use peakrunner_discovery::{wire::invalid, quic::{connect_endpoint, send_control, read_control}};
use peakrunner_protocol::packets::{inputs, Reassembly};
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
pub(crate) async fn run_client(address: &str, hello: ClientMsg, outbound: mpsc::Receiver<Command>,
    lobby: Arc<Mutex<Lobby>>, stop: Arc<AtomicBool>) -> io::Result<()> {
    let (endpoint, conn) = connect_endpoint(address).await?;
    let result = client_connection(&conn, hello, outbound, lobby, stop).await;
    conn.close(0u32.into(), b"client left"); endpoint.wait_idle().await;
    result
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
