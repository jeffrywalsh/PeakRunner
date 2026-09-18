//! Match directory, game server, and the client that joins them.
//!
//! The directory only lists games. A game server advertises itself there and
//! accepts players. The game asks the directory who is hosting, then connects
//! to that server directly.

mod client;
mod directory;
mod gameserver;
mod proto;
mod wire;

pub use client::{browse, connect, Lobby, Session};
pub use directory::serve as serve_directory;
pub use gameserver::{run_from_args as run_game_server, GameHost};
pub use proto::{ServerAdvert, PROTOCOL};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn a_server_is_listed_and_a_client_can_join() {
        let dir = serve_directory("127.0.0.1:0").expect("directory");
        let host = GameHost::bind("127.0.0.1:0", "North Spine", 8, "Valley").expect("host");
        let port = host.local_addr().port();
        host.spawn();
        let id = directory::register(
            &dir.addr.to_string(),
            "North Spine",
            "127.0.0.1",
            port,
            0,
            8,
            "Valley",
        )
        .expect("register");
        assert!(!id.is_empty());

        let listed = browse(&dir.addr.to_string()).expect("browse");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "North Spine");
        assert_eq!(listed[0].port, port);

        let session = connect("127.0.0.1", port, "Ada").expect("connect");
        let ready = wait_until(Duration::from_secs(2), || session.lobby().connected);
        assert!(ready, "welcome did not arrive: {:?}", session.lobby().error);
        let lobby = session.lobby();
        assert_eq!(lobby.match_name, "North Spine");
        assert!(lobby.players.iter().any(|p| p == "Host"), "{:?}", lobby.players);
        assert!(lobby.players.iter().any(|p| p == "Ada"), "{:?}", lobby.players);

        let other = connect("127.0.0.1", port, "Bo").expect("second");
        let both = wait_until(Duration::from_secs(2), || {
            session.lobby().players.iter().any(|p| p == "Bo")
                && other.lobby().players.iter().any(|p| p == "Bo")
        });
        assert!(both, "player list did not update: {:?} {:?}", session.lobby().players, other.lobby().players);

        session.leave();
        let left = wait_until(Duration::from_secs(2), || {
            other.lobby().players.iter().all(|p| p != "Ada")
        });
        assert!(left, "leave did not drop Ada: {:?}", other.lobby().players);
    }

    fn wait_until(limit: Duration, mut pred: impl FnMut() -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < limit {
            if pred() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        pred()
    }
}
