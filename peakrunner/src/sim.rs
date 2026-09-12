use glam::{Vec2, Vec3};

use crate::terrain::{
    height, normal, pillars, spawn, Pillar, EMBER_HOME, EYE, GLACIER_HOME, MAP, PLAYER_RADIUS,
};

pub const STEP: f32 = 1.0 / 60.0;
const GRAVITY: f32 = 26.0;
const JET_ACCEL: f32 = 44.0;
const JUMP_SPEED: f32 = 10.8;
const WALK_ACCEL: f32 = 78.0;
const WALK_MAX: f32 = 11.2;
const GROUND_DRAG: f32 = 16.0;
const SKI_DRAG: f32 = 0.18;
const AIR_DRAG: f32 = 0.05;
const AIR_CONTROL: f32 = 16.0;
const SKI_CONTROL: f32 = 10.0;
const ENERGY_MAX: f32 = 100.0;
const ENERGY_JET: f32 = 21.0;
const ENERGY_REGEN: f32 = 14.0;
const DISC_SPEED: f32 = 66.0;
const BOLT_SPEED: f32 = 98.0;
const MATCH_TIME: f32 = 8.0 * 60.0;
const CAPTURES: u32 = 3;
const SENS: f32 = 0.00235;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Team {
    Ember = 0,
    Glacier = 1,
}

impl Team {
    pub fn other(self) -> Team {
        match self {
            Team::Ember => Team::Glacier,
            Team::Glacier => Team::Ember,
        }
    }
    pub fn idx(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MatchState {
    Flyby = 0,
    Playing = 1,
    Paused = 2,
    Ended = 3,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BotRole {
    Offense,
    Defense,
    Hunter,
}

pub struct Input {
    pub move_x: f32,
    pub move_z: f32,
    pub jump: bool,
    pub fire: bool,
    pub weapon: u8,
    pub look_stick_x: f32,
    pub look_stick_y: f32,
    #[allow(dead_code)]
    pub keys_override: Option<Vec<String>>,
    jump_prev: bool,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            move_x: 0.0,
            move_z: 0.0,
            jump: false,
            fire: false,
            weapon: 0,
            look_stick_x: 0.0,
            look_stick_y: 0.0,
            keys_override: None,
            jump_prev: false,
        }
    }
}

pub struct Player {
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub health: f32,
    pub energy: f32,
    pub team: Team,
    pub alive: bool,
    pub respawn: f32,
    pub carrying: Option<Team>,
    pub is_bot: bool,
    pub on_ground: bool,
    pub skiing: bool,
    pub jetting: bool,
    pub cooldown: f32,
    pub weapon: u8,
    bot_role: BotRole,
    bot_goal: Vec3,
    bot_think: f32,
    regen_delay: f32,
    coyote: f32,
}

pub struct Disc {
    pub pos: Vec3,
    pub vel: Vec3,
    pub team: Team,
    pub owner: usize,
    pub life: f32,
    pub kind: u8, // 0 disc, 1 bolt
}

pub struct Explosion {
    pub pos: Vec3,
    pub age: f32,
    pub max_r: f32,
}

pub struct Flag {
    pub team: Team,
    pub pos: Vec3,
    pub home: Vec3,
    pub carrier: Option<usize>,
    pub drop_timer: f32,
}

pub struct World {
    pub players: Vec<Player>,
    pub discs: Vec<Disc>,
    pub explosions: Vec<Explosion>,
    pub flags: [Flag; 2],
    pub pillars: Vec<Pillar>,
    pub score: [u32; 2],
    pub time_left: f32,
    pub state: MatchState,
    pub acc: f32,
    pub flyby: f32,
    pub player_id: usize,
    pub input: Input,
    pub message: String,
    pub message_t: f32,
    pub hitmarker: f32,
    pub damage_flash: f32,
    pub trauma: f32,
    pub kills: u32,
    pub deaths: u32,
    pub events: String,
    pub time: f32,
    rng: u32,
}

struct Rng(u32);
impl Rng {
    fn f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / 16_777_216.0
    }
}

fn look_dir(yaw: f32, pitch: f32) -> Vec3 {
    let cp = pitch.cos();
    Vec3::new(-yaw.sin() * cp, pitch.sin(), -yaw.cos() * cp)
}

fn move_basis(yaw: f32) -> (Vec3, Vec3) {
    let forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    (forward, right)
}

impl World {
    pub fn new() -> Self {
        let pillars = pillars();
        let ember_h = height(EMBER_HOME.x, EMBER_HOME.z);
        let glac_h = height(GLACIER_HOME.x, GLACIER_HOME.z);
        Self {
            players: Vec::new(),
            discs: Vec::new(),
            explosions: Vec::new(),
            flags: [
                Flag {
                    team: Team::Ember,
                    pos: Vec3::new(EMBER_HOME.x, ember_h + 0.2, EMBER_HOME.z),
                    home: Vec3::new(EMBER_HOME.x, ember_h + 0.2, EMBER_HOME.z),
                    carrier: None,
                    drop_timer: 0.0,
                },
                Flag {
                    team: Team::Glacier,
                    pos: Vec3::new(GLACIER_HOME.x, glac_h + 0.2, GLACIER_HOME.z),
                    home: Vec3::new(GLACIER_HOME.x, glac_h + 0.2, GLACIER_HOME.z),
                    carrier: None,
                    drop_timer: 0.0,
                },
            ],
            pillars,
            score: [0, 0],
            time_left: MATCH_TIME,
            state: MatchState::Flyby,
            acc: 0.0,
            flyby: 0.0,
            player_id: 0,
            input: Input::default(),
            message: String::new(),
            message_t: 0.0,
            hitmarker: 0.0,
            damage_flash: 0.0,
            trauma: 0.0,
            kills: 0,
            deaths: 0,
            events: String::new(),
            time: 0.0,
            rng: 0xC0FFEE,
        }
    }

