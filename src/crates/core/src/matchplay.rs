//! Authoritative match state and the shared movement predictor.
use super::*;

pub const MAX_PLAYERS: usize = 8;

/// An input, never an outcome. No position, velocity, damage or client delta time.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub seq: u64,
    pub move_x: f32,
    pub move_z: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub jump: bool,
    pub jet: bool,
    pub fire: bool,
    #[serde(default)]
    pub interact: bool,
    #[serde(default)]
    pub kit: bool,
    pub weapon: u8,
}

impl Command {
    pub fn valid(&self) -> bool {
        self.seq > 0 && self.seq < u64::MAX && self.move_x.is_finite()
            && self.move_z.is_finite() && self.yaw.is_finite() && self.pitch.is_finite()
            && self.move_x.abs() <= 1.0 && self.move_z.abs() <= 1.0
            && self.yaw.abs() <= 100_000.0 && self.pitch.abs() <= 1.52 && self.weapon < 3
    }
    fn input(self) -> Input {
        Input { move_x: self.move_x, move_z: self.move_z, jump: self.jump,
            jet: self.jet, fire: self.fire, interact:self.interact, kit:self.kit, weapon: self.weapon, ..Input::default() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase { Waiting, Countdown, Playing, Intermission }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    /// Server transport RTT in milliseconds, keyed by connection-assigned player ID.
    pub pings: Vec<(u32, u32)>,
    pub feed: Vec<crate::feed::Entry>,
    pub equipment: Vec<crate::equipment::State>,
    pub blast_serial: u64,
    pub tick: u64,
    pub round: u32,
    pub phase: Phase,
    pub phase_left: f32,
    pub map: MapId,
    pub players: Vec<Player>,
    pub acks: Vec<u64>,
    pub discs: Vec<Disc>,
    pub explosions: Vec<Explosion>,
    pub smoke: Vec<SmokePuff>,
    pub flags: [Flag; 2],
    pub score: [u32; 2],
    pub time_left: f32,
    #[serde(default)]
    pub mode: crate::map_catalog::SupportedMode,
    #[serde(default)]
    pub points: Vec<crate::control::Point>,
}

pub struct Match {
    rename_next: Vec<u64>,
    chat_next: Vec<u64>,
    pub world: World,
    pub tick: u64,
    pub round: u32,
    pub phase: Phase,
    pub phase_left: f32,
    pub acks: Vec<u64>,
}

impl Match {
    /// Server-owned round boundary: retain connection slots/identities and clocks,
    /// but never carry projectiles, equipment, scores or movement between maps.
    pub fn rotate_to(&mut self, map: MapId) {
        let mode = self.world.mode;
        self.rotate_to_mode(map, mode);
    }

    /// Server rotation boundary with the entry's mode.
    pub fn rotate_to_mode(&mut self, map: MapId, mode: crate::map_catalog::SupportedMode) {
        let mut next = Self::new(map);
        next.world.set_mode(mode);
        next.tick = self.tick;
        next.round = self.round;
        next.phase = self.phase;
        next.phase_left = self.phase_left;
        next.acks.clone_from(&self.acks);
        next.chat_next.clone_from(&self.chat_next);
        next.rename_next.clone_from(&self.rename_next);
        for (slot, old) in self.world.players.iter().enumerate() {
            if old.net_id == 0 { continue; }
            let p = &mut next.world.players[slot];
            p.net_id = old.net_id; p.name = old.name.clone(); p.team = old.team;
            next.world.respawn(slot);
        }
        *self = next;
    }

    pub fn new(map: MapId) -> Self {
        let mut world = World::new();
        world.set_map(map);
        world.start_rift(true);
        world.players.clear();
        for _ in 0..MAX_PLAYERS {
            let mut p = make_player(Team::Ember, false, world.stand(true), 0.0, BotRole::Offense);
            p.alive = false;
            p.remote = true;
            world.players.push(p);
        }
        world.network_inputs = vec![Input::default(); MAX_PLAYERS];
        Self { world, tick: 0, round: 0, phase: Phase::Waiting, phase_left: 0.0,
            acks: vec![0; MAX_PLAYERS], chat_next: vec![0; MAX_PLAYERS], rename_next: vec![0; MAX_PLAYERS] }
    }

    pub fn join(&mut self, id: u32, name: &str) -> Option<usize> {
        let name = crate::names::validate(name)?;
        if id == 0 || self.world.players.iter().any(|p| p.net_id == id) { return None; }
        let slot = self.world.players.iter().position(|p| p.net_id == 0)?;
        let count = |team| self.world.players.iter().filter(|p| p.net_id != 0 && p.team == team).count();
        let team = if count(Team::Ember) <= count(Team::Glacier) { Team::Ember } else { Team::Glacier };
        let mut p = make_player(team, false, self.world.stand(team == Team::Ember),
            if team == Team::Ember { std::f32::consts::PI } else { 0.0 }, BotRole::Offense);
        p.net_id = id;
        p.name = name.into();
        self.world.players[slot] = p;
        self.world.respawn(slot);
        self.acks[slot] = 0;
        self.chat_next[slot] = 0;
        self.rename_next[slot] = 0;
        Some(slot)
    }

    /// Slot comes from the connection, never from a client-supplied player ID.
    pub fn rename(&mut self, slot: usize, raw: &str) -> bool {
        let Some(name) = crate::names::validate(raw) else { return false; };
        let Some(p) = self.world.players.get_mut(slot).filter(|p| p.net_id != 0) else { return false; };
        if self.tick < self.rename_next[slot] { return false; }
        if p.name == name { return true; }
        p.name = name.into();
        self.rename_next[slot] = self.tick.saturating_add(600);
        true
    }

    /// Identity comes only from the authenticated connection's occupied slot.
    pub fn chat(&mut self, slot: usize, text: &str) -> bool {
        self.chat_channel(slot, text, false)
    }

    pub fn chat_channel(&mut self, slot: usize, text: &str, team_only: bool) -> bool {
        let Some(p) = self.world.players.get(slot).filter(|p| p.net_id != 0) else { return false; };
        let Some(text) = crate::feed::message(text) else { return false; };
        if self.tick < self.chat_next[slot] { return false; }
        self.chat_next[slot] = self.tick.saturating_add(60);
        let entry = if team_only { crate::feed::Entry::TeamChat { sender:p.name.clone(), text, team:p.team } }
            else { crate::feed::Entry::Chat { sender:p.name.clone(), text } };
        crate::feed::push(&mut self.world.feed, entry);
        true
    }

    pub fn leave(&mut self, slot: usize) {
        if let Some(team) = self.world.players[slot].carrying.take() {
            self.world.drop_flag(team, self.world.players[slot].pos);
        }
        let p = &mut self.world.players[slot];
        p.net_id = 0;
        p.name.clear();
        p.alive = false;
        p.remote = true;
        self.world.discs.retain(|d| d.owner != slot);
        self.world.network_inputs[slot] = Input::default();
        self.acks[slot] = 0;
        if self.world.players.iter().all(|p| p.net_id == 0) {
            // Fresh session, but keep the transport/healthcheck clock monotonic.
            let tick = self.tick;
            let mode = self.world.mode;
            *self = Self::new(self.world.map);
            self.world.set_mode(mode);
            self.tick = tick;
        }
    }

    fn balance_teams(&mut self) {
        loop {
            let mut counts = [0usize; 2];
            for p in &self.world.players {
                if p.net_id != 0 { counts[p.team.idx()] += 1; }
            }
            if counts[0].abs_diff(counts[1]) <= 1 { break; }
            let larger = if counts[0] > counts[1] { Team::Ember } else { Team::Glacier };
            // Avoid carriers first, then prefer dead players and newest arrivals.
            let slot = self.world.players.iter().enumerate()
                .filter(|(_, p)| p.net_id != 0 && p.team == larger)
                .min_by_key(|(_, p)| (p.carrying.is_some(), p.alive, std::cmp::Reverse(p.net_id)))
                .map(|(i, _)| i).unwrap();
            if let Some(flag) = self.world.players[slot].carrying.take() {
                self.world.drop_flag(flag, self.world.players[slot].pos);
            }
            self.world.discs.retain(|d| d.owner != slot);
            self.world.players[slot].team = if larger == Team::Ember { Team::Glacier } else { Team::Ember };
            self.world.respawn(slot);
            self.world.network_inputs[slot] = Input::default();
        }
    }

    fn restart(&mut self) {
        self.world.equipment=crate::equipment::fresh(self.world.map);
        self.world.discs.clear();
        self.world.explosions.clear();
        self.world.smoke.clear();
        self.world.place_flags();
        self.world.reset_points();
        self.world.score = [0, 0];
        self.world.time_left = MATCH_TIME;
        self.world.state = MatchState::Playing;
        for i in 0..MAX_PLAYERS {
            if self.world.players[i].net_id != 0 {
                self.world.respawn(i);
                let p = &mut self.world.players[i];
                p.frags = 0; p.losses = 0; p.hits = 0; p.shots = 0; p.jump_prev = false;
            }
        }
    }

    /// Exactly one server-owned timestep. Packet volume cannot advance time.
    pub fn step(&mut self, commands: &[Option<Command>]) {
        self.tick += 1;
        self.world.events.clear();
        self.world.spatial_sounds.clear();
        if self.world.players.iter().all(|p| p.net_id == 0) { return; }
        // Once per tick, after the server processes the whole departure batch.
        self.balance_teams();
        self.world.time += STEP;
        let both = [Team::Ember, Team::Glacier].iter().all(|team|
            self.world.players.iter().any(|p| p.net_id != 0 && p.team == *team));
        match self.phase {
            Phase::Playing if !both => {
                // An abandoned team cannot concede a string of uncontested rounds.
                self.restart(); self.phase = Phase::Waiting; self.phase_left = 0.0;
            }
            Phase::Waiting if both => { self.phase = Phase::Countdown; self.phase_left = 3.0; }
            Phase::Countdown if !both => { self.phase = Phase::Waiting; self.phase_left = 0.0; }
            Phase::Countdown | Phase::Intermission => {
                self.phase_left -= STEP;
                if self.phase_left <= 0.0 {
                    self.restart();
                    self.round += 1;
                    self.phase = if both { Phase::Playing } else { Phase::Waiting };
                }
            }
            _ => {}
        }
        for (i, p) in self.world.players.iter_mut().enumerate() {
            let command = commands.get(i).copied().flatten().unwrap_or_default();
            let input = if command.valid() && p.net_id != 0 { command } else { Command::default() };
            if input.seq > 0 {
                self.acks[i] = self.acks[i].max(input.seq);
                p.yaw = input.yaw;
                p.pitch = input.pitch;
                p.weapon = input.weapon;
            }
            self.world.network_inputs[i] = input.input();
            if self.phase == Phase::Countdown || self.phase == Phase::Intermission {
                self.world.network_inputs[i] = Input::default();
            }
        }
        if self.phase != Phase::Intermission {
            self.world.physics_step();
            if self.phase != Phase::Playing {
                // Warmup and countdown never capture or score.
                if self.world.points.iter().any(|p| p.owner.is_some() || p.progress > 0.0) {
                    self.world.reset_points();
                }
                self.world.score = [0, 0];
                self.world.time_left = MATCH_TIME;
                self.world.state = MatchState::Playing;
            } else if self.world.state == MatchState::Ended {
                self.phase = Phase::Intermission;
                self.phase_left = 10.0;
            }
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot { pings:Vec::new(), feed:self.world.feed.iter().filter(|e| !matches!(e,crate::feed::Entry::TeamChat {..})).cloned().collect(), equipment:self.world.equipment.clone(),blast_serial: self.world.blast_serial, tick: self.tick, round: self.round, phase: self.phase,
            phase_left: self.phase_left, map: self.world.map, players: self.world.players.clone(),
            acks: self.acks.clone(), discs: self.world.discs.clone(),
            explosions: self.world.explosions.clone(), smoke: self.world.smoke.clone(),
            flags: self.world.flags.clone(), score: self.world.score, time_left: self.world.time_left,
            mode: self.world.mode, points: self.world.points.clone() }
    }

    pub fn snapshot_for(&self, slot: usize) -> Snapshot {
        let mut state = self.snapshot();
        let team = self.world.players.get(slot).filter(|p|p.net_id != 0).map(|p|p.team);
        state.feed = self.world.feed.iter().filter(|e| match e {
            crate::feed::Entry::TeamChat {team:recipient,..} => Some(*recipient) == team,
            _ => true,
        }).cloned().collect();
        state
    }
}

#[cfg(test)]
mod rotation_tests {
    use super::*;
    #[test]
    fn map_transition_preserves_slots_clocks_and_rate_limits_not_world_state() {
        let mut game = Match::new(MapId::Valley);
        game.join(11, "First").unwrap();
        game.join(22, "Second").unwrap();
        game.join(33, "Third").unwrap();
        game.leave(0); // Noncontiguous occupied slots must stay stable.
        game.tick = 500; game.round = 7;
        game.phase = Phase::Intermission; game.phase_left = STEP;
        game.acks[1] = 123;
        assert!(game.chat(1, "Before rotation"));
        game.world.score = [3, 2];
        game.world.players[1].frags = 9;
        game.world.players[1].vel = Vec3::splat(70.);
        let team = game.world.players[1].team;
        game.rotate_to(MapId::Raindance);
        assert_eq!(game.world.map, MapId::Raindance);
        assert_eq!(game.tick, 500); assert_eq!(game.round, 7);
        assert_eq!(game.world.players[0].net_id, 0);
        assert_eq!(game.world.players[1].net_id, 22);
        assert_eq!(game.world.players[2].net_id, 33);
        assert_eq!(game.world.players[1].name, "Second");
        assert_eq!(game.world.players[1].team, team);
        assert_eq!(game.acks[1], 123);
        assert!(!game.chat(1, "Too soon"));
        assert_eq!(game.world.score, [0, 0]);
        assert_eq!(game.world.players[1].frags, 0);
        assert_eq!(game.world.players[1].vel, Vec3::ZERO);
        assert!(game.world.discs.is_empty()); assert!(game.world.feed.is_empty());
        game.step(&[]);
        assert_eq!(game.round, 8); assert_eq!(game.phase, Phase::Playing);
        game.leave(1); game.leave(2);
        assert_eq!(game.phase, Phase::Waiting); assert_eq!(game.round, 0);
        assert!(game.tick > 500);
    }
}

impl World {
    pub fn apply_snapshot(&mut self, snapshot: &Snapshot, id: u32) -> bool {
        let Some(slot) = snapshot.players.iter().position(|p| p.net_id == id) else { return false; };
        if self.map != snapshot.map { self.set_map(snapshot.map); }
        if snapshot.equipment.len()!=crate::equipment::definitions(snapshot.map).len() {return false;}
        self.equipment=snapshot.equipment.clone();
        self.feed=snapshot.feed.clone();
        self.network_inputs.clear();
        self.blast_serial = snapshot.blast_serial;
        self.player_id = slot;
        self.players = snapshot.players.clone();
        for p in &mut self.players { p.remote = true; }
        self.players[slot].remote = false;
        self.input.jump_prev = self.players[slot].jump_prev;
        self.discs = snapshot.discs.clone();
        self.explosions = snapshot.explosions.clone();
        self.smoke = snapshot.smoke.clone();
        self.flags = snapshot.flags.clone();
        self.mode = snapshot.mode;
        self.points = snapshot.points.clone();
        self.score = snapshot.score;
        self.time_left = snapshot.time_left;
        self.state = if snapshot.phase == Phase::Intermission { MatchState::Ended } else { MatchState::Playing };
        self.kills = self.players[slot].frags;
        self.deaths = self.players[slot].losses;
        self.message = match snapshot.phase {
            Phase::Waiting => "WARMUP — WAITING FOR AN OPPONENT".into(),
            Phase::Countdown => format!("MATCH STARTS IN {:.0}", snapshot.phase_left.ceil()),
            Phase::Intermission => format!("NEXT ROUND IN {:.0}", snapshot.phase_left.ceil()),
            Phase::Playing => String::new(),
        };
        true
    }

    /// Reuse the exact movement integrator, but never predict damage or flag outcomes.
    pub fn predict_command(&mut self, command: Command) {
        if self.state != MatchState::Playing || self.players.is_empty() { return; }
        let slot = self.player_id;
        if !self.players[slot].alive { return; }
        self.input = command.input();
        self.input.jump_prev = self.players[slot].jump_prev;
        self.players[slot].yaw = command.yaw;
        self.players[slot].pitch = command.pitch;
        self.players[slot].weapon = command.weapon;
        let discs = self.discs.len();
        self.predicting = true;
        self.step_players(STEP);
        self.predicting = false;
        self.players[slot].jump_prev = command.jump;
        self.discs.truncate(discs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_chat_is_filtered_before_serialization_and_shares_rate_limit() {
        let mut m = Match::new(MapId::Valley);
        let a = m.join(1,"Alice").unwrap();
        let enemy = m.join(2,"Enemy").unwrap();
        let friend = m.join(3,"Friend").unwrap();
        assert!(m.chat_channel(a,"Secret route",true));
        assert!(!m.chat(a,"Bypass cooldown"));
        assert_eq!(m.snapshot_for(a).feed.len(),1);
        assert_eq!(m.snapshot_for(friend).feed.len(),1);
        assert!(m.snapshot_for(enemy).feed.is_empty());
        assert!(m.snapshot_for(99).feed.is_empty());
        assert!(m.snapshot().feed.is_empty());
        m.tick += 60;
        assert!(!m.chat_channel(a,"\nBad",true));
        assert!(m.chat(a,"Public hello"));
        assert_eq!(m.snapshot_for(enemy).feed.len(),1);
        m.world.players[friend].team = m.world.players[enemy].team;
        assert_eq!(m.snapshot_for(friend).feed.len(),1);
    }

    #[test]
    fn rename_is_validated_rate_limited_and_preserves_identity() {
        let mut m = Match::new(MapId::Valley);
        assert!(m.join(99, "Bad<script>").is_none());
        let a = m.join(1, "Alice").unwrap();
        let b = m.join(2, "Bob").unwrap();
        m.world.players[a].frags = 7;
        assert!(!m.rename(a, "\nAdmin"));
        assert!(!m.rename(99, "Admin"));
        assert!(m.rename(a, "  Pilot 42  "));
        assert_eq!(m.world.players[a].name, "Pilot 42");
        assert_eq!(m.world.players[a].net_id, 1);
        assert_eq!(m.world.players[a].frags, 7);
        assert_eq!(m.world.players[b].name, "Bob");
        assert!(!m.rename(a, "Spam"));
        m.tick += 600;
        assert!(m.rename(a, "New Pilot"));
        m.leave(a);
        assert!(!m.rename(a, "Ghost"));
        let reused = m.join(3, "Newcomer").unwrap();
        assert!(m.rename(reused, "Fresh"));
    }

    #[test]
    fn chat_is_authoritative_bounded_rate_limited_and_resets_when_empty() {
        let mut m = Match::new(MapId::Valley);
        let a = m.join(11, "Alice").unwrap();
        let b = m.join(12, "Bob").unwrap();
        assert!(!m.chat(7, "forged"));
        assert!(!m.chat(a, "bad\nmessage"));
        assert!(m.chat(a, "hello"));
        assert!(!m.chat(a, "spam"));
        assert!(m.chat(b, "ready"));
        m.tick += 60;
        assert!(m.chat(a, "go"));
        let mut client = World::new();
        for _ in 0..3 { assert!(client.apply_snapshot(&m.snapshot(), 11)); }
        assert_eq!(client.feed.len(), 3, "repeated snapshots must not duplicate events");
        assert_eq!(client.feed[0], crate::feed::Entry::Chat { sender: "Alice".into(), text: "hello".into() });
        m.leave(a); m.leave(b);
        assert!(m.world.feed.is_empty());
    }

    #[test]
    fn frags_capture_names_and_weapon_once_even_after_slot_reuse() {
        let mut m = duel();
        m.world.players[0].name = "Alice".into();
        m.world.players[1].name = "Bob".into();
        m.world.kill(1, Some(0), "Disc launcher");
        m.world.kill(1, Some(0), "Disc launcher");
        assert_eq!(m.world.feed.len(), 1);
        m.leave(1); m.join(3, "Charlie").unwrap();
        assert_eq!(m.world.feed[0], crate::feed::Entry::Frag {
            killer: "Alice".into(), victim: "Bob".into(), weapon: "Disc launcher".into() });
        m.world.kill(1, None, "Fall");
        assert!(m.world.feed[1].line().contains("Environment fragged Charlie · Fall"));
    }

    fn duel() -> Match {
        let mut game = Match::new(MapId::Valley);
        game.join(1, "Skier").unwrap();
        game.join(2, "Skier").unwrap();
        game.phase = Phase::Playing;
        game.round = 1;
        game
    }

    #[test]
    fn departure_balances_teams_without_resetting_active_match() {
        let mut game = duel();
        for id in 3..=6 { game.join(id, "Skier").unwrap(); }
        game.world.score = [2, 1];
        game.world.players[4].frags = 7;
        game.leave(1);
        game.leave(3);
        game.step(&[]);
        let count = |team| game.world.players.iter().filter(|p| p.net_id != 0 && p.team == team).count();
        assert_eq!((count(Team::Ember), count(Team::Glacier)), (2, 2));
        assert_eq!(game.world.players[4].team, Team::Glacier);
        assert_eq!(game.world.players[4].frags, 7);
        assert_eq!(game.world.score, [2, 1]);
        assert_eq!(game.phase, Phase::Playing);
    }

    #[test]
    fn balance_prefers_non_carriers_and_drops_flags_if_unavoidable() {
        let mut game = duel();
        game.join(3, "Carrier").unwrap();
        game.world.players[2].carrying = Some(Team::Glacier);
        game.world.flags[1].carrier = Some(2);
        game.leave(1);
        game.balance_teams();
        assert_eq!(game.world.players[0].team, Team::Glacier);
        assert_eq!(game.world.players[2].team, Team::Ember);
        assert_eq!(game.world.flags[1].carrier, Some(2));

        // Exercise the fallback independently of the normal one-flag limit.
        game.world.players[0].team = Team::Ember;
        game.world.players[0].carrying = Some(Team::Ember);
        game.world.flags[0].carrier = Some(0);
        game.balance_teams();
        assert_eq!(game.world.players[2].team, Team::Glacier);
        assert_eq!(game.world.flags[1].carrier, None);
        assert_eq!(game.world.players[2].carrying, None);
    }

    #[test]
    fn empty_server_resets_every_phase_and_keeps_health_clock_advancing() {
        for phase in [Phase::Waiting, Phase::Countdown, Phase::Playing, Phase::Intermission] {
            let mut game = duel();
            game.phase = phase;
            game.phase_left = 9.0;
            game.world.score = [2, 1];
            game.world.time_left = 12.0;
            game.tick = 999;
            game.acks[0] = 300;
            game.leave(0);
            game.leave(1);
            assert_eq!(game.phase, Phase::Waiting);
            assert_eq!(game.phase_left, 0.0);
            assert_eq!(game.round, 0);
            assert_eq!(game.world.score, [0, 0]);
            assert_eq!(game.world.time_left, MATCH_TIME);
            assert!(game.acks.iter().all(|ack| *ack == 0));
            assert!(game.world.discs.is_empty() && game.world.smoke.is_empty() && game.world.explosions.is_empty());
            assert!(game.world.flags.iter().all(|f| f.carrier.is_none() && f.pos == f.home));
            game.step(&[]);
            assert_eq!(game.tick, 1000);
            game.join(7, "Fresh").unwrap();
            game.join(8, "Fresh").unwrap();
            game.step(&[]);
            assert_eq!(game.phase, Phase::Countdown);
        }
    }

    #[test]
    fn server_and_predictor_use_the_same_movement_and_controls() {
        for direction in [-1.0, 1.0] {
            let mut game = duel();
            let mut client = World::new();
            client.apply_snapshot(&game.snapshot(), 1);
            let origin = client.players[0].pos;
            for seq in 1..121 {
                let cmd = Command { seq, move_x: direction, move_z: 1.0, yaw: 0.0,
                    jet: seq < 70, jump: true, ..Command::default() };
                game.step(&[Some(cmd)]);
                client.predict_command(cmd);
                let a = &game.world.players[0]; let b = &client.players[0];
                assert!(a.pos.distance(b.pos) < 0.0001, "movement diverged at {seq}");
                assert!(a.vel.distance(b.vel) < 0.0001);
                assert!((a.energy - b.energy).abs() < 0.0001);
            }
            assert!((client.players[0].pos.x - origin.x) * direction > 1.0);
            assert!(client.players[0].pos.z < origin.z);
        }
    }

    #[test]
    fn all_weapons_damage_on_server_and_respawn_on_server() {
        for weapon in 0..3 {
            let mut game = duel();
            game.world.players[0].pos = Vec3::new(128.0, 150.0, 128.0);
            game.world.players[1].pos = Vec3::new(128.0, 150.0, 110.0);
            for p in &mut game.world.players[..2] { p.on_ground = false; p.health = 8.0; p.cooldown = 0.0; }
            // A direct impact for each projectile type, using the production collision path.
            game.world.discs.push(Disc { pos: Vec3::new(128.0, 150.7, 111.0),
                vel: Vec3::new(0.0, 0.0, -95.0), team: Team::Ember, owner: 0,
                life: if weapon == 2 { 1.5 } else { 1.0 }, kind: weapon, spin: 0.0 });
            game.step(&[]);
            assert!(!game.world.players[1].alive, "weapon {weapon} did not kill");
            assert_eq!(game.world.players[0].frags, 1);
            assert_eq!(game.world.players[1].losses, 1);
            let snap = game.snapshot();
            let mut a = World::new(); let mut b = World::new();
            assert!(a.apply_snapshot(&snap, 1)); assert!(b.apply_snapshot(&snap, 2));
            assert!(!a.players[1].alive && !b.players[1].alive);
            for _ in 0..210 { game.step(&[]); }
            assert!(game.world.players[1].alive);
            assert_eq!(game.world.players[1].health, 100.0);
        }
    }

    #[test]
    fn fire_rate_energy_and_invalid_inputs_are_not_client_outcomes() {
        let mut game = duel();
        game.world.players[0].cooldown = 0.0;
        for seq in 1..=120 {
            game.step(&[Some(Command { seq, fire: true, jet: true, ..Command::default() })]);
        }
        assert!(game.world.players[0].shots <= 2);
        assert!(game.world.players[0].energy < ENERGY_MAX);
        let invalid = [Command { seq: 1, move_x: 2.0, ..Command::default() },
            Command { seq: 1, yaw: f32::NAN, ..Command::default() },
            Command { seq: 1, weapon: 3, ..Command::default() }];
        for c in invalid { assert!(!c.valid()); }
        assert!(serde_json::from_str::<Command>(r#"{"seq":1,"position":[999,999,999]}"#).is_err());
    }

    #[test]
    fn flag_capture_disconnect_and_round_restart_are_shared() {
        let mut game = duel();
        game.world.players[0].pos = game.world.flags[1].pos;
        game.step(&[]);
        assert_eq!(game.world.flags[1].carrier, Some(0));
        game.leave(0);
        assert_eq!(game.world.flags[1].carrier, None);
        assert!(game.world.flags[1].drop_timer > 0.0);
        assert_eq!(game.join(3, "New player"), Some(0));
        for _ in 0..3 {
            game.world.players[0].pos = game.world.flags[1].pos;
            game.step(&[]);
            game.world.players[0].pos = game.world.flags[0].home;
            game.step(&[]);
        }
        assert_eq!(game.world.score, [3, 0]);
        assert_eq!(game.phase, Phase::Intermission);
        let round = game.round;
        for _ in 0..605 { game.step(&[]); }
        assert_eq!(game.phase, Phase::Playing);
        assert_eq!(game.round, round + 1);
        assert_eq!(game.world.score, [0, 0]);
        assert_eq!(game.world.players[0].net_id, 3);
        game.world.time_left = STEP * 0.5;
        game.step(&[]);
        assert_eq!(game.phase, Phase::Intermission);
    }

    #[test]
    fn late_join_snapshot_keeps_airborne_height_flags_and_health() {
        let mut game = duel();
        game.world.players[0].pos.y += 40.0;
        game.world.players[0].health = 37.0;
        game.world.flags[1].carrier = Some(0);
        game.world.players[0].carrying = Some(Team::Glacier);
        let slot = game.join(3, "Late").unwrap();
        assert_eq!(slot, 2);
        let mut client = World::new();
        client.apply_snapshot(&game.snapshot(), 3);
        assert_eq!(client.player_id, 2);
        assert_eq!(client.players[0].pos, game.world.players[0].pos);
        assert_eq!(client.players[0].health, 37.0);
        assert_eq!(client.flags[1].carrier, Some(0));
    }

    #[test]
    fn abandoned_match_returns_to_warmup_and_requires_new_countdown() {
        let mut game = duel();
        game.world.score = [2, 1];
        game.leave(1);
        game.step(&[]);
        assert_eq!(game.phase, Phase::Waiting);
        assert_eq!(game.world.score, [0, 0]);
        game.join(3, "Replacement").unwrap();
        game.step(&[]);
        assert_eq!(game.phase, Phase::Countdown);
    }
}
