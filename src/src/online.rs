//! Client-only presentation and input prediction. Outcomes always come from snapshots.
use std::collections::VecDeque;
use std::time::Instant;
use crate::sim::{Command, Phase, Player, Snapshot, World, STEP};

pub struct Online {
    pub tick: u64,
    pub round: u32,
    pub phase: Phase,
    pub latency_ms: f32,
    seq: u64,
    acc: f32,
    pending: VecDeque<(Command, Instant)>,
    targets: Vec<Player>,
    balance_notice_until: Option<Instant>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{sim::Match, terrain::MapId};
    #[test]
    fn capture_cues_follow_team_and_do_not_replay_or_sound_on_reset() {
        let mut server=Match::new(MapId::Valley);
        server.join(1,"A").unwrap();server.join(2,"B").unwrap();server.tick=1;
        let mut world=World::new();let mut online=Online::default();
        online.receive(&mut world,&server.snapshot(),1);
        for (score,expected) in [([1,0],"capture_win,"),([1,1],"capture_loss,"),([0,0],"")] {
            world.events.clear();server.tick+=3;server.world.score=score;
            let state=server.snapshot();online.receive(&mut world,&state,1);
            assert_eq!(world.events,expected);
            world.events.clear();online.receive(&mut world,&state,1);
            assert!(world.events.is_empty());
        }
    }

    #[test]
    fn remote_sounds_keep_positions_and_repeated_snapshots_are_silent() {
        let mut server = Match::new(MapId::Valley);
        server.join(1, "A").unwrap(); server.join(2, "B").unwrap();
        server.tick = 1;
        let mut world = World::new(); let mut online = Online::default();
        online.receive(&mut world, &server.snapshot(), 1);
        server.tick += 3;
        server.world.players[1].pos = server.world.players[0].pos + glam::Vec3::X * 40.0;
        server.world.players[1].shots += 1;
        server.world.blast_serial += 1;
        let position = server.world.players[1].pos;
        server.world.explosions.push(crate::sim::Explosion { pos:position, age:0.0, max_r:9.0, kind:0 });
        let state = server.snapshot();
        online.receive(&mut world, &state, 1);
        assert!(world.spatial_sounds.contains(&("boom", position)));
        assert!(world.spatial_sounds.contains(&("disc", position)));
        assert!(!world.events.contains("boom"));
        world.spatial_sounds.clear();
        online.receive(&mut world, &state, 1);
        assert!(world.spatial_sounds.is_empty());
    }

    #[test]
    fn team_transfer_discards_old_prediction_and_announces_redeployment() {
        let mut server = Match::new(MapId::Valley);
        for id in 1..=3 { server.join(id, "Skier").unwrap(); }
        server.step(&[]);
        let mut world = World::new();
        let mut online = Online::default();
        online.receive(&mut world, &server.snapshot(), 3);
        online.pending.push_back((Command { seq: 100, move_z: 1.0, ..Command::default() }, Instant::now()));
        server.leave(1);
        server.step(&[]);
        let snapshot = server.snapshot();
        online.receive(&mut world, &snapshot, 3);
        assert!(online.pending.is_empty());
        assert_eq!(world.players[2].team, snapshot.players[2].team);
        assert_eq!(world.players[2].pos, snapshot.players[2].pos);
        assert!(world.message.starts_with("AUTO-BALANCED"));
        server.step(&[]);
        online.receive(&mut world, &server.snapshot(), 3);
        assert!(world.message.starts_with("AUTO-BALANCED"));
    }