    fn rng(&mut self) -> f32 {
        let mut r = Rng(self.rng);
        let v = r.f32();
        self.rng = r.0;
        v
    }

    pub fn set_paused(&mut self, paused: bool) {
        match (self.state, paused) {
            (MatchState::Playing, true) => self.state = MatchState::Paused,
            (MatchState::Paused, false) => self.state = MatchState::Playing,
            _ => {}
        }
    }

    pub fn start_match(&mut self, ember: bool) {
        let team = if ember { Team::Ember } else { Team::Glacier };
        self.players.clear();
        self.discs.clear();
        self.explosions.clear();
        self.score = [0, 0];
        self.kills = 0;
        self.deaths = 0;
        self.time_left = MATCH_TIME;
        self.state = MatchState::Playing;
        self.acc = 0.0;
        self.trauma = 0.0;
        self.message = "DEPLOYED — TAKE THEIR FLAG".into();
        self.message_t = 3.2;
        self.events = "start".into();

        let yaw = if ember { 0.0 } else { std::f32::consts::PI };
        self.players.push(make_player(team, false, spawn(ember), yaw, BotRole::Offense));
        self.player_id = 0;

        for i in 0..2 {
            let o = Vec3::new((i as f32 - 0.5) * 6.0, 0.0, 4.0);
            let mut p = spawn(ember) + o;
            p.y = height(p.x, p.z) + 1.2;
            let role = if i == 0 { BotRole::Offense } else { BotRole::Defense };
            self.players.push(make_player(team, true, p, yaw, role));
        }
        let other = team.other();
        let other_ember = other == Team::Ember;
        let oyaw = if other_ember { 0.0 } else { std::f32::consts::PI };
        for i in 0..3 {
            let o = Vec3::new((i as f32 - 1.0) * 5.5, 0.0, 3.0);
            let mut p = spawn(other_ember) + o;
            p.y = height(p.x, p.z) + 1.2;
            let role = if i == 2 { BotRole::Defense } else { BotRole::Offense };
            self.players.push(make_player(other, true, p, oyaw, role));
        }

        let eh = height(EMBER_HOME.x, EMBER_HOME.z);
        let gh = height(GLACIER_HOME.x, GLACIER_HOME.z);
        self.flags[0] = Flag {
            team: Team::Ember,
            pos: Vec3::new(EMBER_HOME.x, eh + 0.2, EMBER_HOME.z),
            home: Vec3::new(EMBER_HOME.x, eh + 0.2, EMBER_HOME.z),
            carrier: None,
            drop_timer: 0.0,
        };
        self.flags[1] = Flag {
            team: Team::Glacier,
            pos: Vec3::new(GLACIER_HOME.x, gh + 0.2, GLACIER_HOME.z),
            home: Vec3::new(GLACIER_HOME.x, gh + 0.2, GLACIER_HOME.z),
            carrier: None,
            drop_timer: 0.0,
        };
    }

    pub fn add_look(&mut self, dx: f32, dy: f32) {
        if self.state != MatchState::Playing || self.players.is_empty() {
            return;
        }
        let p = &mut self.players[self.player_id];
        if !p.alive {
            return;
        }
        p.yaw -= dx * SENS;
        p.pitch = (p.pitch - dy * SENS).clamp(-1.52, 1.52);
    }

