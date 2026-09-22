//! Dedicated match hosting. No client rendering, audio or directory application.
mod gameserver;
pub mod rotation;
pub mod quic;
pub mod websocket;
use peakrunner_protocol as proto;
use peakrunner_discovery::wire;
pub use gameserver::{run_from_args as run_game_server, GameHost, GameHandle};
#[cfg(test)]
use peakrunner_net::{connect, connect_private, browse};
#[cfg(test)]
mod directory {
    pub use peakrunner_directory::lan::serve;
    pub use peakrunner_discovery::lan::*;
}
#[cfg(test)]
use directory::serve as serve_directory;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use peakrunner_core::sim::{Command, Phase};
    use std::time::{Duration, Instant};
    use std::net::TcpStream;
    use std::io::Write;

    fn wait(mut pred: impl FnMut() -> bool) {
        let start = Instant::now();
        while !pred() {
            assert!(start.elapsed() < Duration::from_secs(4), "condition timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    #[test]
    #[ignore = "requires the four reference packs and PEAKRUNNER_PRIVATE_MAPS_DIR"]
    fn private_collection_rotates_connected_clients_and_resets() {
        use peakrunner_core::terrain::MapId;
        let maps = [MapId::Raindance, MapId::Skybreak, MapId::BroadsideClone,
            MapId::StonehengeClone, MapId::SnowblindClone, MapId::DesertOfDeathClone];
        let policy = serde_json::to_string(&maps.iter().map(|m|
            serde_json::json!({"map":m.key(),"mode":"ctf"})).collect::<Vec<_>>()).unwrap();
        let mut host = GameHost::bind("127.0.0.1:0", "Private rotation QA", 8, "raindance")
            .unwrap().with_rotation(&policy).unwrap();
        host.accelerated_rounds = true;
        let port = host.local_addr().port();
        let handle = host.spawn();
        let a = connect_private("127.0.0.1", port, "Alpha", "").unwrap();
        let b = connect_private("127.0.0.1", port, "Beta", "").unwrap();
        wait(|| a.lobby().connected && b.lobby().connected);
        let ids = [a.lobby().player_id, b.lobby().player_id];
        let mut tick = 0;
        let mut seq = 0;
        let mut last_input = Instant::now() - Duration::from_secs(1);
        for map in maps.into_iter().chain([MapId::Raindance]) {
            wait(|| {
                let send = last_input.elapsed() >= Duration::from_millis(20);
                if send { seq += 1; last_input = Instant::now(); }
                for c in [&a,&b] {
                    assert!(c.lobby().error.is_none(), "waiting for {map:?}: {:?}", c.lobby().error);
                    if send { assert!(c.send_input(Command { seq, ..Command::default() }), "input queue full for {map:?}"); }
                }
                [&a,&b].iter().all(|c| c.lobby().snapshot.as_ref().is_some_and(|s|s.map == map))
            });
            for (i,c) in [&a,&b].iter().enumerate() {
                let lobby = c.lobby();
                assert_eq!(lobby.player_id, ids[i]);
                assert!(lobby.error.is_none());
                let state = lobby.snapshot.unwrap();
                assert!(state.tick >= tick);
                assert!(state.players.iter().all(|p|p.pos.is_finite() && p.vel.is_finite()));
            }
            tick = a.lobby().snapshot.unwrap().tick;
        }
        // A late join receives the current authoritative map and stable peers.
        let late = connect_private("127.0.0.1", port, "Late", "").unwrap();
        wait(|| late.lobby().snapshot.is_some());
        assert_eq!(late.lobby().snapshot.unwrap().map, MapId::Raindance);
        a.leave(); b.leave(); late.leave();
        wait(|| handle.players.load(std::sync::atomic::Ordering::Relaxed) == 0);
        wait(|| handle.status.lock().unwrap().map == "Raindance");
    }

    #[test]
    fn server_rotation_selects_initial_map_for_real_clients() {
        let host = GameHost::bind("127.0.0.1:0", "Rotation test", 8, "Valley").unwrap()
            .with_rotation(r#"[{"map":"skybreak-bastions","mode":"ctf"},{"map":"raindance","mode":"ctf"}]"#).unwrap();
        let port = host.local_addr().port();
        let host = host.spawn();
        let a = connect_private("127.0.0.1", port, "Alpha", "").unwrap();
        let b = connect_private("127.0.0.1", port, "Beta", "").unwrap();
        wait(|| a.lobby().snapshot.is_some() && b.lobby().snapshot.is_some());
        for client in [&a, &b] {
            assert_eq!(client.lobby().snapshot.unwrap().map, peakrunner_core::terrain::MapId::Skybreak);
        }
        wait(|| host.status.lock().unwrap().map == "Skybreak Bastions");
        a.leave(); b.leave();
        wait(|| host.players.load(std::sync::atomic::Ordering::Relaxed) == 0);
    }

    #[test]
    fn team_chat_never_reaches_the_opposing_connection() {
        let host = GameHost::bind("127.0.0.1:0", "Team chat test", 8, "Valley").unwrap();
        let port = host.local_addr().port(); let _host = host.spawn();
        let a = connect("127.0.0.1",port,"Alice").unwrap();
        wait(||a.lobby().connected);
        let enemy = connect("127.0.0.1",port,"Enemy").unwrap();
        wait(||enemy.lobby().connected);
        let friend = connect("127.0.0.1",port,"Friend").unwrap();
        wait(||friend.lobby().connected);
        assert!(a.send_team_chat("Secret route".into()));
        wait(||friend.lobby().snapshot.as_ref().is_some_and(|s|s.feed.iter().any(|e|e.line().contains("Secret route"))));
        let tick = friend.lobby().snapshot.unwrap().tick;
        wait(||enemy.lobby().snapshot.as_ref().is_some_and(|s|s.tick >= tick));
        assert!(!enemy.lobby().snapshot.unwrap().feed.iter().any(|e|e.line().contains("Secret route")));
    }

    #[test]
    fn mismatched_map_content_is_rejected_before_a_slot_is_assigned() {
        use std::io::{BufRead,BufReader};
        let host=GameHost::bind("127.0.0.1:0","Content check",8,"Raindance").unwrap();
        let addr=host.local_addr();let handle=host.spawn();
        let mut stream=TcpStream::connect(addr).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let hello=proto::ClientMsg::Hello {name:"Wrong content".into(),password:String::new(),
            protocol:format!("{}:different-content",proto::game_protocol())};
        let mut bytes=serde_json::to_vec(&hello).unwrap();bytes.push(b'\n');stream.write_all(&bytes).unwrap();
        let mut reply=String::new();BufReader::new(stream).read_line(&mut reply).unwrap();
        assert!(matches!(serde_json::from_str::<proto::ServerMsg>(&reply).unwrap(),
            proto::ServerMsg::Reject {message} if message.contains("map pack")));
        assert_eq!(handle.players.load(std::sync::atomic::Ordering::Relaxed),0);
    }
    #[test]
    fn two_clients_share_identity_map_airborne_movement_and_disconnect() {
        let dir = serve_directory("127.0.0.1:0").unwrap();
        let host = GameHost::bind("127.0.0.1:0", "North Spine", 8, "Valley").unwrap();
        let port = host.local_addr().port();
        let _host = host.spawn();
        let lease = directory::register(&dir.addr.to_string(), "North Spine", "127.0.0.1",
            port, 0, 8, "Valley").unwrap();
        assert!(!lease.token.is_empty());
        assert_eq!(browse(&dir.addr.to_string()).unwrap()[0].port, port);
        // Identical display names must not alias identity or hide either player.
        let a = connect("127.0.0.1", port, "Skier").unwrap();
        let b = connect("127.0.0.1", port, "Skier").unwrap();
        wait(|| a.lobby().connected && b.lobby().connected && a.lobby().players.len() == 2);
        let aid = a.lobby().player_id;
        assert!(a.rename("  Pilot 42  ".into()));
        wait(|| b.lobby().players.contains(&"Pilot 42".to_string()));
        assert!(a.rename("<script>".into()));
        assert_eq!(a.lobby().player_id, aid);
        assert_ne!(aid, b.lobby().player_id);
        assert_eq!(a.lobby().map, "Valley");
        assert!(a.send_chat("Ready to play".into()));
        wait(|| b.lobby().snapshot.as_ref().is_some_and(|s| s.feed.iter().any(|e|
            matches!(e, peakrunner_core::feed::Entry::Chat { sender, text } if sender == "Pilot 42" && text == "Ready to play"))));
        let before = a.lobby().snapshot.unwrap().players.into_iter().find(|p| p.net_id == aid).unwrap();
        let started = Instant::now();
        let mut seq = 0;
        while started.elapsed() < Duration::from_millis(3600) {
            seq += 1;
            assert!(a.send_input(Command { seq, jet: true, move_x: 1.0, ..Command::default() }));
            assert!(b.send_input(Command { seq, ..Command::default() }));
            std::thread::sleep(Duration::from_secs_f32(1.0 / 60.0));
        }
        wait(|| {
            let al = a.lobby(); let bl = b.lobby();
            assert!(al.error.is_none(), "{:?}", al.error);
            assert!(bl.error.is_none(), "{:?}", bl.error);
            let (Some(sa), Some(sb)) = (al.snapshot, bl.snapshot) else { return false; };
            if sa.tick != sb.tick { return false; }
            assert_eq!(sa.phase, Phase::Playing);
            assert_eq!(serde_json::to_string(&sa).unwrap(), serde_json::to_string(&sb).unwrap());
            let p = sa.players.iter().find(|p| p.net_id == aid).unwrap();
            assert!(p.pos.y > before.pos.y + 1.0, "airborne Y must be authoritative");
            assert!(p.energy < before.energy);
            assert!(p.pos.x > before.pos.x);
            true
        });
        a.leave();
        wait(|| b.lobby().players.len() == 1);
    }
    #[test]
    fn eight_real_slots_and_password_gate() {
        let host = GameHost::bind("127.0.0.1:0", "Test", 8, "Valley").unwrap().with_password("invite".into());
        let port = host.local_addr().port();
        let _host = host.spawn();
        let wrong = connect_private("127.0.0.1", port, "Wrong", "no").unwrap();
        wait(|| wrong.lobby().error.is_some());
        assert!(!wrong.lobby().connected);
        let mut clients = Vec::new();
        for i in 0..8 { clients.push(connect_private("127.0.0.1", port, &format!("P{i}"), "invite").unwrap()); }
        wait(|| clients.iter().all(|c| c.lobby().players.len() == 8));
        let extra = connect_private("127.0.0.1", port, "Ninth", "invite").unwrap();
        wait(|| extra.lobby().error.is_some());
        assert!(!extra.lobby().connected);
        let snapshot = clients[0].lobby().snapshot.unwrap();
        assert_eq!(snapshot.players.iter().filter(|p| p.team == peakrunner_core::sim::Team::Ember).count(), 4);
    }
    #[test]
    fn delayed_input_burst_is_bounded_and_does_not_replay_stale_ticks() {
        let host = GameHost::bind("127.0.0.1:0", "Burst", 8, "Valley").unwrap();
        let port = host.local_addr().port();
        let _host = host.spawn();
        let client = connect("127.0.0.1", port, "Burst").unwrap();
        wait(|| client.lobby().connected);
        let start = client.lobby().snapshot.unwrap().tick;
        for seq in 1..=16 {
            assert!(client.send_input(peakrunner_core::sim::Command { seq, move_z: 1.0, ..Default::default() }));
        }
        wait(|| client.lobby().snapshot.as_ref().is_some_and(|s| s.acks.iter().any(|ack| *ack == 16)));
        let lobby = client.lobby();
        assert!(lobby.connected && lobby.error.is_none());
        assert!(lobby.snapshot.unwrap().tick - start < 16, "stale burst replayed as a sixteen-tick backlog");
    }
    #[test]
    fn forged_outcomes_oversize_and_flood_do_not_break_server() {
        let host = GameHost::bind("127.0.0.1:0", "Test", 8, "Valley").unwrap();
        let port = host.local_addr().port();
        let _host = host.spawn();
        for payload in [b"{\"op\":\"pose\",\"x\":999999}\n".to_vec(), vec![b'x'; 4096],
            b"{\"op\":\"hello\",\"name\":\"Spam\",\"protocol\":\"peakrunner-2\",\"password\":\"\"}\n".repeat(40)] {
            let mut socket = TcpStream::connect(("127.0.0.1", port)).unwrap();
            let _ = socket.write_all(&payload);
        }
        let good = connect("127.0.0.1", port, "Good").unwrap();
        wait(|| good.lobby().connected);
        assert_eq!(good.lobby().players, vec!["Good"]);
        assert!(GameHost::bind("0.0.0.0:0", "Public", 8, "Valley").is_err());
    }
    #[test]
    fn framing_preserves_partial_messages_and_bounds_lines() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mut sender = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (socket, _) = listener.accept().unwrap();
        let mut reader = wire::Framed::new(socket, 64).unwrap();
        sender.write_all(b"{\"op\":").unwrap();
        std::thread::sleep(Duration::from_millis(10));
        assert!(reader.receive::<proto::ClientMsg>().unwrap().is_empty());
        sender.write_all(b"\"leave\"}\n").unwrap();
        wait(|| !reader.receive::<proto::ClientMsg>().unwrap().is_empty());
        sender.write_all(&[b'x'; 65]).unwrap();
        wait(|| reader.receive::<proto::ClientMsg>().is_err());
    }

    #[test]
    #[ignore = "12-second real-socket load test"]
    fn eight_clients_sustain_movement_and_all_weapons() {
        let host = GameHost::bind("127.0.0.1:0", "Load test", 8, "Raindance").unwrap();
        let port = host.local_addr().port();
        let _host = host.spawn();
        let clients: Vec<_> = (0..8).map(|i| connect("127.0.0.1", port, &format!("P{i}")).unwrap()).collect();
        wait(|| clients.iter().all(|c| c.lobby().connected));
        let start = Instant::now();
        let mut next = start;
        let mut max_bytes = 0;
        for seq in 1..=720 {
            for (i, c) in clients.iter().enumerate() {
                if seq % 90 == 1 { assert!(c.send_chat(format!("Match comms test {i}: {}", "abcdefghij".repeat(12)))); }
                assert!(c.send_input(Command { seq, move_z: 1.0, move_x: (i as f32 * 0.5).sin(),
                    yaw: (seq as f32 * 0.003 + i as f32).rem_euclid(std::f32::consts::TAU),
                    jump: true, jet: seq % 240 < 120, fire: true, weapon: ((seq / 120) % 3) as u8,
                    ..Command::default() }));
                let lobby = c.lobby();
                assert!(lobby.error.is_none(), "client {i}: {:?}", lobby.error);
                assert!(lobby.connected);
                let s = lobby.snapshot.unwrap();
                max_bytes = max_bytes.max(serde_json::to_vec(&s).unwrap().len());
                assert!(s.players.iter().all(|p| p.pos.is_finite() && p.vel.is_finite()));
            }
            next += Duration::from_secs_f64(1.0 / 60.0);
            if next > Instant::now() { std::thread::sleep(next - Instant::now()); }
        }
        let snapshot = clients[0].lobby().snapshot.unwrap();
        assert_eq!(snapshot.players.iter().filter(|p| p.net_id != 0).count(), 8);
        assert!(snapshot.players.iter().all(|p| p.shots > 10));
        assert!(snapshot.tick >= 680, "server failed to sustain close to 60 Hz: {}", snapshot.tick);
        println!("8-client load test: {} ticks, {:.2}s, largest snapshot {} bytes", snapshot.tick, start.elapsed().as_secs_f32(), max_bytes);
    }
}