    #[test]
    fn delayed_snapshots_reconcile_pending_inputs_without_replaying_audio() {
        let mut server = Match::new(MapId::Valley);
        server.join(1, "A").unwrap(); server.join(2, "B").unwrap();
        server.phase = Phase::Playing;
        let mut client = World::new();
        let mut online = Online::default();
        server.step(&[]);
        online.receive(&mut client, &server.snapshot(), 1);
        let mut commands = Vec::new();
        let mut snapshots = VecDeque::new();
        for seq in 1..=180 {
            let command = Command { seq, yaw: 0.0, move_x: if seq < 90 { -1.0 } else { 1.0 },
                move_z: 1.0, jet: seq < 100, jump: true, ..Command::default() };
            commands.push(command);
            online.pending.push_back((command, Instant::now()));
            client.predict_command(command);
            server.step(&[Some(command)]);
            if seq % 3 == 0 { snapshots.push_back((seq + 6, server.snapshot())); }
            if snapshots.front().is_some_and(|(at, _)| *at <= seq) {
                let (_, snapshot) = snapshots.pop_front().unwrap();
                client.events.clear();
                online.receive(&mut client, &snapshot, 1);
                assert!(!client.events.contains("disc"));
                assert!(client.players[0].pos.distance(server.world.players[0].pos) < 0.001,
                    "reconciliation changed the predicted route at {seq}");
                assert!(online.pending.len() <= 9);
                assert_eq!(client.players[0].health, snapshot.players[0].health);
            }
        }
        assert!(client.players[0].pos.is_finite());
        let tick = online.tick;
        let mut stale = server.snapshot(); stale.tick = tick - 1;
        stale.players[0].health = 1.0;
        online.receive(&mut client, &stale, 1);
        assert_eq!(online.tick, tick);
        assert_ne!(client.players[0].health, 1.0);
    }
}
impl Default for Online {
    fn default() -> Self { Self { tick: 0, round: 0, phase: Phase::Waiting, latency_ms: 0.0,
        seq: 0, acc: 0.0, pending: VecDeque::new(), targets: Vec::new(), balance_notice_until: None } }
}
impl Online {
    pub fn receive(&mut self, world: &mut World, state: &Snapshot, id: u32) {
        if state.tick <= self.tick { return; }
        let was_online = self.tick > 0;
        let old_blast = world.blast_serial;
        let old = world.players.clone();
        let old_score = world.score;
        let old_flags = world.flags.clone();
        let was = old.get(world.player_id).cloned();
        let input = world.input.clone();
        if !world.apply_snapshot(state, id) { return; }
        let me = world.player_id;
        let new_round = state.round != self.round;
        let team_changed = was.as_ref().is_some_and(|p| p.net_id == id && p.team != world.players[me].team);
        let ack = state.acks[me];
        while self.pending.front().is_some_and(|(c, _)| c.seq <= ack) {
            if let Some((_, sent)) = self.pending.pop_front() { self.latency_ms = sent.elapsed().as_secs_f32() * 1000.0; }
        }
        if new_round || team_changed { self.pending.clear(); }
        self.tick = state.tick; self.round = state.round; self.phase = state.phase;
        self.targets = state.players.clone();
        let current = world.players[me].clone();
        if !new_round && was_online {
            if state.blast_serial > old_blast {
                let count = (state.blast_serial - old_blast).min(state.explosions.len() as u64) as usize;
                for e in state.explosions.iter().rev().take(count) {
                    world.spatial_sounds.push(("boom", e.pos));
                }
            }
            if old_score != state.score {
                for team in 0..2 {
                    // Score decreases on reset are not captures; duplicate/stale
                    // snapshots are already rejected above.
                    for _ in 0..state.score[team].saturating_sub(old_score[team]).min(3) {
                        world.events.push_str(if team == current.team.idx() { "capture_win," } else { "capture_loss," });
                    }
                }
            }
            else if old_flags.iter().zip(&state.flags).any(|(a, b)| a.carrier != b.carrier) {
                world.events.push_str("flag,");
            }
            for (i, p) in state.players.iter().enumerate() {
                if i != me && p.net_id != 0 && p.pos.distance(current.pos) < 120.0 {
                    if old.get(i).is_some_and(|o| o.net_id == p.net_id && p.shots > o.shots) {
                        world.spatial_sounds.push((match p.weapon { 0 => "disc", 1 => "chain", _ => "grenade" }, p.pos));
                    }
                }
            }
        }
        if let Some(prev) = &was {
            if prev.net_id == id && !new_round {
                if current.hits > prev.hits { world.hitmarker = 1.0; world.events.push_str("hit,"); }
                if current.health < prev.health { world.damage_flash = 0.6; world.events.push_str("pain,"); }
                if prev.alive && !current.alive { world.events.push_str("death,"); }
            }
        }
        let events = world.events.clone();
        let trauma = world.trauma;
        if matches!(self.phase, Phase::Playing | Phase::Waiting) {
            for (command, _) in &self.pending { world.predict_command(*command); }
        }
        world.events = events;
        world.trauma = trauma;
        if let Some(prev) = was.filter(|p| p.net_id == id && p.alive == current.alive && !new_round && !team_changed) {
            let correction = prev.pos - world.players[me].pos;
            world.net_camera_offset = if correction.length() < 2.0 {
                (world.net_camera_offset + correction).clamp_length_max(2.0)
            } else { glam::Vec3::ZERO };
            // Mouse look is immediate, including between simulation steps.
            world.players[me].yaw = prev.yaw;
            world.players[me].pitch = prev.pitch;
        } else { world.net_camera_offset = glam::Vec3::ZERO; }
        for (i, p) in world.players.iter_mut().enumerate() {
            if i == me || p.net_id == 0 { continue; }
            if let Some(prev) = old.get(i).filter(|a| a.net_id == p.net_id && a.alive == p.alive
                && a.pos.distance(p.pos) < 30.0 && !new_round) {
                p.pos = prev.pos;
                p.yaw = prev.yaw;
            }
        }
        world.input = input;
        if team_changed { self.balance_notice_until = Some(Instant::now() + std::time::Duration::from_secs(4)); }
        if self.balance_notice_until.is_some_and(|until| until > Instant::now()) {
            world.message = "AUTO-BALANCED — REDEPLOYED TO THE OTHER TEAM".into();
        }
    }