    #[allow(dead_code)]
    pub fn apply_override(&mut self) {
        let Some(codes) = self.input.keys_override.clone() else {
            return;
        };
        let has = |c: &str| codes.iter().any(|k| k == c);
        let mut mx = 0.0;
        let mut mz = 0.0;
        if has("KeyW") || has("ArrowUp") {
            mz += 1.0;
        }
        if has("KeyS") || has("ArrowDown") {
            mz -= 1.0;
        }
        if has("KeyD") || has("ArrowRight") {
            mx += 1.0;
        }
        if has("KeyA") || has("ArrowLeft") {
            mx -= 1.0;
        }
        self.input.move_x = mx;
        self.input.move_z = mz;
        self.input.jump = has("Space");
        if has("Mouse0") || has("KeyF") {
            self.input.fire = true;
        }
        if has("Digit2") {
            self.input.weapon = 1;
        }
        if has("Digit1") {
            self.input.weapon = 0;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.events.clear();
        self.flyby += dt;
        self.time += dt;
        self.hitmarker = (self.hitmarker - dt * 3.5).max(0.0);
        self.damage_flash = (self.damage_flash - dt * 2.8).max(0.0);
        self.trauma = (self.trauma - dt * 1.6).max(0.0);
        self.message_t = (self.message_t - dt).max(0.0);
        if self.message_t <= 0.0 {
            self.message.clear();
        }

        if self.state == MatchState::Flyby {
            return;
        }
        if self.state != MatchState::Playing {
            return;
        }

        if !self.players.is_empty() {
            let p = &mut self.players[self.player_id];
            p.weapon = self.input.weapon.min(1);
            p.yaw -= self.input.look_stick_x * 1.8 * dt;
            p.pitch = (p.pitch - self.input.look_stick_y * 1.4 * dt).clamp(-1.52, 1.52);
        }

        self.acc += dt.min(0.1);
        let mut n = 0;
        while self.acc >= STEP && n < 5 {
            self.physics_step();
            self.acc -= STEP;
            n += 1;
        }
    }

    fn physics_step(&mut self) {
        let dt = STEP;
        self.time_left -= dt;
        if self.time_left <= 0.0 {
            self.time_left = 0.0;
            self.state = MatchState::Ended;
            self.msg("TIME — MATCH OVER", 4.0);
            self.push_event("end");
            return;
        }

        self.think_bots(dt);
        self.step_players(dt);
        self.step_discs(dt);
        self.step_flags(dt);
        self.step_explosions(dt);
    }

    fn think_bots(&mut self, dt: f32) {
        let n = self.players.len();
        for i in 0..n {
            if !self.players[i].is_bot || !self.players[i].alive {
                continue;
            }
            self.players[i].bot_think -= dt;
            if self.players[i].bot_think > 0.0 {
                continue;
            }
            let wait = 0.18 + self.rng() * 0.22;
            let jx = (self.rng() - 0.5) * 12.0;
            let jz = (self.rng() - 0.5) * 8.0;
            self.players[i].bot_think = wait;
            let team = self.players[i].team;
            let role = self.players[i].bot_role;
            let carrying = self.players[i].carrying;
            let pos = self.players[i].pos;
            let enemy_flag_pos = self.flags[team.other().idx()].pos;
            let enemy_carrier = self.flags[team.other().idx()].carrier;
            let own_home = self.flags[team.idx()].home;
            let own_carrier = self.flags[team.idx()].carrier;

            let mut goal = enemy_flag_pos;
            if carrying.is_some() {
                goal = own_home;
            } else if own_carrier.is_some() && role != BotRole::Offense {
                if let Some(c) = own_carrier {
                    if c < n {
                        goal = self.players[c].pos;
                    }
                }
            } else if role == BotRole::Defense {
                goal = own_home + Vec3::new(jx, 0.0, jz);
            } else if let Some(c) = enemy_carrier {
                if c < n {
                    goal = self.players[c].pos;
                }
            }

            // Hunt nearby enemies.
            if role == BotRole::Hunter || carrying.is_none() {
                let mut best = 80.0;
                for (j, other) in self.players.iter().enumerate() {
                    if j == i || !other.alive || other.team == team {
                        continue;
                    }
                    let d = pos.distance(other.pos);
                    if d < best {
                        best = d;
                        if d < 48.0 && role != BotRole::Defense {
                            goal = other.pos;
                        }
                    }
                }
            }

            self.players[i].bot_goal = goal;

            // Aim at nearest enemy with lead.
            let mut aim_at: Option<Vec3> = None;
            let mut aim_d = 95.0;
            for (j, other) in self.players.iter().enumerate() {
                if j == i || !other.alive || other.team == team {
                    continue;
                }
                let d = pos.distance(other.pos);
                if d < aim_d {
                    aim_d = d;
                    let lead = d / DISC_SPEED;
                    aim_at = Some(other.pos + Vec3::Y * 1.1 + other.vel * lead * 0.85);
                }
            }
            if let Some(at) = aim_at {
                let dir = (at - (pos + Vec3::Y * 1.4)).normalize_or_zero();
                if dir.length_squared() > 0.1 {
                    self.players[i].yaw = (-dir.x).atan2(-dir.z);
                    self.players[i].pitch = dir.y.asin().clamp(-0.7, 0.7);
                }
                self.players[i].weapon = if aim_d < 28.0 { 1 } else { 0 };
            }
        }
    }

    fn step_players(&mut self, dt: f32) {
        let n = self.players.len();
        let jump_edge_player = self.input.jump && !self.input.jump_prev;
        self.input.jump_prev = self.input.jump;

        for i in 0..n {
            if !self.players[i].alive {
                self.players[i].respawn -= dt;
                if self.players[i].respawn <= 0.0 {
                    self.respawn(i);
                }
                continue;
            }

            let is_local = i == self.player_id && !self.players[i].is_bot;
            let (wish_x, wish_z, jump_held, jump_edge, fire) = if is_local {
                (
                    self.input.move_x.clamp(-1.0, 1.0),
                    self.input.move_z.clamp(-1.0, 1.0),
                    self.input.jump,
                    jump_edge_player,
                    self.input.fire,
                )
            } else {
                self.bot_wish(i)
            };

            let land_hit = {
                let p = &mut self.players[i];
                p.cooldown = (p.cooldown - dt).max(0.0);
                let (fwd, right) = move_basis(p.yaw);
                let mut wish = right * wish_x + fwd * wish_z;
                let wlen = wish.length();
                if wlen > 1.0 {
                    wish /= wlen;
                }

                p.vel.y -= GRAVITY * dt;

                let h = height(p.pos.x, p.pos.z);
                let nrm = normal(p.pos.x, p.pos.z);
                let ground_y = h + PLAYER_RADIUS;
                let was_ground = p.on_ground;
                p.on_ground = p.pos.y <= ground_y + 0.12 && p.vel.y <= 6.0;
                if p.on_ground {
                    p.coyote = 0.14;
                } else {
                    p.coyote = (p.coyote - dt).max(0.0);
                }

                if p.pos.y < ground_y {
                    p.pos.y = ground_y;
                    let vn = p.vel.dot(nrm);
                    if vn < 0.0 {
                        p.vel -= nrm * vn;
                    }
                    p.on_ground = true;
                }

                p.skiing = jump_held && (p.on_ground || p.coyote > 0.0);
                p.jetting = false;

                if jump_edge && (p.on_ground || p.coyote > 0.0) {
                    p.vel.y = JUMP_SPEED.max(p.vel.y);
                    p.on_ground = false;
                    p.coyote = 0.0;
                    p.skiing = false;
                }

                if jump_held && !p.on_ground && p.energy > 0.5 {
                    p.jetting = true;
                    p.vel.y += JET_ACCEL * dt;
                    let look_h = Vec3::new(-p.yaw.sin(), 0.0, -p.yaw.cos());
                    p.vel += look_h * 5.5 * dt;
                    p.energy = (p.energy - ENERGY_JET * dt).max(0.0);
                    p.regen_delay = 0.42;
                }

                if p.jetting {
                    // keep
                } else {
                    p.regen_delay = (p.regen_delay - dt).max(0.0);
                    if p.regen_delay <= 0.0 {
                        p.energy = (p.energy + ENERGY_REGEN * dt).min(ENERGY_MAX);
                    }
                }

                let horiz = Vec3::new(p.vel.x, 0.0, p.vel.z);
                let speed = horiz.length();

                if p.on_ground && !p.skiing {
                    // Walk: strong friction, accelerate toward walk cap.
                    if speed > 0.05 {
                        let drag = (GROUND_DRAG * dt).min(1.0);
                        p.vel.x *= 1.0 - drag;
                        p.vel.z *= 1.0 - drag;
                    }
                    p.vel += wish * WALK_ACCEL * dt;
                    let h2 = Vec3::new(p.vel.x, 0.0, p.vel.z);
                    let s2 = h2.length();
                    if s2 > WALK_MAX {
                        let scale = WALK_MAX / s2;
                        p.vel.x *= scale;
                        p.vel.z *= scale;
                    }
                } else if p.on_ground && p.skiing {
                    p.vel.x *= 1.0 - SKI_DRAG * dt;
                    p.vel.z *= 1.0 - SKI_DRAG * dt;
                    p.vel += wish * SKI_CONTROL * dt;
                } else {
                    p.vel.x *= 1.0 - AIR_DRAG * dt;
                    p.vel.z *= 1.0 - AIR_DRAG * dt;
                    p.vel += wish * AIR_CONTROL * dt;
                }

                let vlen = p.vel.length();
                if vlen > 128.0 {
                    p.vel *= 128.0 / vlen;
                }

                p.pos += p.vel * dt;

                // Bounds.
                if p.pos.x < 6.0 {
                    p.pos.x = 6.0;
                    p.vel.x = p.vel.x.abs() * 0.4;
                }
                if p.pos.x > MAP - 6.0 {
                    p.pos.x = MAP - 6.0;
                    p.vel.x = -p.vel.x.abs() * 0.4;
                }
                if p.pos.z < 6.0 {
                    p.pos.z = 6.0;
                    p.vel.z = p.vel.z.abs() * 0.4;
                }
                if p.pos.z > MAP - 6.0 {
                    p.pos.z = MAP - 6.0;
                    p.vel.z = -p.vel.z.abs() * 0.4;
                }

                let mut land_hit = 0.0;
                if !was_ground && p.on_ground && p.vel.y < -18.0 {
                    p.health -= ((-p.vel.y - 18.0) * 1.4).min(22.0);
                    land_hit = 0.25;
                }
                land_hit
            };

            if land_hit > 0.0 && i == self.player_id {
                self.trauma = (self.trauma + land_hit).min(1.0);
            }

            self.collide_pillars(i);

            if fire && self.players[i].cooldown <= 0.0 && self.players[i].alive {
                self.shoot(i);
            }

            if self.players[i].health <= 0.0 && self.players[i].alive {
                self.kill(i, None);
            }
        }
    }

    fn bot_wish(&mut self, i: usize) -> (f32, f32, bool, bool, bool) {
        let p = &self.players[i];
        let to = p.bot_goal - p.pos;
        let yaw = p.yaw;
        let pitch = p.pitch;
        let team = p.team;
        let pos = p.pos;
        let on_ground = p.on_ground;
        let (fwd, right) = move_basis(yaw);
        let horiz = Vec3::new(to.x, 0.0, to.z);
        let hlen = horiz.length();
        let wish = if hlen > 1.0 { horiz / hlen } else { horiz };
        let mx = wish.dot(right).clamp(-1.0, 1.0);
        let mz = wish.dot(fwd).clamp(-1.0, 1.0);
        let mut fire = false;
        for (j, o) in self.players.iter().enumerate() {
            if j == i || !o.alive || o.team == team {
                continue;
            }
            let d = pos.distance(o.pos);
            let dir = look_dir(yaw, pitch);
            let to_e = ((o.pos + Vec3::Y) - (pos + Vec3::Y * 1.4)).normalize_or_zero();
            if d < 78.0 && dir.dot(to_e) > 0.86 {
                fire = true;
            }
        }
        let r = self.rng();
        let jump_edge = on_ground && r < 0.02;
        (mx, mz.max(0.15), true, jump_edge, fire && r < 0.7)
    }

    fn collide_pillars(&mut self, i: usize) {
        let pos = self.players[i].pos;
        let mut vel = self.players[i].vel;
        let mut new_pos = pos;
        for p in &self.pillars {
            let base_y = height(p.x, p.z);
            if pos.y > base_y + p.h + 1.2 {
                continue;
            }
            let dx = new_pos.x - p.x;
            let dz = new_pos.z - p.z;
            let d = dx.hypot(dz);
            let min = p.r + PLAYER_RADIUS;
            if d < min {
                let (nx, nz) = if d > 1e-3 { (dx / d, dz / d) } else { (1.0, 0.0) };
                new_pos.x = p.x + nx * min;
                new_pos.z = p.z + nz * min;
                let vn = vel.x * nx + vel.z * nz;
                if vn < 0.0 {
                    vel.x -= nx * vn * 1.35;
                    vel.z -= nz * vn * 1.35;
                }
            }
        }
        self.players[i].pos = new_pos;
        self.players[i].vel = vel;
    }

    fn shoot(&mut self, i: usize) {
        let p = &self.players[i];
        let dir = look_dir(p.yaw, p.pitch);
        let origin = p.pos + Vec3::Y * 1.35 + dir * 1.5;
        let kind = p.weapon;
        let speed = if kind == 0 { DISC_SPEED } else { BOLT_SPEED };
        let inherit = p.vel * 0.4;
        let mut vel = dir * speed + inherit;
        let is_bot = p.is_bot;
        let team = p.team;
        if is_bot {
            let a = self.rng() - 0.5;
            let b = self.rng() - 0.5;
            let c = self.rng() - 0.5;
            vel += Vec3::new(a, b, c) * 3.2;
        }
        self.discs.push(Disc {
            pos: origin,
            vel,
            team,
            owner: i,
            life: if kind == 0 { 3.4 } else { 1.6 },
            kind,
        });
        {
            let p = &mut self.players[i];
            p.cooldown = if kind == 0 { 1.05 } else { 0.16 };
            if i == self.player_id {
                p.pitch = (p.pitch + 0.018).min(1.5);
            }
        }
        if i == self.player_id {
            self.trauma = (self.trauma + 0.08).min(1.0);
            self.push_event(if kind == 0 { "disc" } else { "bolt" });
        }
    }

    fn step_discs(&mut self, dt: f32) {
        let n_sub = 4;
        let sdt = dt / n_sub as f32;
        let mut explode: Vec<(Vec3, usize, u8, Team)> = Vec::new();
        let mut keep = Vec::new();

        for mut d in self.discs.drain(..) {
            d.life -= dt;
            if d.life <= 0.0 {
                explode.push((d.pos, d.owner, d.kind, d.team));
                continue;
            }
            let mut dead = false;
            for _ in 0..n_sub {
                d.vel.y -= GRAVITY * 0.22 * sdt;
                let next = d.pos + d.vel * sdt;
                let h = height(next.x, next.z);
                if next.y < h + 0.28 || next.x < 1.0 || next.x > MAP - 1.0 || next.z < 1.0 || next.z > MAP - 1.0
                {
                    explode.push((Vec3::new(next.x, h.max(next.y), next.z), d.owner, d.kind, d.team));
                    dead = true;
                    break;
                }
                for (pi, pl) in self.players.iter().enumerate() {
                    if !pl.alive || pl.team == d.team {
                        continue;
                    }
                    let c = pl.pos + Vec3::Y * 0.9;
                    if next.distance(c) < PLAYER_RADIUS + 0.45 {
                        explode.push((next, d.owner, d.kind, d.team));
                        dead = true;
                        let _ = pi;
                        break;
                    }
                }
                if dead {
                    break;
                }
                d.pos = next;
            }
            if !dead {
                keep.push(d);
            }
        }
        self.discs = keep;
        for (pos, owner, kind, team) in explode {
            self.explode(pos, owner, kind, team);
        }
    }

    fn explode(&mut self, pos: Vec3, owner: usize, kind: u8, team: Team) {
        let max_r = if kind == 0 { 8.4 } else { 2.2 };
        self.explosions.push(Explosion {
            pos,
            age: 0.0,
            max_r,
        });
        self.push_event("boom");
        let dmg_core = if kind == 0 { 52.0 } else { 14.0 };
        let mut killed_by_player = false;
        let pid = self.player_id;
        for i in 0..self.players.len() {
            if !self.players[i].alive {
                continue;
            }
            let d = self.players[i].pos.distance(pos);
            if d > max_r {
                continue;
            }
            let fall = (1.0 - d / max_r).clamp(0.0, 1.0);
            let same = self.players[i].team == team;
            if same && i != owner {
                continue;
            }
            let mul = if i == owner { 0.4 } else { 1.0 };
            let dmg = (12.0 + dmg_core * fall * fall) * mul;
            self.players[i].health -= dmg;
            let dir = (self.players[i].pos + Vec3::Y - pos).normalize_or_zero();
            let kick = if kind == 0 { 22.0 } else { 6.0 };
            self.players[i].vel += dir * kick * fall;
            if i == pid {
                self.damage_flash = 0.7;
                self.trauma = (self.trauma + 0.35 * fall).min(1.0);
                self.push_event("pain");
            }
            if owner == pid && i != pid && !same {
                self.hitmarker = 1.0;
                self.push_event("hit");
            }
            if self.players[i].health <= 0.0 {
                if owner == pid && i != pid {
                    killed_by_player = true;
                }
                self.kill(i, Some(owner));
            }
        }
        if killed_by_player {
            self.kills += 1;
            self.msg("FRAG", 1.1);
        }
        if owner == pid {
            self.trauma = (self.trauma + 0.12).min(1.0);
        }
    }

    fn kill(&mut self, i: usize, _killer: Option<usize>) {
        if !self.players[i].alive {
            return;
        }
        self.players[i].alive = false;
        self.players[i].health = 0.0;
        self.players[i].respawn = 3.4;
        self.players[i].jetting = false;
        self.players[i].skiing = false;
        if let Some(team) = self.players[i].carrying.take() {
            self.drop_flag(team, self.players[i].pos);
        }
        if i == self.player_id {
            self.deaths += 1;
            self.msg("YOU WERE FRAGGED", 2.2);
            self.push_event("death");
            self.trauma = 0.8;
        }
        self.explosions.push(Explosion {
            pos: self.players[i].pos + Vec3::Y,
            age: 0.0,
            max_r: 4.5,
        });
    }

    fn respawn(&mut self, i: usize) {
        let ember = self.players[i].team == Team::Ember;
        let mut pos = spawn(ember);
        pos.x += (self.rng() - 0.5) * 8.0;
        pos.z += (self.rng() - 0.5) * 6.0;
        pos.y = height(pos.x, pos.z) + 1.2;
        let p = &mut self.players[i];
        p.pos = pos;
        p.vel = Vec3::ZERO;
        p.health = 100.0;
        p.energy = ENERGY_MAX;
        p.alive = true;
        p.carrying = None;
        p.yaw = if ember { 0.0 } else { std::f32::consts::PI };
        p.pitch = 0.0;
        p.cooldown = 0.4;
        if i == self.player_id {
            self.msg("REDEPLOYED", 1.4);
        }
    }

    fn drop_flag(&mut self, team: Team, pos: Vec3) {
        let f = &mut self.flags[team.idx()];
        f.carrier = None;
        f.pos = Vec3::new(pos.x, height(pos.x, pos.z) + 0.4, pos.z);
        f.drop_timer = 14.0;
        self.msg(
            if team == self.players[self.player_id].team {
                "YOUR FLAG DROPPED"
            } else {
                "ENEMY FLAG DROPPED"
            },
            2.0,
        );
        self.push_event("drop");
    }

    fn step_flags(&mut self, dt: f32) {
        for fi in 0..2 {
            if let Some(c) = self.flags[fi].carrier {
                if c < self.players.len() && self.players[c].alive {
                    self.flags[fi].pos = self.players[c].pos + Vec3::Y * 2.2;
                    continue;
                } else {
                    let pos = self.flags[fi].pos;
                    let team = self.flags[fi].team;
                    self.drop_flag(team, pos);
                }
            }
            if self.flags[fi].carrier.is_none() && self.flags[fi].drop_timer > 0.0 {
                self.flags[fi].drop_timer -= dt;
                let pos = self.flags[fi].pos;
                self.flags[fi].pos.y = height(pos.x, pos.z) + 0.4;
                if self.flags[fi].drop_timer <= 0.0 {
                    self.return_flag(fi);
                }
            }
        }

        // Pickups / captures.
        let n = self.players.len();
        for i in 0..n {
            if !self.players[i].alive {
                continue;
            }
            let team = self.players[i].team;
            let pos = self.players[i].pos;
            let enemy = team.other().idx();
            let own = team.idx();

            // Return own dropped flag.
            if self.flags[own].carrier.is_none() {
                let at_home = self.flags[own].pos.distance(self.flags[own].home) < 2.5;
                if !at_home && pos.distance(self.flags[own].pos) < 2.4 {
                    self.return_flag(own);
                    if i == self.player_id {
                        self.msg("FLAG RETURNED", 2.0);
                    }
                }
            }

            // Grab enemy flag.
            if self.players[i].carrying.is_none() && self.flags[enemy].carrier.is_none() {
                if pos.distance(self.flags[enemy].pos) < 2.6 {
                    self.flags[enemy].carrier = Some(i);
                    self.flags[enemy].drop_timer = 0.0;
                    self.players[i].carrying = Some(self.flags[enemy].team);
                    self.push_event("flag");
                    if i == self.player_id {
                        self.msg("YOU HAVE THE FLAG — GET HOME", 3.0);
                    } else if self.players[i].team != self.players[self.player_id].team {
                        self.msg("YOUR FLAG HAS BEEN TAKEN", 3.0);
                    } else {
                        self.msg("ALLY HAS THEIR FLAG", 2.0);
                    }
                }
            }

            // Capture.
            if self.players[i].carrying.is_some() {
                let own_home = self.flags[own].home;
                let own_at_home = self.flags[own].carrier.is_none()
                    && self.flags[own].pos.distance(own_home) < 3.0;
                if own_at_home && pos.distance(own_home) < 4.2 {
                    self.capture(i);
                }
            }
        }
    }

    fn return_flag(&mut self, fi: usize) {
        let home = self.flags[fi].home;
        self.flags[fi].pos = home;
        self.flags[fi].carrier = None;
        self.flags[fi].drop_timer = 0.0;
        self.push_event("return");
    }

    fn capture(&mut self, i: usize) {
        let team = self.players[i].team;
        if let Some(ft) = self.players[i].carrying.take() {
            self.return_flag(ft.idx());
        }
        self.score[team.idx()] += 1;
        self.push_event("capture");
        self.trauma = 0.55;
        if i == self.player_id {
            self.msg("CAPTURE", 3.2);
        } else if team == self.players[self.player_id].team {
            self.msg("ALLY CAPTURED THE FLAG", 3.0);
        } else {
            self.msg("ENEMY CAPTURED YOUR FLAG", 3.0);
        }
        if self.score[team.idx()] >= CAPTURES {
            self.state = MatchState::Ended;
            self.msg(
                if team == self.players[self.player_id].team {
                    "VICTORY"
                } else {
                    "DEFEAT"
                },
                8.0,
            );
            self.push_event("end");
        }
    }

    fn step_explosions(&mut self, dt: f32) {
        for e in &mut self.explosions {
            e.age += dt;
        }
        self.explosions.retain(|e| e.age < 0.55);
    }

    fn msg(&mut self, s: &str, t: f32) {
        self.message = s.into();
        self.message_t = t;
    }

    fn push_event(&mut self, e: &str) {
        if !self.events.is_empty() {
            self.events.push(',');
        }
        self.events.push_str(e);
    }

    pub fn player_yaw(&self) -> f32 {
        self.players.get(self.player_id).map(|p| p.yaw).unwrap_or(0.0)
    }
    pub fn player_speed(&self) -> f32 {
        self.players
            .get(self.player_id)
            .map(|p| Vec2::new(p.vel.x, p.vel.z).length())
            .unwrap_or(0.0)
    }
    pub fn player_pos(&self) -> Vec3 {
        self.players
            .get(self.player_id)
            .map(|p| p.pos)
            .unwrap_or(Vec3::ZERO)
    }

    pub fn camera(&self) -> (Vec3, Vec3, f32) {
        if self.state == MatchState::Flyby || self.players.is_empty() {
            let t = self.flyby;
            let eye = Vec3::new(
                128.0 + (t * 0.18).sin() * 52.0,
                38.0 + (t * 0.11).cos() * 6.0,
                128.0 + (t * 0.18).cos() * 86.0,
            );
            let target = Vec3::new(128.0, 16.0, 128.0);
            let dir = (target - eye).normalize_or_zero();
            return (eye, dir, 62.0);
        }
        let p = &self.players[self.player_id];
        let shake = self.trauma * self.trauma;
        let t = self.time;
        let off = Vec3::new(
            (t * 37.1).sin() * shake * 0.18,
            (t * 41.7).cos() * shake * 0.14,
            (t * 29.3).sin() * shake * 0.12,
        );
        let spd = Vec2::new(p.vel.x, p.vel.z).length();
        let bob = if p.on_ground && !p.skiing && spd > 2.0 && p.alive {
            (self.time * 9.5).sin() * 0.04
        } else {
            0.0
        };
        let eye = p.pos + Vec3::Y * EYE + off + Vec3::Y * bob;
        let dir = look_dir(p.yaw, p.pitch);
        let fov = 76.0 + (spd * 0.14).min(17.0);
        (eye, dir, fov)
    }

    pub fn hud_json(&self) -> String {
        let p = self.players.get(self.player_id);
        let health = p.map(|x| x.health.max(0.0)).unwrap_or(0.0);
        let energy = p.map(|x| x.energy).unwrap_or(0.0);
        let speed = self.player_speed();
        let yaw = self.player_yaw();
        let pitch = p.map(|x| x.pitch).unwrap_or(0.0);
        let pos = self.player_pos();
        let team = p.map(|x| x.team.idx()).unwrap_or(0);
        let weapon = p.map(|x| x.weapon).unwrap_or(0);
        let carrying = p.and_then(|x| x.carrying).map(|t| t.idx() as i32).unwrap_or(-1);
        let on_g = p.map(|x| if x.on_ground { 1 } else { 0 }).unwrap_or(0);
        let ski = p.map(|x| if x.skiing { 1 } else { 0 }).unwrap_or(0);
        let jet = p.map(|x| if x.jetting { 1 } else { 0 }).unwrap_or(0);
        let alive = p.map(|x| if x.alive { 1 } else { 0 }).unwrap_or(0);
        let cd = p.map(|x| x.cooldown).unwrap_or(0.0);
        let own_flag_home = self.flags.get(team).map(|f| {
            if f.carrier.is_none() && f.pos.distance(f.home) < 3.0 {
                1
            } else if f.carrier.is_some() {
                2
            } else {
                3
            }
        }).unwrap_or(1);
        let winner = if self.state == MatchState::Ended {
            if self.score[0] > self.score[1] {
                0
            } else if self.score[1] > self.score[0] {
                1
            } else {
                -1
            }
        } else {
            -2
        };
        let mut blips = String::new();
        for (i, pl) in self.players.iter().enumerate() {
            if !pl.alive {
                continue;
            }
            if !blips.is_empty() {
                blips.push(';');
            }
            blips.push_str(&format!(
                "{:.0},{:.0},{},{}",
                pl.pos.x,
                pl.pos.z,
                pl.team.idx(),
                if i == self.player_id { 1 } else { 0 }
            ));
        }
        for f in &self.flags {
            blips.push_str(&format!(
                ";{:.0},{:.0},{},2",
                f.pos.x,
                f.pos.z,
                f.team.idx()
            ));
        }
        let msg = self.message.replace('"', "");
        format!(
            "{{\"health\":{:.1},\"energy\":{:.1},\"speed\":{:.1},\"yaw\":{:.4},\"pitch\":{:.4},\"px\":{:.2},\"py\":{:.2},\"pz\":{:.2},\"ember\":{},\"glacier\":{},\"time\":{:.1},\"state\":{},\"weapon\":{},\"flag\":{},\"ownFlag\":{},\"hit\":{:.2},\"flash\":{:.2},\"msg\":\"{}\",\"kills\":{},\"deaths\":{},\"winner\":{},\"team\":{},\"onGround\":{},\"ski\":{},\"jet\":{},\"alive\":{},\"cd\":{:.2},\"events\":\"{}\",\"blips\":\"{}\"}}",
            health,
            energy,
            speed,
            yaw,
            pitch,
            pos.x,
            pos.y,
            pos.z,
            self.score[0],
            self.score[1],
            self.time_left,
            self.state as u8,
            weapon,
            carrying,
            own_flag_home,
            self.hitmarker,
            self.damage_flash,
            msg,
            self.kills,
            self.deaths,
            winner,
            team,
            on_g,
            ski,
            jet,
            alive,
            cd,
            self.events,
            blips
        )
    }
}

#[cfg(test)]
mod controls {
    use super::*;