    pub fn advance(&mut self, world: &mut World, dt: f32, active: bool, session: &peakrunner_net::Session) -> bool {
        let dt = dt.clamp(0.0, 0.1);
        world.time += dt;
        world.hitmarker = (world.hitmarker - dt * 3.5).max(0.0);
        world.damage_flash = (world.damage_flash - dt * 2.8).max(0.0);
        world.trauma = (world.trauma - dt * 1.6).max(0.0);
        world.net_camera_offset *= (-dt * 18.0).exp();
        let input = world.input.clone();
        if active {
            if let Some(p) = world.players.get_mut(world.player_id) {
                p.yaw -= input.look_stick_x * 1.8 * dt;
                p.pitch = (p.pitch - input.look_stick_y * 1.4 * dt).clamp(-1.52, 1.52);
            }
        }
        self.acc = (self.acc + dt).min(STEP * 5.0);
        while self.acc >= STEP {
            self.acc -= STEP;
            let Some(p) = world.players.get(world.player_id) else { return false; };
            self.seq += 1;
            let command = Command { seq: self.seq,
                move_x: if active { input.move_x } else { 0.0 }, move_z: if active { input.move_z } else { 0.0 },
                yaw: p.yaw.rem_euclid(std::f32::consts::TAU), pitch: p.pitch,
                jump: active && input.jump, jet: active && input.jet, fire: active && input.fire,
                interact:active && input.interact, weapon: input.weapon };
            if self.pending.len() >= 120 || !session.send_input(command) { return false; }
            self.pending.push_back((command, Instant::now()));
            if matches!(self.phase, Phase::Playing | Phase::Waiting) { world.predict_command(command); }
        }
        world.input = input;
        let blend = 1.0 - (-dt * 20.0).exp();
        for (i, p) in world.players.iter_mut().enumerate() {
            if i == world.player_id || p.net_id == 0 { continue; }
            if let Some(target) = self.targets.get(i).filter(|a| a.net_id == p.net_id) {
                p.pos = p.pos.lerp(target.pos, blend);
                let angle = (target.yaw - p.yaw).sin().atan2((target.yaw - p.yaw).cos());
                p.yaw += angle * blend;
            }
        }
        // Extrapolate only visuals between snapshots, never resolve local hits.
        for d in &mut world.discs { d.pos += d.vel * dt; d.spin += dt * 18.0; }
        for e in &mut world.explosions { e.age += dt; }
        for p in &mut world.smoke { p.age += dt; }
        for f in &mut world.flags {
            if let Some(i) = f.carrier { f.pos = world.players[i].pos + glam::Vec3::Y * 2.2; }
        }
        true
    }
}