    fn step(world: &mut World, frames: u32) {
        for _ in 0..frames {
            world.tick(STEP);
        }
    }

    #[test]
    fn a_strafes_left_when_facing_negative_z() {
        let mut world = World::new();
        world.start_match(true);
        let x0 = world.player_pos().x;
        world.input.move_x = -1.0;
        step(&mut world, 180);
        assert!(
            world.player_pos().x < x0 - 0.4,
            "A should move toward -X (left of a -Z facing), x0={x0} x={}",
            world.player_pos().x
        );
    }

    #[test]
    fn d_strafes_right_when_facing_negative_z() {
        let mut world = World::new();
        world.start_match(true);
        let x0 = world.player_pos().x;
        world.input.move_x = 1.0;
        step(&mut world, 180);
        assert!(
            world.player_pos().x > x0 + 0.4,
            "D should move toward +X (right of a -Z facing), x0={x0} x={}",
            world.player_pos().x
        );
    }

    #[test]
    fn w_moves_along_facing() {
        let mut world = World::new();
        world.start_match(true);
        let z0 = world.player_pos().z;
        world.input.move_z = 1.0;
        step(&mut world, 180);
        assert!(
            world.player_pos().z < z0 - 0.4,
            "W should move toward -Z at yaw 0, z0={z0} z={}",
            world.player_pos().z
        );
    }
}

fn make_player(team: Team, bot: bool, pos: Vec3, yaw: f32, role: BotRole) -> Player {
    Player {
        pos,
        vel: Vec3::ZERO,
        yaw,
        pitch: -0.08,
        health: 100.0,
        energy: ENERGY_MAX,
        team,
        alive: true,
        respawn: 0.0,
        carrying: None,
        is_bot: bot,
        on_ground: true,
        skiing: false,
        jetting: false,
        cooldown: 0.0,
        weapon: 0,
        bot_role: role,
        bot_goal: pos,
        bot_think: 0.0,
        regen_delay: 0.0,
        coyote: 0.0,
    }
}
