use eframe::egui::{self, Align2, Color32, FontId, RichText, Sense, Vec2};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
use eframe::egui_wgpu::Callback;
use serde::Deserialize;

use crate::audio::Audio;
use crate::drawlist::build_frame_with;
use crate::mouse;
use crate::scene::{self, SceneCallback};
use crate::sim::{MatchState, World, ENERGY_MAX};
use crate::terrain::{self, MapId};

const EMBER: Color32 = Color32::from_rgb(226, 74, 50);
const GLACIER: Color32 = Color32::from_rgb(62, 200, 224);
const FG: Color32 = Color32::from_rgb(231, 238, 246);
const MUTED: Color32 = Color32::from_rgb(154, 168, 184);
const BG: Color32 = Color32::from_rgb(8, 13, 20);
const SECONDARY_BUTTON_BG: Color32 = Color32::from_rgb(22, 28, 38);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Menu,
    Play,
    Pause,
    End,
    Browser,
    Lobby,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Hud {
    health: f32,
    energy: f32,
    speed: f32,
    yaw: f32,
    ember: u32,
    glacier: u32,
    time: f32,
    state: u8,
    weapon: u8,
    flag: i32,
    own_flag: i32,
    /// The enemy flag: 1 home, 2 carried, 3 dropped.
    #[serde(default)]
    enemy_flag: i32,
    hit: f32,
    flash: f32,
    msg: String,
    kills: u32,
    deaths: u32,
    winner: i32,
    team: u32,
    ski: u8,
    jet: u8,
    alive: u8,
    events: String,
    blips: String,
    #[serde(default)]
    map_size: f32,
    #[serde(default)]
    armor: u8,
    #[serde(default)]
    ammo: u16,
    #[serde(default)]
    ammo_max: u16,
    #[serde(default)]
    pack: i32,
    #[serde(default)]
    repairing: u8,
    #[serde(default)]
    station: u8,
    #[serde(default)]
    grenades: u8,
    #[serde(default)]
    mines: u8,
    #[serde(default)]
    grenades_max: u8,
    #[serde(default)]
    mines_max: u8,
    /// Objective caption under the score, from the world's mode.
    #[serde(skip)]
    goal: &'static str,
    /// Football: no weapons; the bottom line shows the ball instead.
    #[serde(skip)]
    football: bool,
    /// Current key bindings, for hints.
    #[serde(skip)]
    keys: crate::keybinds::Keybinds,
    /// A grenade or mine being wound up (what, seconds held).
    #[serde(skip)]
    winding: Option<(u8, f32)>,
    /// Placing a pack: why it can't go where you aim ("" when it can).
    #[serde(skip)]
    placing: Option<&'static str>,
    /// The rifle carried: 0 none, 1 laser, 2 railgun.
    #[serde(default)]
    rifle: u8,
    /// Looking through a rifle's zoom.
    #[serde(skip)]
    zoomed: bool,
    /// Deathmatch: the score line ("You 4 · Leader Ace 7"), replacing the
    /// team scores.
    #[serde(skip)]
    ffa_line: Option<String>,
    /// Deathmatch modes: this round's time, weather and twist.
    #[serde(skip)]
    conditions: String,
}

/// Keyboard bindings and the screens that use them.
#[derive(Default)]
struct Controls {
    keys: crate::keybinds::Keybinds,
    /// The inventory screen is open (at your team's inventory station).
    shop_open: bool,
    /// A grenade or mine being wound up: what, and seconds held.
    winding: Option<(u8, f32)>,
    /// Lining up the carried pack (the deployer replaces the weapon), its
    /// turn from your facing, the wheel travel not yet turned, and whether
    /// fire was down last frame (a click places).
    placing: bool,
    turn: f32,
    wheel: f32,
    fire_was: bool,
    /// The controls screen is open, and the action waiting for a key.
    keys_open: bool,
    rebinding: Option<crate::keybinds::Action>,
    /// While placing: why the hologram can't go down ("" when it can).
    ghost_problem: Option<&'static str>,
    /// Stood at the inventory station last frame (it opens on arrival), and
    /// the third-person view.
    was_at_inventory: bool,
    third_person: bool,
    /// Seconds alive since the last spawn.
    alive_for: f32,
}

struct Pad {
    x: f32,
    z: f32,
    lx: f32,
    ly: f32,
    jump: bool,
    fire: bool,
    jet: bool,
}

pub struct PeakRunnerApp {
    overlay: crate::world_overlay::OverlayState,
    effects: crate::effects::Effects,
    announcer: crate::flag_announce::Announcer,
    generator_watch: crate::generator_announce::GeneratorWatch,
    point_watch: crate::control_hud::PointWatch,
    football_watch: crate::football_hud::FootballWatch,
    game_mode: peakrunner_core::map_catalog::SupportedMode,
    chat_team: bool,
    chat_open: bool,
    chat_text: String,
    chat_error: String,
    chat_next: f64,
    world: World,
    audio: Audio,
    mode: Mode,
    ember: bool,
    map: MapId,
    hud: Option<Hud>,
    frame_aspect: f32,
    stick: [f32; 2],
    touch: bool,
    grabbed: bool,
    touch_jump: bool,
    touch_jet: bool,
    touch_fire: bool,
    touch_interact: bool,
    touch_swap: bool,
    look_pending: egui::Vec2,
    /// The click that started the match is still down. Don't treat it as fire.
    wait_fire_release: bool,
    /// Key bindings, the inventory screen and the grenade/mine wind-up.
    ctl: Controls,
    #[cfg(not(target_arch = "wasm32"))]
    pads: Option<gilrs::Gilrs>,
    #[cfg(not(target_arch = "wasm32"))]
    net: NetUi,
}

impl PeakRunnerApp {
    #[cfg(all(test, not(target_arch = "wasm32")))]
    pub(crate) fn comms_fixture() -> Self {
        let mut world = World::new(); world.start_rift(true);
        world.feed = vec![
            peakrunner_core::feed::Entry::Chat { sender: "Ridge".into(), text: "Two incoming at the west entrance.".into() },
            peakrunner_core::feed::Entry::Frag { killer: "Ridge".into(), victim: "Echo".into(), weapon: "Disc launcher".into() },
            peakrunner_core::feed::Entry::Frag { killer: "Nova".into(), victim: "Ridge".into(), weapon: "Grenade launcher".into() },
            peakrunner_core::feed::Entry::Chat { sender: "Echo".into(), text: "On my way. Cover the flag!".into() },
        ];
        Self { overlay: Default::default(), effects: Default::default(), announcer: Default::default(), generator_watch: Default::default(), point_watch: Default::default(), football_watch: Default::default(), game_mode: Default::default(), chat_team:false, world, audio: Audio::silent(), mode: Mode::Play, ember: true, map: MapId::Valley,
            hud: None, frame_aspect: 1.6, stick: [0.;2], touch: false, grabbed: false,
            touch_jump: false, touch_jet: false, touch_fire: false, touch_interact: false,
            touch_swap: false, look_pending: Vec2::ZERO, wait_fire_release: false, ctl: Controls::default(),
            pads: None, net: NetUi::new(), chat_open: true, chat_text: "Nice shot!".into(), chat_error: String::new(), chat_next: 0.0 }
    }

    #[cfg(all(test, not(target_arch = "wasm32")))]
    pub(crate) fn capture_comms(&mut self, ctx: &egui::Context) { style_ui(ctx); self.chat_ui(ctx); }

    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        scene::install(cc)?;
        style_ui(&cc.egui_ctx);
        #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
        let mut app = Self {
            overlay: Default::default(),
            effects: Default::default(),
            announcer: Default::default(),
            generator_watch: Default::default(),
            point_watch: Default::default(),
            football_watch: Default::default(),
            game_mode: Default::default(),
            chat_team: false,
            chat_open: false, chat_text: String::new(), chat_error: String::new(), chat_next: 0.0,
            world: {
                let mut world = World::new();
                world.set_map(MapId::Raindance);
                world
            },
            audio: Audio::new(),
            mode: Mode::Menu,
            ember: true,
            map: MapId::Raindance,
            hud: None,
            frame_aspect: 16.0 / 9.0,
            stick: [0.0, 0.0],
            touch: false,
            grabbed: false,
            touch_jump: false,
            touch_jet: false,
            touch_fire: false,
            touch_interact: false,
            touch_swap: false,
            look_pending: egui::Vec2::ZERO,
            wait_fire_release: false,
            ctl: Controls::default(),
            #[cfg(not(target_arch = "wasm32"))]
            pads: gilrs::Gilrs::new().ok(),
            #[cfg(not(target_arch = "wasm32"))]
            net: NetUi::load(),
        };
        #[cfg(not(target_arch = "wasm32"))]
        app.world.set_bot_difficulty(app.net.preferences.saved.bot_difficulty);
        #[cfg(not(target_arch = "wasm32"))]
        { app.ctl.keys = app.net.preferences.saved.keys.clone(); }
        #[cfg(not(target_arch = "wasm32"))]
        if let Ok(address) = std::env::var("PEAKRUNNER_JOIN") {
            if let Ok(name) = std::env::var("PEAKRUNNER_NAME") { app.net.name = name; }
            app.net.password = std::env::var("PEAKRUNNER_MATCH_PASSWORD").unwrap_or_default();
            if address.starts_with("quic://") {
                app.join_server(&address, 7777);
            } else if let Some((host, port)) = address.rsplit_once(':').and_then(|(h, p)| p.parse::<u16>().ok().map(|p| (h, p))) {
                app.join_server(host, port);
            }
        }
        Ok(app)
    }

    fn step(&mut self, ctx: &egui::Context, dt: f32) {
        if ctx.input(|i| i.viewport().close_requested()) {
            return;
        }
        let rect = ctx.content_rect();
        if rect.height() > 1.0 {
            self.frame_aspect = rect.width() / rect.height();
        }
        if ctx.input(|i| i.any_touches()) {
            self.touch = true;
        }
        self.world.events.clear();
        self.world.spatial_sounds.clear();
        self.poll_net(ctx);

        let was_chat = self.chat_open;
        let keys = self.ctl.keys.clone();
        use crate::keybinds::Action;
        if self.mode != Mode::Play { self.chat_open = false; }
        if self.mode == Mode::Play && !self.chat_open && !self.ctl.shop_open {
            let public = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, keys.get(Action::ChatPublic)));
            let team = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, keys.get(Action::ChatTeam)));
            if public || team {
                self.chat_open = true;
                self.chat_team = team;
                self.chat_error.clear();
                // Do not insert the opening hotkey's text event into the message.
                ctx.input_mut(|i| i.events.retain(|e| !matches!(e,egui::Event::Text(_))));
            }
        }
        if self.chat_open && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.chat_open = false;
            self.wait_fire_release = true;
        }
        // The inventory screen closes on Escape or the use key, when you step
        // off the station, or die.
        let at_inventory = self.hud.as_ref().is_some_and(|h| h.station == 1 && h.alive == 1)
            || std::env::var_os("QA_SHOP").is_some();
        if self.ctl.shop_open {
            let close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                || i.consume_key(egui::Modifiers::NONE, keys.get(Action::Use)));
            if close || self.mode != Mode::Play || !at_inventory { self.close_shop(); }
        }
        let gameplay_input = self.mode == Mode::Play && !self.chat_open && !was_chat && !self.ctl.shop_open
            && ctx.input(|i| i.focused);
        // While a key is being rebound, Escape cancels that instead.
        let escape = self.ctl.rebinding.is_none() && ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if escape && self.mode == Mode::Play {
            self.pause(ctx);
        } else if escape && self.mode == Mode::Pause {
            self.resume(ctx);
        }

        if self.look_pending != egui::Vec2::ZERO {
            if gameplay_input { self.world.add_look(self.look_pending.x, self.look_pending.y); }
            self.look_pending = egui::Vec2::ZERO;
        }
        let mouse = mouse::sample(ctx, gameplay_input);
        if gameplay_input {
            self.grab(ctx, true);
            self.world.add_look(mouse.dx, mouse.dy);
        } else {
            self.grab(ctx, false);
        }
        if self.touch_swap && gameplay_input {
            // Cycle the weapons, including the rifle once one is bought.
            let slots = if self.world.players.get(self.world.player_id).is_some_and(|p| p.rifle) { 4 } else { 3 };
            self.world.input.weapon = (self.world.input.weapon + 1) % slots;
            self.touch_swap = false;
        }

        let pad = self.poll_pad();
        let mut mx = self.stick[0] + pad.x;
        let mut mz = self.stick[1] + pad.z;
        ctx.input(|i| {
            if !gameplay_input { return; }
            if keys.down(i, Action::Left) || i.key_down(egui::Key::ArrowLeft) { mx -= 1.0; }
            if keys.down(i, Action::Right) || i.key_down(egui::Key::ArrowRight) { mx += 1.0; }
            if keys.down(i, Action::Forward) || i.key_down(egui::Key::ArrowUp) { mz += 1.0; }
            if keys.down(i, Action::Back) || i.key_down(egui::Key::ArrowDown) { mz -= 1.0; }
            for (action, weapon) in [(Action::Disc, 0), (Action::Chaingun, 1), (Action::Launcher, 2), (Action::Rifle, 3)] {
                if keys.pressed(i, action) { self.world.input.weapon = weapon; }
            }
        });
        let jump = (gameplay_input && ctx.input(|i| keys.down(i, Action::Jump))) || pad.jump || self.touch_jump;
        let jet = mouse.jet || pad.jet || self.touch_jet;
        let mut mouse_fire = self.mode == Mode::Play && mouse.fire;
        if self.wait_fire_release {
            if mouse_fire {
                mouse_fire = false;
            } else {
                self.wait_fire_release = false;
            }
        }
        let fire = mouse_fire || pad.fire || self.touch_fire || (gameplay_input && std::env::var_os("QA_FIRE").is_some());
        let play = self.mode == Mode::Play;
        self.world.input.move_x = mx.clamp(-1.0, 1.0);
        self.world.input.move_z = mz.clamp(-1.0, 1.0);
        self.world.input.jump = jump;
        self.world.input.jet = jet && play;
        self.world.input.fire = fire;
        let use_pressed = gameplay_input && ctx.input(|i| keys.pressed(i, Action::Use));
        // Stepping onto your inventory station opens its screen, as in
        // Tribes; after closing it, the use key opens it again.
        // Not on a respawn beside one: only after a second alive.
        let alive = self.hud.as_ref().is_some_and(|h| h.alive == 1);
        self.ctl.alive_for = if alive { self.ctl.alive_for+dt } else { 0.0 };
        if at_inventory && !self.ctl.was_at_inventory && self.ctl.alive_for > 1.0 && gameplay_input && !self.world.ball.active {
            self.ctl.shop_open = true;
        }
        self.ctl.was_at_inventory = at_inventory || self.ctl.alive_for <= 1.0;
        // Away from a station, holding the use key zooms a rifle (Tribes' E).
        let at_station = self.hud.as_ref().is_some_and(|h| h.station != 0);
        self.world.zoomed = gameplay_input && !at_station && !self.ctl.placing && ctx.input(|i| keys.down(i, Action::Use));
        // The view key: third person, unless it's turning a pack being placed.
        if gameplay_input && !self.ctl.placing && ctx.input(|i| keys.pressed(i, Action::View)) {
            self.ctl.third_person = !self.ctl.third_person;
        }
        self.world.third_person = self.ctl.third_person;
        self.world.input.interact = play && (ctx.input(|i| keys.down(i, Action::Use)) || self.touch_interact);
        // Use at your inventory station opens the inventory screen.
        if use_pressed && at_inventory && !self.world.ball.active { self.ctl.shop_open = true; }
        self.world.input.repair = play && ctx.input(|i| keys.down(i, Action::Repair));
        // One-shot intents latch until a tick sends them (World::tick and the
        // online predictor clear them), so a press is never lost.
        // Deploying: the deploy key brings up the deployer and a hologram of
        // the pack where you aim (see `World::aim_placement`); click places
        // it, the wheel or the rotate key turns it, and the deploy key, a
        // weapon key or the repair tool puts the deployer away.
        let pack = self.world.players.get(self.world.player_id).filter(|p| p.alive).and_then(|p| p.pack);
        if gameplay_input && ctx.input(|i| keys.pressed(i, Action::Deploy)) && pack.is_some() && !self.world.ball.active {
            self.ctl.placing = !self.ctl.placing;
            self.ctl.turn = 0.0;
        }
        if self.ctl.placing {
            let leave = pack.is_none() || !play || self.world.input.repair
                || ctx.input(|i| [Action::Disc, Action::Chaingun, Action::Launcher, Action::Rifle].iter().any(|&a| keys.pressed(i, a)));
            if leave {
                self.ctl.placing = false;
                // A click still held from placing isn't a shot.
                self.wait_fire_release = true;
            } else if gameplay_input {
                let step = std::f32::consts::PI / 12.0;
                if ctx.input(|i| keys.pressed(i, Action::View)) { self.ctl.turn += step; }
                self.ctl.wheel += ctx.input(|i| i.smooth_scroll_delta.y);
                while self.ctl.wheel.abs() >= 30.0 {
                    let dir = self.ctl.wheel.signum();
                    self.ctl.turn += step * dir;
                    self.ctl.wheel -= 30.0 * dir;
                }
                self.ctl.turn = (self.ctl.turn + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                if fire && !self.ctl.fire_was {
                    self.world.input.deploy = true;
                    self.world.input.deploy_turn = self.ctl.turn;
                }
            }
            // The deployer doesn't shoot.
            self.world.input.fire = false;
        }
        self.ctl.fire_was = fire;
        // Ctrl+K: suicide, as in Tribes. One press, one death: holding the
        // keys doesn't kill you again when you respawn.
        self.world.input.suicide |= play && ctx.input(|i| i.modifiers.ctrl && keys.pressed(i, Action::Suicide));
        // Grenades and mines: hold to wind up, release to throw (base Tribes'
        // throw strength). The wind-up shows on the HUD.
        use peakrunner_core::sim::throwables::{THROW_GRENADE, THROW_MINE, WIND_UP};
        let held = if !gameplay_input { None } else if ctx.input(|i| keys.down(i, Action::Grenade)) { Some(THROW_GRENADE) }
            else if ctx.input(|i| keys.down(i, Action::Mine)) { Some(THROW_MINE) } else { None };
        match (self.ctl.winding, held) {
            (Some((what, t)), Some(now)) if what == now => self.ctl.winding = Some((what, t + dt)),
            (Some((what, t)), _) => {
                // Released (or switched): throw what was wound up.
                if gameplay_input {
                    self.world.input.throw = what;
                    self.world.input.throw_strength = (t / WIND_UP).clamp(0.0, 1.0);
                }
                self.ctl.winding = held.map(|now| (now, 0.0));
            }
            (None, now) => self.ctl.winding = now.map(|now| (now, 0.0)),
        }
        self.world.input.look_stick_x = pad.lx;
        self.world.input.look_stick_y = pad.ly;
        if !gameplay_input {
            // Keep the weapon, and a purchase made on the inventory screen;
            // standing at the station keeps it servicing you while you shop.
            let old = std::mem::take(&mut self.world.input);
            self.world.input.weapon = old.weapon;
            self.world.input.buy = old.buy;
            self.world.input.deploy = old.deploy;
            self.world.input.deploy_turn = old.deploy_turn;
            self.world.input.interact = self.ctl.shop_open;
            self.touch_swap = false;
            self.stick = [0.0; 2];
        }

        if self.online() {
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(session) = &self.net.session {
                let active = gameplay_input;
                if !self.net.predictor.advance(&mut self.world, dt, active, session) {
                    self.net.lobby.error = Some("Connection stalled; leave and rejoin the match.".into());
                    self.net.session.take();
                    self.net.dropped_in = false;
                    self.mode = Mode::Lobby;
                    self.world.state = MatchState::Paused;
                    self.grab(ctx, false);
                }
            }
        } else {
            self.world.tick(dt.min(0.1).max(0.0));
        }
        // The deploy hologram follows your aim every frame.
        self.world.placing = self.ctl.placing && self.mode == Mode::Play;
        self.world.deploy_ghost = None;
        self.ctl.ghost_problem = None;
        if self.world.placing {
            let me = self.world.player_id;
            if let Some(kind) = self.world.players.get(me).and_then(|p| p.pack) {
                match self.world.aim_placement(me, kind, self.ctl.turn) {
                    Some(g) => {
                        self.ctl.ghost_problem = Some(g.problem.unwrap_or(""));
                        self.world.deploy_ghost = Some((g.unit, g.problem.is_none()));
                    }
                    None => self.ctl.ghost_problem = Some("TOO FAR AWAY"),
                }
            }
        }
        let raw = self.world.hud_json();
        self.audio.frame(&self.world, dt, self.mode == Mode::Play);
        let mut sounds = std::mem::take(&mut self.world.spatial_sounds);
        for (name, position) in sounds.drain(..) {
            self.audio.play_world(name, position, &self.world);
        }
        self.world.spatial_sounds = sounds;
        if let Ok(mut hud) = serde_json::from_str::<Hud>(&raw) {
            hud.goal = match self.world.mode {
                peakrunner_core::map_catalog::SupportedMode::CaptureAndHold => "Hold the points · first to 300",
                peakrunner_core::map_catalog::SupportedMode::Football => crate::football_hud::GOAL_LINE,
                peakrunner_core::map_catalog::SupportedMode::Ctf => "First to 3 captures",
                peakrunner_core::map_catalog::SupportedMode::Deathmatch => "Everyone's a target · first to 20 frags",
                peakrunner_core::map_catalog::SupportedMode::TeamDeathmatch => "Frag their team · first to 40",
            };
            hud.football = self.world.ball.active;
            hud.keys = self.ctl.keys.clone();
            hud.winding = self.ctl.winding;
            hud.placing = self.ctl.ghost_problem;
            hud.zoomed = self.world.zoom_active();
            if self.world.deathmatch() { hud.conditions = self.world.conditions.describe(); }
            if self.world.ffa() {
                let mine = self.world.players.get(self.world.player_id).map_or(0, |p| p.frags);
                hud.ffa_line = Some(match self.world.ffa_leader() {
                    Some(i) if i != self.world.player_id => format!("You {mine} · {} {}", self.world.display_name(i), self.world.players[i].frags),
                    _ => format!("You {mine} · leading"),
                });
            }
            if !hud.events.is_empty() {
                for event in hud.events.split(',') {
                    self.audio.play(event);
                }
            }
            self.audio.set_map_ambience(self.world.map,self.world.player_pos(),self.mode==Mode::Play);
            if hud.state == 3 && self.mode == Mode::Play {
                self.mode = Mode::End;
                self.grab(ctx, false);
            }
            self.hud = Some(hud);
        }
    }

    fn start(&mut self, ctx: &egui::Context) {
        self.audio.unlock();
        self.world.set_map(self.map);
        crate::qa_overrides::stage_points(&mut self.world);
        use peakrunner_core::map_catalog::SupportedMode;
        let mode = match self.game_mode {
            _ if crate::qa_overrides::football_staged() => SupportedMode::Football,
            SupportedMode::CaptureAndHold if self.world.control_point_count() >= 2 => SupportedMode::CaptureAndHold,
            SupportedMode::Football if peakrunner_core::sim::football::has_field(self.map) => SupportedMode::Football,
            // Both deathmatch modes run anywhere.
            m if m.deathmatch() => m,
            // A stadium otherwise hosts only Football.
            _ if peakrunner_core::sim::football::has_field(self.map) => SupportedMode::Football,
            _ => SupportedMode::Ctf,
        };
        self.world.set_mode(mode);
        // Fresh randomness per match (deathmatch conditions, spawns).
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.subsec_nanos());
        self.world.reseed(seed);
        self.world.start_match(self.ember);
        self.audio.play("start");
        self.mode = Mode::Play;
        self.wait_fire_release = true;
        self.grab(ctx, true);
    }

    fn pause(&mut self, ctx: &egui::Context) {
        if !self.online() { self.world.set_paused(true); }
        self.mode = Mode::Pause;
        self.grab(ctx, false);
    }

    fn resume(&mut self, ctx: &egui::Context) {
        if !self.online() { self.world.set_paused(false); }
        self.mode = Mode::Play;
        self.grab(ctx, true);
    }

    fn menu(&mut self, ctx: &egui::Context) {
        #[cfg(not(target_arch = "wasm32"))]
        self.net.disconnect();
        self.world.state = MatchState::Flyby;
        self.mode = Mode::Menu;
        self.grab(ctx, false);
        self.audio.set_jet(false);
    }

    fn grab(&mut self, ctx: &egui::Context, lock: bool) {
        self.grabbed = lock && self.mode == Mode::Play && !self.chat_open;
        ctx.send_viewport_cmd(egui::ViewportCommand::CursorGrab(if self.grabbed {
            egui::CursorGrab::Locked
        } else {
            egui::CursorGrab::None
        }));
        ctx.send_viewport_cmd(egui::ViewportCommand::CursorVisible(!self.grabbed));
    }

    fn poll_pad(&mut self) -> Pad {
        #[cfg(target_arch = "wasm32")]
        {
            return poll_web_pad();
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
        let mut pad = Pad {
            x: 0.0,
            z: 0.0,
            lx: 0.0,
            ly: 0.0,
            jump: false,
            fire: false,
            jet: false,
        };
        if let Some(pads) = &mut self.pads {
            while pads.next_event().is_some() {}
            if let Some((_, gp)) = pads.gamepads().find(|(_, g)| g.is_connected()) {
                let (x, y) = deadzone(gp.value(gilrs::Axis::LeftStickX), gp.value(gilrs::Axis::LeftStickY));
                let (lx, ly) = deadzone(gp.value(gilrs::Axis::RightStickX), gp.value(gilrs::Axis::RightStickY));
                pad.x = x;
                pad.z = y;
                pad.lx = lx;
                pad.ly = -ly;
                pad.jump = gp.is_pressed(gilrs::Button::South) || gp.is_pressed(gilrs::Button::LeftTrigger2);
                pad.jet = gp.is_pressed(gilrs::Button::LeftTrigger);
                pad.fire = gp.is_pressed(gilrs::Button::RightTrigger2) || gp.is_pressed(gilrs::Button::RightTrigger);
            }
        }
        pad
        }
    }
}

fn deadzone(x: f32, y: f32) -> (f32, f32) {
    let m = x.hypot(y);
    if m < 0.15 {
        return (0.0, 0.0);
    }
    let s = (m - 0.15) / 0.85 / m;
    (x * s, y * s)
}

#[cfg(target_arch = "wasm32")]
fn poll_web_pad() -> Pad {
    let mut pad = Pad {
        x: 0.0,
        z: 0.0,
        lx: 0.0,
        ly: 0.0,
        jump: false,
        fire: false,
        jet: false,
    };
    let Some(window) = web_sys::window() else {
        return pad;
    };
    let Ok(list) = window.navigator().get_gamepads() else {
        return pad;
    };
    for i in 0..list.length() {
        let value = list.get(i);
        if value.is_null() || value.is_undefined() {
            continue;
        }
        let Ok(gp) = value.dyn_into::<web_sys::Gamepad>() else {
            continue;
        };
        let axes = gp.axes();
        let lx = axes.get(0).as_f64().unwrap_or(0.0) as f32;
        let ly = axes.get(1).as_f64().unwrap_or(0.0) as f32;
        let rx = axes.get(2).as_f64().unwrap_or(0.0) as f32;
        let ry = axes.get(3).as_f64().unwrap_or(0.0) as f32;
        let (x, y) = deadzone(lx, ly);
        let (look_x, look_y) = deadzone(rx, ry);
        pad.x = x;
        pad.z = -y;
        pad.lx = look_x;
        pad.ly = look_y;
        let buttons = gp.buttons();
        let pressed = |idx: u32| {
            buttons
                .get(idx)
                .dyn_into::<web_sys::GamepadButton>()
                .map(|b| b.pressed())
                .unwrap_or(false)
        };
        pad.jump = pressed(0) || pressed(6);
        pad.jet = pressed(4);
        pad.fire = pressed(7) || pressed(5);
        break;
    }
    pad
}

impl eframe::App for PeakRunnerApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(not(target_arch = "wasm32"))]
        if self.net.settings_changed.is_some_and(|t|t.elapsed().as_millis()>=600) {
            self.net.save_settings();
        }
        let dt = ctx.input(|i| i.stable_dt);
        self.step(ctx, if dt > 0.0 { dt } else { 1.0 / 60.0 });
        crate::qa_overrides::apply(&mut self.world);
        crate::qa_overrides::apply_points(&mut self.world);
        crate::qa_overrides::apply_loadout(&mut self.world);
        if std::env::var_os("QA_SHOP").is_some() && self.mode == Mode::Play { self.ctl.shop_open = true; }
        if std::env::var_os("QA_CONTROLS").is_some() && self.mode == Mode::Menu { self.ctl.keys_open = true; }
        if let Some((kind, turn)) = crate::qa_overrides::placing(&mut self.world) {
            if self.mode == Mode::Play {
                if let Some(p) = self.world.players.get_mut(self.world.player_id) { p.pack = Some(kind); }
                self.ctl.placing = true;
                self.ctl.turn = turn;
            }
        }
        ctx.request_repaint();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let rect = ui.max_rect();
        let pixels = ui.ctx().pixels_per_point();
        let frame = build_frame_with(&self.world, self.frame_aspect, ui.input(|i| i.stable_dt).max(1.0 / 120.0), &mut self.effects);
        ui.painter().add(Callback::new_paint_callback(
            rect,
            SceneCallback {
                pixels: [
                    (rect.width() * pixels).round().max(1.0) as u32,
                    (rect.height() * pixels).round().max(1.0) as u32,
                ],
                frame,
            },
        ));

        if let Some(hud) = self.hud.clone() {
            if self.mode == Mode::Play && hud.flash > 0.0 {
                ui.painter().rect_filled(
                    rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(226, 74, 50, (hud.flash * 90.0) as u8),
                );
            }
        }
        // Eye below a water surface: a murky tint over the view.
        if self.mode == Mode::Play {
            // A QA fly-camera below the surface tints like a player's eye would.
            let eye = crate::drawlist::qa_flycam_eye().unwrap_or(self.world.camera().0);
            if let Some(c) = peakrunner_core::water::eye_under(self.world.map, &self.world.staged_water, eye) {
                let [r, g, b] = c.map(|v| (v * 255.0) as u8);
                ui.painter().rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(r, g, b, 120));
            }
        }

        match self.mode {
            Mode::Menu => self.menu_ui(ui),
            Mode::Play => {
                let dt = ui.ctx().input(|i| i.stable_dt).min(0.1);
                crate::world_overlay::draw(ui, &self.world, &mut self.overlay, dt);
                crate::flag_hud::draw(ui, &self.world);
                crate::control_hud::draw(ui, &self.world);
                crate::football_hud::draw(ui, &self.world);
                for text in self.generator_watch.update(&self.world) { self.announcer.push(text); }
                for text in self.point_watch.update(&self.world) { self.announcer.push(text); }
                for text in self.football_watch.update(&self.world) { self.announcer.push(text); }
                self.announcer.update(&self.world, dt);
                self.announcer.draw(ui);
                reference_measurements(ui,&self.world);
                #[cfg(not(target_arch = "wasm32"))]
                if self.net.dropped_in {
                    rift_roster(ui, &self.net.lobby);
                }
                if let Some(hud) = self.hud.clone() {
                    let touch = self.touch;
                    let (jump, fire, jet, swap, look) = play_hud(ui, &hud, touch, &mut self.stick);
                    self.touch_jump = jump;
                    self.touch_jet = jet;
                    self.touch_fire = fire;
                    if swap {
                        self.touch_swap = true;
                    }
                    self.look_pending += look;
                }
                self.touch_interact=false;
                if self.touch && self.world.equipment_prompt().is_some() {
                    let center=ui.max_rect().center_bottom()+Vec2::new(0.0,-180.0);
                    self.touch_interact=ui.put(egui::Rect::from_center_size(center,Vec2::new(112.0,48.0)),
                        egui::Button::new("Use / repair").sense(egui::Sense::click_and_drag())).is_pointer_button_down_on();
                }
                self.chat_ui(ui.ctx());
                if self.ctl.shop_open { self.shop_ui(ui.ctx()); }
            }
            Mode::Pause => self.pause_ui(ui),
            Mode::End => self.end_ui(ui),
            Mode::Browser => self.browser_ui(ui),
            Mode::Lobby => self.lobby_ui(ui),
        }
        if self.ctl.keys_open && self.mode != Mode::Play { self.controls_ui(ui.ctx()); }
        #[cfg(not(target_arch = "wasm32"))]
        if self.online() && !self.chat_open && (ui.input(|i| i.key_down(egui::Key::Tab)) || self.mode == Mode::End) {
            self.scoreboard_ui(ui);
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.03, 0.05, 0.08, 1.0]
    }
}

impl PeakRunnerApp {
    fn chat_ui(&mut self, ctx: &egui::Context) {
        if !self.chat_open { ctx.memory_mut(|m|m.surrender_focus(egui::Id::new("chat_input"))); }
        let width = (ctx.content_rect().width() - 32.0).min(440.0).max(160.0);
        let bottom = if self.touch { 224.0 } else { 112.0 };
        let mut submit = false;
        egui::Area::new(egui::Id::new("match_comms"))
            .fade_in(false)
            .anchor(Align2::LEFT_BOTTOM, [16.0, -bottom])
            .order(egui::Order::Foreground).show(ctx, |ui| {
                egui::Frame::new().fill(Color32::from_rgba_unmultiplied(BG.r(),BG.g(),BG.b(),230)).corner_radius(6).inner_margin(10).show(ui, |ui| {
                    ui.set_width(width - 20.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("MATCH COMMS").size(11.0).color(MUTED));
                        if !self.chat_open { ui.label(RichText::new(format!("Public · {}   Team · {}", self.ctl.keys.name(crate::keybinds::Action::ChatPublic),
                            self.ctl.keys.name(crate::keybinds::Action::ChatTeam))).size(11.0).color(MUTED)); }
                        if !self.chat_open && self.touch && ui.add_sized([80.0,44.0],egui::Button::new("Chat")).clicked() {
                            self.chat_open = true;
                            self.chat_team = false;
                            self.chat_error.clear();
                        }
                    });
                    egui::ScrollArea::vertical().id_salt("match_feed").max_height(if self.chat_open {160.0} else {92.0})
                        .stick_to_bottom(true).show(ui, |ui| {
                            let skip = if self.chat_open { 0 } else { self.world.feed.len().saturating_sub(4) };
                            for entry in self.world.feed.iter().skip(skip) {
                                use peakrunner_core::feed::Entry;
                                let (prefix, color) = match entry {
                                    Entry::Frag {..} => ("FRAG  ", FG),
                                    Entry::Play {..} => ("PLAY  ", Color32::from_rgb(255, 214, 120)),
                                    _ => ("CHAT  ", GLACIER),
                                };
                                ui.add(egui::Label::new(RichText::new(format!("{prefix}{}", entry.line()))
                                    .color(color).size(12.0)).wrap());
                            }
                        });
                    if self.chat_open {
                        ui.separator();
                        let response = ui.add(egui::TextEdit::singleline(&mut self.chat_text)
                            .id(egui::Id::new("chat_input")).hint_text(if self.chat_team {"Message your team…"} else {"Message everyone…"}).char_limit(160).desired_width(f32::INFINITY));
                        response.request_focus();
                        submit = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                        ui.horizontal(|ui| {
                            let h=if self.touch {44.0} else {26.0};
                            submit |= ui.add_sized([56.0,h],egui::Button::new("Send")).clicked();
                            if ui.add_sized([100.0,h],egui::Button::new("Cancel · Esc")).clicked() { self.chat_open = false; self.wait_fire_release=true; }
                            ui.label(RichText::new(if self.chat_team {"Team only"} else {"All players"}).size(11.0).color(MUTED));
                        });
                        if !self.chat_error.is_empty() { ui.colored_label(EMBER, &self.chat_error); }
                    }
                });
            });
        if submit {
            let now = ctx.input(|i| i.time);
            if let Some(text) = peakrunner_core::feed::message(&self.chat_text) {
                if now < self.chat_next { self.chat_error = "Wait a moment before sending again.".into(); return; }
                let mut sent = false;
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(session) = &self.net.session { sent = if self.chat_team { session.send_team_chat(text.clone()) } else {session.send_chat(text.clone())}; }
                if !self.online() {
                    let sender = self.world.display_name(self.world.player_id);
                    let entry = if self.chat_team { peakrunner_core::feed::Entry::TeamChat { sender,text,team:self.world.players[self.world.player_id].team } }
                        else { peakrunner_core::feed::Entry::Chat { sender,text } };
                    peakrunner_core::feed::push(&mut self.world.feed, entry);
                    sent = true;
                }
                if sent {
                    self.chat_next = now + 1.1;
                    self.chat_text.clear(); self.chat_error.clear(); self.chat_open = false;
                    self.wait_fire_release = true;
                } else { self.chat_error = "Message not queued. Try again.".into(); }
            } else { self.chat_error = "Use 1–160 characters (240 UTF-8 bytes), without control characters.".into(); }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn scoreboard_ui(&self, ui: &egui::Ui) {
        let Some(snapshot) = &self.net.lobby.snapshot else { return; };
        egui::Area::new(egui::Id::new("online-scoreboard"))
            .anchor(Align2::RIGHT_TOP, Vec2::new(-18.0, 60.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::new().fill(Color32::from_rgba_unmultiplied(12, 18, 28, 242))
                    .corner_radius(8.0).inner_margin(16.0).show(ui, |ui| {
                        ui.set_width(360.0);
                        ui.label(RichText::new("MATCH ROSTER").color(GLACIER).size(13.0));
                        // The client world mirrors the latest snapshot (and QA staging).
                        let w = &self.world;
                        if w.mode == peakrunner_core::map_catalog::SupportedMode::CaptureAndHold {
                            ui.label(RichText::new(format!("CAPTURE & HOLD · Ember {} – Glacier {} · first to {}",
                                w.score[0], w.score[1], peakrunner_core::control::CNH_TARGET)).color(FG));
                            ui.horizontal_wrapped(|ui| {
                                for p in w.points.iter().filter(|p| p.active) {
                                    let owner = p.owner.map_or("neutral", |o| if o == 0 { "Ember" } else { "Glacier" });
                                    ui.label(RichText::new(format!("{} · {owner}", p.name)).color(crate::control_hud::team_color(p.owner)));
                                }
                            });
                        }
                        if w.deathmatch() {
                            let text = if w.ffa() { format!("DEATHMATCH · first to {} frags", peakrunner_core::sim::deathmatch::FFA_FRAG_LIMIT) }
                                else { format!("TEAM DEATHMATCH · Ember {} – Glacier {} · first to {}", w.score[0], w.score[1], peakrunner_core::sim::deathmatch::TDM_FRAG_LIMIT) };
                            ui.label(RichText::new(text).color(FG));
                            ui.label(RichText::new(w.conditions.describe()).color(MUTED));
                        }
                        ui.label(RichText::new(format!("{} · input ack {:.0} ms", self.net.lobby.map,
                            self.net.predictor.latency_ms)).color(MUTED));
                        egui::Grid::new("match-scores").striped(true).show(ui, |ui| {
                            ui.label("Player"); ui.label("Team");
                            ui.label(if w.ball.active { "Points / D" } else { "K / D" }); ui.label("Ping"); ui.end_row();
                            for p in snapshot.players.iter().filter(|p| p.net_id != 0) {
                                let team = if p.team == crate::sim::Team::Ember { "Ember" } else { "Glacier" };
                                let color = if p.team == crate::sim::Team::Ember { EMBER } else { GLACIER };
                                let you = if p.net_id == self.net.lobby.player_id { " · you" } else { "" };
                                ui.label(RichText::new(format!("{} #{}{you}", p.name, p.net_id)).color(FG));
                                ui.label(RichText::new(team).color(color));
                                ui.label(format!("{} / {}", p.frags, p.losses));
                                ui.label(snapshot.pings.iter().find(|(id,_)| *id == p.net_id)
                                    .map_or_else(|| "—".into(), |(_,ms)| format!("{ms} ms")));
                                ui.end_row();
                            }
                        });
                        ui.label(RichText::new("Ping = server RTT · — unavailable on local TCP").size(11.0).color(MUTED));
                    });
            });
    }

    fn menu_ui(&mut self, ui: &mut egui::Ui) {
        use peakrunner_core::map_catalog::{self, SupportedMode};
        // Maps the chosen mode can use (QA staging can widen Football and
        // Capture & Hold to any map).
        let maps_for = |mode: SupportedMode| -> Vec<terrain::MapInfo> {
            terrain::maps().into_iter().filter(|m| match mode {
                SupportedMode::Football if crate::qa_overrides::football_staged() => true,
                SupportedMode::CaptureAndHold if crate::qa_overrides::staged_point_count().is_some_and(|n| n >= 2) => true,
                _ => map_catalog::supports(m.id, mode),
            }).collect()
        };
        let card = |ui: &mut egui::Ui, on: bool, title: &str, detail: &str, width: f32| -> bool {
            let fill = if on { Color32::from_rgb(34, 58, 78) } else { Color32::from_rgb(20, 26, 36) };
            let stroke = if on { egui::Stroke::new(2.0, GLACIER) } else { egui::Stroke::new(1.0, Color32::from_rgb(44, 54, 68)) };
            let response = egui::Frame::new().fill(fill).stroke(stroke).corner_radius(8.0).inner_margin(egui::Margin::symmetric(12, 9))
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        ui.set_min_height(58.0);
                        ui.label(RichText::new(title).color(if on { FG } else { Color32::from_rgb(210, 216, 226) }).size(15.0).strong());
                        ui.add(egui::Label::new(RichText::new(detail).color(MUTED).size(11.5)).wrap());
                    });
                }).response.interact(egui::Sense::click());
            response.clicked()
        };
        egui::Area::new(egui::Id::new("menu"))
            .anchor(Align2::LEFT_CENTER, Vec2::new(36.0, 0.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::new().fill(Color32::from_rgba_unmultiplied(10, 14, 22, 226)).corner_radius(14.0)
                    .inner_margin(24.0).stroke(egui::Stroke::new(1.0, Color32::from_rgb(40, 52, 68))).show(ui, |ui| {
                ui.set_width(600.0);
                ui.label(RichText::new("PEAKRUNNER").color(FG).size(52.0).strong());
                ui.label(RichText::new("Ski the ridgelines, jet the gaps, take their flag.").color(MUTED));
                ui.add_space(14.0);

                ui.label(RichText::new("1 · GAME MODE").color(GLACIER).size(13.0));
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    for (mode, detail) in [(SupportedMode::Ctf, "Grab their flag, bring it home. Stations, turrets, deployables."),
                        (SupportedMode::CaptureAndHold, "Hold capture towers to score. First to 300."),
                        (SupportedMode::Football, "No weapons. Pass, tackle, carry it into their end zone."),
                        (SupportedMode::TeamDeathmatch, "Team frags score. Random time of day, weather and a twist.")] {
                        let usable = !maps_for(mode).is_empty();
                        if card(ui, self.game_mode == mode, mode.label(), if usable { detail } else { "No maps for this yet." }, 170.0) && usable {
                            self.game_mode = mode;
                        }
                    }
                });
                // Keep the map valid for the mode.
                let maps = maps_for(self.game_mode);
                if !maps.iter().any(|m| m.id == self.map) {
                    if let Some(first) = maps.first() { self.map = first.id; self.world.set_map(first.id); }
                }
                ui.add_space(12.0);

                ui.label(RichText::new("2 · MAP").color(GLACIER).size(13.0));
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    for spec in &maps {
                        let size = if spec.size >= 1000.0 { format!("{:.1} km", spec.size / 1000.0) } else { format!("{:.0} m", spec.size) };
                        if card(ui, self.map == spec.id, &format!("{} · {size}", spec.name), spec.note, 170.0) {
                            self.map = spec.id;
                            self.world.set_map(spec.id);
                        }
                    }
                });
                ui.add_space(12.0);

                ui.label(RichText::new("3 · TEAM AND BOTS").color(GLACIER).size(13.0));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    team_button(ui, "Ember", self.ember, EMBER, || self.ember = true);
                    team_button(ui, "Glacier", !self.ember, GLACIER, || self.ember = false);
                    ui.add_space(12.0);
                    use peakrunner_core::bot_nav::Difficulty;
                    let current = self.world.bot_difficulty;
                    for difficulty in Difficulty::ALL {
                        let on = current == difficulty;
                        let fill = if on { FG } else { Color32::from_rgb(22, 28, 38) };
                        let text = if on { BG } else { FG };
                        let button = egui::Button::new(RichText::new(difficulty.label()).color(text)).fill(fill).min_size(Vec2::new(72.0, 36.0));
                        if ui.add(button).clicked() {
                            self.world.set_bot_difficulty(difficulty);
                            #[cfg(not(target_arch = "wasm32"))]
                            self.net.preferences.set_bot_difficulty(difficulty);
                        }
                    }
                });
                let about = match self.world.bot_difficulty {
                    peakrunner_core::bot_nav::Difficulty::Easy => "Bots: mostly rookies, slow to react, wide of the mark.",
                    peakrunner_core::bot_nav::Difficulty::Normal => "Bots: a spread of grunts, riders, skirmishers, anchors and hawks.",
                    peakrunner_core::bot_nav::Difficulty::Hard => "Bots: quick, accurate skiers, jetters, high-flying hawks and aces.",
                    peakrunner_core::bot_nav::Difficulty::Mixed => "Bots: any personality, from rookie to ace.",
                };
                ui.label(RichText::new(about).color(MUTED).size(12.0));
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(RichText::new("Start match").size(20.0).color(BG)).fill(FG).min_size(Vec2::new(200.0, 46.0))).clicked() {
                        self.start(ui.ctx());
                    }
                    if ui.add(secondary_button("Find match").min_size(Vec2::new(160.0, 46.0))).clicked() {
                        self.open_browser();
                    }
                });
                ui.add_space(14.0);
                ui.horizontal_wrapped(|ui| {
                    use crate::keybinds::Action;
                    let k = &self.ctl.keys;
                    hint(ui, "Move", &[Action::Forward, Action::Left, Action::Back, Action::Right].map(|a| k.name(a)).concat());
                    hint(ui, "Ski / jump", k.name(Action::Jump));
                    hint(ui, "Jet", "Right click");
                    hint(ui, "Fire", &format!("Click · {}/{}/{}", k.name(Action::Disc), k.name(Action::Chaingun), k.name(Action::Launcher)));
                    hint(ui, "Repair", &format!("Hold {}", k.name(Action::Repair)));
                    hint(ui, "Inventory", &format!("{} at a station", k.name(Action::Use)));
                    hint(ui, "Grenade / mine", &format!("{} / {}", k.name(Action::Grenade), k.name(Action::Mine)));
                    hint(ui, "Deploy", k.name(Action::Deploy));
                    hint(ui, "Respawn", &format!("Ctrl+{}", k.name(Action::Suicide)));
                    if ui.add(secondary_button("Controls…")).clicked() { self.ctl.keys_open = true; }
                });
                    });
            });
    }

    fn close_shop(&mut self) {
        if self.ctl.shop_open {
            self.ctl.shop_open = false;
            // The click on a button isn't a shot.
            self.wait_fire_release = true;
        }
    }

    /// The inventory screen, laid out like base Tribes' station menu: Armor,
    /// Weapons, Packs, Miscellany. Standing at the station heals you and
    /// restocks rounds, grenades and mines; the buttons (or 1-6) pick armor
    /// and a pack. Purchases are intents the server checks.
    fn shop_ui(&mut self, ctx: &egui::Context) {
        use peakrunner_core::sim::{deploy::{team_limit, DeployKind}, loadout::*, throwables::max_throwables};
        use crate::keybinds::Action;
        let Some(me) = self.world.players.get(self.world.player_id).cloned() else { return };
        let out = |kind: DeployKind| self.world.deployables.iter().filter(|e| e.team == me.team && e.kind == kind).count();
        let heavy = me.armor == ArmorClass::Heavy;
        let max = max_ammo(me.armor);
        let throws = max_throwables(me.armor);
        let packs = [(DeployKind::Turret, BUY_TURRET, "Turret", "Chaingun rounds at enemies in sight, 70 m."),
            (DeployKind::Wall, BUY_WALL, "Wall", "Solid cover. Stops everyone and every shot."),
            (DeployKind::Field, BUY_FIELD, "Force field", "A door for your team; a wall for theirs."),
            (DeployKind::Ammo, BUY_AMMO, "Ammo station", "Your team restocks here.")];
        let mut buy = 0u8;
        ctx.input(|i| for (key, code) in [(egui::Key::Num1, BUY_LIGHT), (egui::Key::Num2, BUY_HEAVY), (egui::Key::Num3, BUY_TURRET),
            (egui::Key::Num4, BUY_WALL), (egui::Key::Num5, BUY_FIELD), (egui::Key::Num6, BUY_AMMO), (egui::Key::Num7, BUY_RIFLE)] {
            if i.key_pressed(key) { buy = code; }
        });
        let mut close = false;
        let heading = |ui: &mut egui::Ui, text: &str| {
            ui.add_space(8.0);
            ui.label(RichText::new(text).size(13.0).color(GLACIER).strong());
            ui.separator();
        };
        egui::Area::new(egui::Id::new("inventory_station"))
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(12, 18, 28, 240))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(40, 70, 90)))
                    .corner_radius(10.0)
                    .inner_margin(20.0)
                    .show(ui, |ui| {
                        let width = (ctx.content_rect().width() - 64.0).clamp(280.0, 560.0);
                        ui.set_width(width);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Inventory station").size(24.0).color(FG).strong());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add(secondary_button("Close")).clicked() { close = true; }
                            });
                        });
                        ui.label(RichText::new("Standing here heals you and restocks rounds, grenades and mines.").size(12.0).color(MUTED));
                        // A row: its shortcut, name and blurb; a note (counts); and, for
                        // things you can take, a button, disabled with `why` when not.
                        let row = |ui: &mut egui::Ui, key: &str, name: &str, about: &str, note: &str, action: Option<(bool, &str)>| -> bool {
                            let mut clicked = false;
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(key).monospace().color(MUTED));
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(name).size(15.0).color(FG));
                                    ui.label(RichText::new(about).size(11.0).color(MUTED));
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if let Some((enabled, why)) = action {
                                        clicked = ui.add_enabled(enabled, egui::Button::new(RichText::new(if enabled { "Take" } else { why }).color(FG))
                                            .min_size(Vec2::new(86.0, 26.0))).clicked();
                                    }
                                    if !note.is_empty() { ui.label(RichText::new(note).size(12.0).color(GLACIER)); }
                                });
                            });
                            clicked
                        };
                        egui::ScrollArea::vertical().max_height((ctx.content_rect().height() - 160.0).max(200.0)).show(ui, |ui| {
                            heading(ui, "ARMOR");
                            if row(ui, "1", "Light armor", "Fast, jets hard. Carries the grenade launcher.", "", Some((heavy, "Wearing"))) { buy = BUY_LIGHT; }
                            if row(ui, "2", "Heavy armor", "Twice the armor, slow, a long weak jet. Carries the mortar.", "", Some((!heavy, "Wearing"))) { buy = BUY_HEAVY; }
                            heading(ui, "WEAPONS");
                            let third = if heavy { "Mortar" } else { "Grenade launcher" };
                            for (k, (name, about)) in [("Disc launcher", "Splash damage and disc jumps."), ("Chaingun", "Fast rounds, some spread."),
                                (third, if heavy { "Sticks where it lands; a big blast." } else { "Bouncing shells on a short fuse." })].into_iter().enumerate() {
                                row(ui, " ", name, about, &format!("{}/{}", me.ammo[k], max[k]), None);
                            }
                            let (rifle, blurb) = if heavy { ("Railgun", "Heavy only. A very fast straight slug; hard hitting. Hold E to zoom.") }
                                else { ("Laser rifle", "Light only. An instant beam at any range; spends energy. Hold E to zoom.") };
                            let note = if !me.rifle { String::new() } else if heavy { format!("{}/{}", me.ammo[3], max[3]) } else { "Energy".to_string() };
                            if row(ui, "7", rifle, blurb, &note, Some((!me.rifle, "Carrying"))) { buy = BUY_RIFLE; }
                            heading(ui, "PACKS · one at a time");
                            for (n, (kind, code, name, about)) in packs.into_iter().enumerate() {
                                let carrying = me.pack == Some(kind);
                                let full = out(kind) >= team_limit(kind);
                                // A full team can still carry one, to place when one comes down.
                                let note = format!("{}/{} out{}", out(kind), team_limit(kind), if full { " (full)" } else { "" });
                                if row(ui, &(n + 3).to_string(), name, about, &note, Some((!carrying, "Carrying"))) { buy = code; }
                            }
                            heading(ui, "MISCELLANY");
                            let keys = &self.ctl.keys;
                            row(ui, " ", "Grenades", &format!("Hold {} to wind up, release to throw. 2 s fuse.", keys.name(Action::Grenade)),
                                &format!("{}/{}", me.throwables[0], throws[0]), None);
                            row(ui, " ", "Mines", &format!("Hold {} to throw. Arms at rest; enemies set it off.", keys.name(Action::Mine)),
                                &format!("{}/{}", me.throwables[1], throws[1]), None);
                            row(ui, " ", "Repair tool", &format!("Hold {}. Spends energy.", keys.name(Action::Repair)), "Carried", None);
                        });
                        ui.add_space(6.0);
                        ui.label(RichText::new(format!("{} or Esc closes · 1-7 take", self.ctl.keys.name(Action::Use))).size(11.0).color(MUTED));
                    });
            });
        if buy != 0 { self.world.input.buy = buy; }
        if close { self.close_shop(); }
    }

    /// Key bindings: click an action, then press its new key. A key already
    /// in use swaps onto the action's old key. Saved with the preferences.
    fn controls_ui(&mut self, ctx: &egui::Context) {
        use crate::keybinds::{Action, Keybinds};
        if let Some(action) = self.ctl.rebinding {
            let pressed = ctx.input_mut(|i| {
                let key = i.events.iter().find_map(|e| match e {
                    egui::Event::Key { key, pressed: true, repeat: false, .. } => Some(*key),
                    _ => None,
                });
                if key.is_some() { i.events.retain(|e| !matches!(e, egui::Event::Key { .. } | egui::Event::Text(_))); }
                key
            });
            if let Some(key) = pressed {
                if key != egui::Key::Escape { self.ctl.keys.bind(action, key); }
                self.ctl.rebinding = None;
                self.save_keys();
            }
        }
        let mut open = true;
        let mut reset = false;
        egui::Window::new("Controls")
            .order(egui::Order::Foreground)
            .frame(egui::Frame::new().fill(Color32::from_rgb(16, 22, 32)).stroke(egui::Stroke::new(1.0, Color32::from_rgb(40, 70, 90))).corner_radius(10.0).inner_margin(16.0))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(RichText::new("Click an action, then press a key. Esc cancels. Mouse: left fire, right jet. Arrow keys also move.")
                    .size(12.0).color(MUTED));
                egui::ScrollArea::vertical().max_height((ctx.content_rect().height() - 180.0).max(200.0)).show(ui, |ui| {
                    egui::Grid::new("keybinds").num_columns(2).spacing([24.0, 6.0]).striped(true).show(ui, |ui| {
                        for action in Action::ALL {
                            ui.label(RichText::new(action.label()).color(FG));
                            let waiting = self.ctl.rebinding == Some(action);
                            let text = if waiting { "Press a key…".to_string() } else if action == Action::Suicide {
                                format!("Ctrl+{}", self.ctl.keys.name(action)) } else { self.ctl.keys.name(action).to_string() };
                            if ui.add(egui::Button::new(RichText::new(text).monospace()).selected(waiting).min_size(Vec2::new(120.0, 24.0))).clicked() {
                                self.ctl.rebinding = if waiting { None } else { Some(action) };
                            }
                            ui.end_row();
                        }
                    });
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.add_enabled(!self.ctl.keys.is_default(), egui::Button::new("Reset to defaults")).clicked() { reset = true; }
                });
            });
        if reset {
            self.ctl.keys = Keybinds::default();
            self.ctl.rebinding = None;
            self.save_keys();
        }
        if !open {
            self.ctl.keys_open = false;
            self.ctl.rebinding = None;
        }
    }

    fn save_keys(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        self.net.preferences.set_keys(self.ctl.keys.clone());
    }

    fn pause_ui(&mut self, ui: &mut egui::Ui) {
        egui::Area::new(egui::Id::new("pause"))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(16, 22, 32, 235))
                    .corner_radius(10.0)
                    .inner_margin(22.0)
                    .show(ui, |ui| {
                        ui.set_width(280.0);
                        ui.label(RichText::new(if self.online() { "Match menu" } else { "Paused" }).size(28.0).color(FG).strong());
                        if self.online() { ui.label(RichText::new("The match continues while this menu is open.").color(MUTED)); }
                        ui.add_space(12.0);
                        #[cfg(not(target_arch = "wasm32"))]
                        if self.online() {
                            ui.label(RichText::new("Your name").color(MUTED));
                            if ui.add(egui::TextEdit::singleline(&mut self.net.name).char_limit(24)).changed() {
                                self.net.settings_edited();
                            }
                            let valid = peakrunner_core::names::validate(&self.net.name).is_some();
                            ui.label(RichText::new(peakrunner_core::names::HELP).size(11.0).color(MUTED));
                            let ready = self.net.rename_sent.is_none_or(|sent| sent.elapsed().as_secs() >= 11);
                            if ui.add_enabled(valid && ready, egui::Button::new("Apply name")).clicked() {
                                if let Some(session) = &self.net.session {
                                    if session.rename(self.net.name.clone()) { self.net.rename_sent = Some(std::time::Instant::now()); }
                                }
                            }
                            if let Some(session) = &self.net.session {
                                let lobby = session.lobby();
                                if let Some(p) = lobby.snapshot.as_ref().and_then(|s| s.players.iter().find(|p| p.net_id == lobby.player_id)) {
                                    ui.label(RichText::new(format!("Current: {}", p.name)).color(FG));
                                }
                            }
                            ui.label(RichText::new(if ready { "One change every 10 seconds." } else { "Name requested. Please wait before changing again." }).size(11.0).color(MUTED));
                            ui.add_space(8.0);
                        }
                        if big(ui, "Resume", true) {
                            self.resume(ui.ctx());
                        }
                        if big(ui, "Controls", false) { self.ctl.keys_open = true; }
                        let mute = if self.audio.muted() { "Unmute" } else { "Mute" };
                        if big(ui, mute, false) {
                            let next = !self.audio.muted();
                            self.audio.set_muted(next);
                        }
                        if big(ui, "Leave rift", false) {
                            self.menu(ui.ctx());
                        }
                    });
            });
    }

    fn end_ui(&mut self, ui: &mut egui::Ui) {
        let hud = self.hud.clone();
        egui::Area::new(egui::Id::new("end"))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(16, 22, 32, 238))
                    .corner_radius(10.0)
                    .inner_margin(28.0)
                    .show(ui, |ui| {
                        ui.set_width(320.0);
                        let (title, ember, glacier, detail) = hud
                            .as_ref()
                            .map(|h| {
                                let title = if h.winner == -1 {
                                    "Draw"
                                } else if h.winner == h.team as i32 {
                                    "Victory"
                                } else {
                                    "Defeat"
                                };
                                (
                                    title,
                                    h.ember,
                                    h.glacier,
                                    format!("{} frags · {} deaths", h.kills, h.deaths),
                                )
                            })
                            .unwrap_or(("Match over", 0, 0, String::new()));
                        ui.label(RichText::new("MATCH OVER").size(12.0).color(MUTED));
                        ui.label(RichText::new(title).size(48.0).color(FG).strong());
                        ui.label(
                            RichText::new(format!("{ember}  –  {glacier}"))
                                .size(28.0)
                                .color(FG),
                        );
                        ui.label(RichText::new(detail).color(MUTED));
                        ui.add_space(12.0);
                        if self.online() {
                            ui.label(RichText::new(&self.world.message).color(GLACIER));
                        } else if big(ui, "Start match", true) {
                            self.start(ui.ctx());
                        }
                        if big(ui, "Menu", false) {
                            self.menu(ui.ctx());
                        }
                    });
            });
    }
}

fn reference_measurements(ui:&egui::Ui,world:&World) {
    let Some(pack)=peakrunner_core::map_pack::on(world.map) else {return};
    if !pack.manifest.private_reference {return;}
    let Some(player)=world.players.get(world.player_id) else {return};
    let Some(base)=pack.manifest.reference_bases.iter().min_by(|a,b| {
        player.pos.distance_squared(glam::Vec3::from_array(a.position))
            .total_cmp(&player.pos.distance_squared(glam::Vec3::from_array(b.position)))
    }) else {return};
    let delta=player.pos-glam::Vec3::from_array(base.position);
    let local=base.world_to_local.map(|row|glam::Vec3::from_array(row).dot(delta));
    let direction=glam::Vec3::new(-player.yaw.sin()*player.pitch.cos(),player.pitch.sin(),-player.yaw.cos()*player.pitch.cos());
    let distance=peakrunner_core::interior_survey::structure_distance(pack,player.pos,direction,500.)
        .map(|metres|format!("{:.2} m",metres)).unwrap_or_else(||"no structure within 500 m".into());
    let text=format!("PRIVATE BROADSIDE REFERENCE\nSource XYZ: {:.2}  {:.2}  {:.2}\n{} local XY: {:.2}  {:.2}\nCenter above deck: {:.2} m\nStructure ray: {}",
        player.pos.x-1024.,player.pos.z-1024.,player.pos.y,base.name,local[0],local[1],local[2],distance);
    let origin=ui.max_rect().left_top()+Vec2::new(22.,114.);
    let area=egui::Rect::from_min_size(origin-Vec2::splat(8.),Vec2::new(370.,100.));
    ui.painter().rect_filled(area,4.,BG);
    ui.painter().text(origin,Align2::LEFT_TOP,text,FontId::monospace(12.),FG);
}

fn play_hud(
    ui: &mut egui::Ui,
    hud: &Hud,
    touch: bool,
    stick: &mut [f32; 2],
) -> (bool, bool, bool, bool, egui::Vec2) {
    let rect = ui.max_rect();
    let painter = ui.painter();
    let kph = (hud.speed * 3.6).round() as i32;
    let mm = (hud.time / 60.0).floor() as i32;
    let ss = (hud.time % 60.0).floor() as i32;
    painter.text(
        rect.left_top() + Vec2::new(22.0, 18.0),
        Align2::LEFT_TOP,
        "SPEED",
        FontId::proportional(12.0),
        MUTED,
    );
    painter.text(
        rect.left_top() + Vec2::new(22.0, 32.0),
        Align2::LEFT_TOP,
        format!("{kph}"),
        FontId::proportional(42.0),
        FG,
    );
    let stance = if hud.ski == 1 {
        "Skiing"
    } else if hud.jet == 1 {
        "Jet"
    } else if hud.alive == 1 {
        "Planted"
    } else {
        "Down"
    };
    painter.text(
        rect.left_top() + Vec2::new(22.0, 78.0),
        Align2::LEFT_TOP,
        stance,
        FontId::proportional(13.0),
        MUTED,
    );
    painter.text(
        rect.center_top() + Vec2::new(0.0, 18.0),
        Align2::CENTER_TOP,
        match &hud.ffa_line { Some(line) => format!("{line}    {mm}:{ss:02}"), None => format!("{}    {mm}:{ss:02}    {}", hud.ember, hud.glacier) },
        FontId::proportional(26.0),
        FG,
    );
    painter.text(
        rect.center_top() + Vec2::new(0.0, 50.0),
        Align2::CENTER_TOP,
        if hud.conditions.is_empty() { hud.goal.to_string() } else { format!("{} · {}", hud.goal, hud.conditions) },
        FontId::proportional(12.0),
        MUTED,
    );
    if !hud.msg.is_empty() {
        painter.text(
            rect.center_top() + Vec2::new(0.0, 92.0),
            Align2::CENTER_TOP,
            &hud.msg,
            FontId::proportional(((rect.width()-32.0)/(hud.msg.chars().count().max(1) as f32*0.6)).clamp(14.0,26.0)),
            FG,
        );
    }
    if hud.flag >= 0 {
        painter.text(
            rect.center_top() + Vec2::new(0.0, 124.0),
            Align2::CENTER_TOP,
            "You have the flag",
            FontId::proportional(15.0),
            EMBER,
        );
    } else if hud.own_flag == 2 {
        painter.text(
            rect.center_top() + Vec2::new(0.0, 124.0),
            Align2::CENTER_TOP,
            "Your flag is taken",
            FontId::proportional(15.0),
            EMBER,
        );
    } else if hud.own_flag == 3 {
        painter.text(
            rect.center_top() + Vec2::new(0.0, 124.0),
            Align2::CENTER_TOP,
            "Your flag is down · touch it to return it",
            FontId::proportional(15.0),
            EMBER,
        );
    }
    // The enemy flag lying loose is a chance: say so (the marker shows where).
    if hud.enemy_flag == 3 && hud.flag < 0 && !hud.football {
        painter.text(
            rect.center_top() + Vec2::new(0.0, if hud.own_flag == 1 { 124.0 } else { 144.0 }),
            Align2::CENTER_TOP,
            "Their flag is down · grab it",
            FontId::proportional(15.0),
            GLACIER,
        );
    }

    let c = rect.center();
    if hud.zoomed {
        // A sight: dark surround outside a circle, fine cross hairs.
        let r = rect.height().min(rect.width())*0.42;
        let shade = Color32::from_black_alpha(215);
        painter.rect_filled(egui::Rect::from_min_max(rect.min, egui::pos2(c.x-r, rect.max.y)), 0.0, shade);
        painter.rect_filled(egui::Rect::from_min_max(egui::pos2(c.x+r, rect.min.y), rect.max), 0.0, shade);
        painter.rect_filled(egui::Rect::from_min_max(egui::pos2(c.x-r, rect.min.y), egui::pos2(c.x+r, c.y-r)), 0.0, shade);
        painter.rect_filled(egui::Rect::from_min_max(egui::pos2(c.x-r, c.y+r), egui::pos2(c.x+r, rect.max.y)), 0.0, shade);
        for k in 0..48 {
            let a0 = k as f32/48.0*std::f32::consts::TAU; let a1 = (k+1) as f32/48.0*std::f32::consts::TAU;
            // Corner wedges between the square and the circle.
            let corner = |a: f32| egui::pos2(c.x+a.cos()*r*1.42, c.y+a.sin()*r*1.42);
            painter.add(egui::Shape::convex_polygon(vec![egui::pos2(c.x+a0.cos()*r, c.y+a0.sin()*r), corner(a0), corner(a1),
                egui::pos2(c.x+a1.cos()*r, c.y+a1.sin()*r)], shade, egui::Stroke::NONE));
        }
        painter.circle_stroke(c, r, egui::Stroke::new(2.0, Color32::from_rgb(120, 220, 255)));
        for (a, b) in [(egui::vec2(-r, 0.0), egui::vec2(-8.0, 0.0)), (egui::vec2(8.0, 0.0), egui::vec2(r, 0.0)),
            (egui::vec2(0.0, -r), egui::vec2(0.0, -8.0)), (egui::vec2(0.0, 8.0), egui::vec2(0.0, r))] {
            painter.line_segment([c+a, c+b], egui::Stroke::new(1.0, Color32::from_rgb(120, 220, 255)));
        }
    }
    let hit = if hud.hit > 0.0 { EMBER } else { FG };
    for (a, b) in [
        (c + Vec2::new(-14.0, 0.0), c + Vec2::new(-6.0, 0.0)),
        (c + Vec2::new(6.0, 0.0), c + Vec2::new(14.0, 0.0)),
        (c + Vec2::new(0.0, -14.0), c + Vec2::new(0.0, -6.0)),
        (c + Vec2::new(0.0, 6.0), c + Vec2::new(0.0, 14.0)),
    ] {
        painter.line_segment([a, b], egui::Stroke::new(1.5, hit));
    }

    bar(ui, rect.left_bottom() + Vec2::new(22.0, -78.0), "Armor", hud.health, 100.0, EMBER);
    bar(ui, rect.left_bottom() + Vec2::new(22.0, -48.0), "Energy", hud.energy, ENERGY_MAX, GLACIER);
    if hud.alive == 1 && !hud.football {
        let armor = if hud.armor == 1 { "Heavy armor" } else { "Light armor" };
        use crate::keybinds::Action;
        let k = &hud.keys;
        let (text, color) = if hud.repairing == 1 { (format!("{armor} · repairing…"), Color32::from_rgb(120, 230, 140)) }
            else { (format!("{armor} · hold {} to repair", k.name(Action::Repair)), FG) };
        painter.text(rect.left_bottom() + Vec2::new(214.0, -78.0), Align2::LEFT_BOTTOM, text, FontId::proportional(13.0), color);
        let pack = match hud.pack { 0 => "Turret pack", 1 => "Wall pack", 2 => "Force field pack", 3 => "Ammo station pack", _ => "" };
        if !pack.is_empty() {
            painter.text(rect.left_bottom() + Vec2::new(214.0, -60.0), Align2::LEFT_BOTTOM,
                format!("{pack} · {} to deploy", k.name(Action::Deploy)), FontId::proportional(13.0), GLACIER);
        }
        if let Some(problem) = hud.placing {
            let (text, color) = if problem.is_empty() { ("Click to place".to_string(), GLACIER) }
                else { (problem.to_string(), MUTED) };
            painter.text(rect.center() + Vec2::new(0.0, 34.0), Align2::CENTER_TOP, text, FontId::proportional(15.0), color);
            painter.text(rect.center() + Vec2::new(0.0, 54.0), Align2::CENTER_TOP,
                format!("Wheel or {} turns · {} puts it away", k.name(Action::View), k.name(Action::Deploy)),
                FontId::proportional(12.0), MUTED);
        }
        // Grenades and mines, and the wind-up while one is held.
        painter.text(rect.left_bottom() + Vec2::new(214.0, -42.0), Align2::LEFT_BOTTOM,
            format!("Grenades {}/{} ({}) · Mines {}/{} ({})", hud.grenades, hud.grenades_max, k.name(Action::Grenade),
                hud.mines, hud.mines_max, k.name(Action::Mine)), FontId::proportional(13.0), MUTED);
        if let Some((what, held)) = hud.winding {
            let t = (held / peakrunner_core::sim::throwables::WIND_UP).clamp(0.0, 1.0);
            let at = rect.center() + Vec2::new(-60.0, 46.0);
            painter.rect_filled(egui::Rect::from_min_size(at, Vec2::new(120.0, 6.0)), 3.0, Color32::from_black_alpha(140));
            painter.rect_filled(egui::Rect::from_min_size(at, Vec2::new(120.0 * (0.3 + 0.7 * t), 6.0)), 3.0, EMBER);
            painter.text(at + Vec2::new(60.0, 10.0), Align2::CENTER_TOP,
                if what == peakrunner_core::sim::throwables::THROW_MINE { "Mine" } else { "Grenade" }, FontId::proportional(12.0), FG);
        }
        let use_key = k.name(Action::Use);
        let station = match hud.station {
            1 => format!("INVENTORY STATION · {use_key} to open"),
            2 => format!("AMMO STATION · hold {use_key} to restock"),
            _ => String::new(),
        };
        if !station.is_empty() {
            painter.text(rect.center_bottom() + Vec2::new(0.0, -120.0), Align2::CENTER_BOTTOM, station, FontId::proportional(14.0), GLACIER);
        }
    }
    painter.text(
        rect.center_bottom() + Vec2::new(0.0, -56.0),
        Align2::CENTER_BOTTOM,
        &if hud.football { "Football".to_string() } else {
            let name = match hud.weapon { 0 => "Disc", 1 => "Chaingun", 3 if hud.rifle == 2 => "Railgun", 3 => "Laser rifle",
                _ if hud.armor == 1 => "Mortar", _ => "Grenade launcher" };
            if hud.placing.is_some() { "Deployer".to_string() } else { format!("{name} · {}/{}", hud.ammo, hud.ammo_max) }
        },
        FontId::proportional(22.0),
        FG,
    );
    painter.text(
        rect.center_bottom() + Vec2::new(0.0, -32.0),
        Align2::CENTER_BOTTOM,
        if hud.football { format!("{} pts · {} down", hud.kills, hud.deaths) } else { format!("{} frag · {} down", hud.kills, hud.deaths) },
        FontId::proportional(13.0),
        MUTED,
    );
    if !touch {
        painter.text(
            rect.right_bottom() + Vec2::new(-22.0, -40.0),
            Align2::RIGHT_BOTTOM,
            format!("Hold {} to ski · Right click jet · Ctrl+{} respawn · Esc\n{}", hud.keys.name(crate::keybinds::Action::Jump),
                hud.keys.name(crate::keybinds::Action::Suicide), if hud.team == 0 { "Ember" } else { "Glacier" }),
            FontId::proportional(13.0),
            MUTED,
        );
    }
    minimap(ui, rect.right_top() + Vec2::new(-22.0, 18.0), hud);

    if touch {
        touch_controls(ui, stick)
    } else {
        (false, false, false, false, egui::Vec2::ZERO)
    }
}

fn bar(ui: &egui::Ui, origin: egui::Pos2, label: &str, value: f32, max: f32, color: Color32) {
    let painter = ui.painter();
    painter.text(origin, Align2::LEFT_TOP, format!("{label}  {:.0}", value), FontId::proportional(12.0), MUTED);
    let track = egui::Rect::from_min_size(origin + Vec2::new(0.0, 16.0), Vec2::new(180.0, 6.0));
    painter.rect_filled(track, 3.0, Color32::from_rgb(28, 36, 48));
    let mut fill = track;
    let span = if max > 0.0 { max } else { 1.0 };
    fill.max.x = track.min.x + track.width() * (value / span).clamp(0.0, 1.0);
    painter.rect_filled(fill, 3.0, color);
}

fn minimap(ui: &egui::Ui, top_right: egui::Pos2, hud: &Hud) {
    let size = 112.0;
    let rect = egui::Rect::from_min_max(top_right - Vec2::new(size, 0.0), top_right + Vec2::new(0.0, size));
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(8, 13, 20, 210));
    painter.rect_stroke(rect, 4.0, egui::Stroke::new(1.0, Color32::from_white_alpha(40)), egui::StrokeKind::Inside);
    for part in hud.blips.split(';') {
        if part.is_empty() {
            continue;
        }
        let mut bits = part.split(',');
        let x = bits.next().and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
        let z = bits.next().and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
        let team = bits.next().and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
        let kind = bits.next().unwrap_or("0");
        let span = if hud.map_size > 1.0 { hud.map_size } else { 256.0 };
        let px = rect.left() + 6.0 + (x / span) * (size - 12.0);
        let py = rect.top() + 6.0 + (z / span) * (size - 12.0);
        let color = if team == 0 { EMBER } else { GLACIER };
        let p = egui::pos2(px, py);
        if kind == "2" {
            painter.rect_filled(egui::Rect::from_center_size(p, Vec2::splat(6.0)), 0.0, color);
        } else if kind == "1" {
            painter.circle_filled(p, 4.0, color);
            painter.circle_stroke(p, 5.0, egui::Stroke::new(1.0, FG));
            let fx = -hud.yaw.sin();
            let fz = -hud.yaw.cos();
            painter.line_segment(
                [p, p + Vec2::new(fx, fz) * 10.0],
                egui::Stroke::new(1.5, FG),
            );
        } else {
            painter.circle_filled(p, 2.4, color);
        }
    }
}

fn touch_controls(ui: &mut egui::Ui, stick: &mut [f32; 2]) -> (bool, bool, bool, bool, egui::Vec2) {
    let rect = ui.max_rect();
    let zone = egui::Rect::from_min_size(rect.left_bottom() + Vec2::new(16.0, -168.0), Vec2::splat(140.0));
    let drag = ui.interact(zone, ui.id().with("move"), Sense::drag());
    ui.painter().circle_stroke(zone.center(), 56.0, egui::Stroke::new(1.0, Color32::from_white_alpha(50)));
    if drag.dragged() {
        if let Some(p) = drag.interact_pointer_pos() {
            let d = p - zone.center();
            stick[0] = (d.x / 56.0).clamp(-1.0, 1.0);
            stick[1] = (-d.y / 56.0).clamp(-1.0, 1.0);
        }
    } else if !drag.is_pointer_button_down_on() {
        stick[0] = 0.0;
        stick[1] = 0.0;
    }
    let look = egui::Rect::from_min_max(
        egui::pos2(rect.center().x, rect.top()),
        rect.right_bottom() + Vec2::new(0.0, -150.0),
    );
    let look_drag = ui.interact(look, ui.id().with("look"), Sense::drag());
    let look = if look_drag.dragged() {
        look_drag.drag_delta() * 1.6
    } else {
        egui::Vec2::ZERO
    };

    let fire = hold_button(ui, rect.right_bottom() + Vec2::new(-28.0, -28.0), "Fire", EMBER);
    let jet = hold_button(ui, rect.right_bottom() + Vec2::new(-108.0, -28.0), "Jet", GLACIER);
    let jump = hold_button(ui, rect.right_bottom() + Vec2::new(-188.0, -28.0), "Ski", FG);
    let swap = tap_button(ui, rect.right_bottom() + Vec2::new(-268.0, -28.0), "Swap");
    (jump, fire, jet, swap, look)
}

fn hold_button(ui: &mut egui::Ui, center_br: egui::Pos2, label: &str, color: Color32) -> bool {
    let rect = egui::Rect::from_center_size(center_br + Vec2::new(-32.0, -32.0), Vec2::splat(64.0));
    let resp = ui.interact(rect, ui.id().with(label), Sense::click_and_drag());
    ui.painter().circle_filled(rect.center(), 32.0, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 180));
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, FontId::proportional(14.0), BG);
    resp.is_pointer_button_down_on()
}

fn tap_button(ui: &mut egui::Ui, center_br: egui::Pos2, label: &str) -> bool {
    let rect = egui::Rect::from_center_size(center_br + Vec2::new(-28.0, -28.0), Vec2::splat(56.0));
    let resp = ui.interact(rect, ui.id().with(label), Sense::click());
    ui.painter().circle_stroke(rect.center(), 28.0, egui::Stroke::new(1.0, FG));
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, FontId::proportional(13.0), FG);
    resp.clicked()
}

impl PeakRunnerApp {
    fn online(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        { self.net.dropped_in }
        #[cfg(target_arch = "wasm32")]
        { false }
    }

    fn poll_net(&mut self, _ctx: &egui::Context) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(inbox) = &self.net.inbox {
                if let Ok(result) = inbox.try_recv() {
                    self.net.listing = match result {
                        Ok(servers) => Listing::Ready(servers),
                        Err(err) => Listing::Failed(format!("Directory unavailable: {err}. Direct joining still works.")),
                    };
                    self.net.inbox = None;
                }
            }
            if let Some(session) = &self.net.session { self.net.lobby = session.lobby(); }
            if let Some(error) = self.net.lobby.error.clone() {
                if self.net.session.is_some() {
                    self.net.session.take();
                    self.net.dropped_in = false;
                    self.mode = Mode::Lobby;
                    self.world.state = MatchState::Paused;
                    self.audio.set_jet(false);
                    self.grab(_ctx, false);
                    self.net.lobby.error = Some(error);
                }
                return;
            }
            if self.net.session.is_some() && self.net.lobby.connected {
                if let Some(snapshot) = self.net.lobby.snapshot.clone() {
                    if !self.net.dropped_in {
                        self.net.predictor = crate::online::Online::default();
                        self.net.dropped_in = true;
                        self.world.players.clear();
                        self.world.input = Default::default();
                        self.mode = Mode::Play;
                        self.wait_fire_release = true;
                        self.audio.unlock();
                        self.audio.play("start");
                    }
                    self.net.predictor.receive(&mut self.world, &snapshot, self.net.lobby.player_id);
                    self.map = snapshot.map;
                    self.ember = self.world.players[self.world.player_id].team == crate::sim::Team::Ember;
                    if snapshot.phase == crate::sim::Phase::Intermission && self.mode != Mode::Pause {
                        self.mode = Mode::End;
                    } else if snapshot.phase != crate::sim::Phase::Intermission && self.mode == Mode::End {
                        self.mode = Mode::Play;
                        self.wait_fire_release = true;
                    }
                }
            }
        }
    }

    fn open_browser(&mut self) {
        self.mode = Mode::Browser;
        #[cfg(not(target_arch = "wasm32"))]
        self.refresh_servers();
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn refresh_servers(&mut self) {
        self.net.save_settings();
        let addr = self.net.directory.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        self.net.inbox = Some(rx);
        self.net.listing = Listing::Loading;
        std::thread::spawn(move || {
            let result = peakrunner_net::browse(&addr).map_err(|err| err.to_string());
            let _ = tx.send(result);
        });
    }

    fn browser_ui(&mut self, ui: &mut egui::Ui) {
        egui::Area::new(egui::Id::new("browser"))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(12, 18, 28, 236))
                    .corner_radius(10.0)
                    .inner_margin(22.0)
                    .show(ui, |ui| {
                        ui.set_width(460.0);
                        ui.label(RichText::new("FIND A RIFT").size(12.0).color(GLACIER));
                        ui.label(RichText::new("Open games").size(28.0).color(FG).strong());
                        ui.add_space(8.0);
                        #[cfg(target_arch = "wasm32")]
                        {
                            ui.label(RichText::new("Match listing runs in the desktop game.").color(MUTED));
                            if big(ui, "Back", false) {
                                self.mode = Mode::Menu;
                            }
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        self.browser_native(ui);
                    });
            });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn browser_native(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Encrypted public matches · server-authoritative CTF").size(12.0).color(GLACIER));
        ui.label(RichText::new("Directory").size(12.0).color(MUTED));
        if ui.add(egui::TextEdit::singleline(&mut self.net.directory).char_limit(2048)).changed() {
            self.net.settings_edited();
        }
        ui.label(RichText::new("Your name").size(12.0).color(MUTED));
        if ui.add(egui::TextEdit::singleline(&mut self.net.name).char_limit(24)).changed() {
            self.net.settings_edited();
        }
        ui.label(RichText::new(peakrunner_core::names::HELP).size(11.0).color(MUTED));
        ui.label(RichText::new("Match password (optional)").size(12.0).color(MUTED));
        ui.add(egui::TextEdit::singleline(&mut self.net.password).password(true));
        ui.label(RichText::new("Direct server address").size(12.0).color(MUTED));
        if ui.add(egui::TextEdit::singleline(&mut self.net.direct).char_limit(2048)).changed() {
            self.net.settings_edited();
        }
        let mut antialiasing=scene::MSAA_WANTED.load(std::sync::atomic::Ordering::Relaxed);
        if ui.checkbox(&mut antialiasing,"Anti-aliasing (smoother edges; off can help slower GPUs)").changed() {
            scene::MSAA_WANTED.store(antialiasing,std::sync::atomic::Ordering::Relaxed);
            self.net.preferences.set_antialiasing(antialiasing);
        }
        ui.horizontal(|ui| {
            use crate::preferences::Bloom;
            ui.label(RichText::new("Glow (bloom)").size(12.0).color(MUTED));
            let current=self.net.preferences.saved.bloom;
            for (value,label) in [(Bloom::Off,"Off"),(Bloom::Low,"Low"),(Bloom::High,"High")] {
                if ui.selectable_label(current==value,label).clicked() && current!=value {
                    scene::BLOOM_LEVEL.store(value.level(),std::sync::atomic::Ordering::Relaxed);
                    self.net.preferences.set_bloom(value);
                }
            }
        });
        if let Some(warning)=&self.net.preferences.warning {
            ui.label(RichText::new(warning).size(11.0).color(EMBER));
        } else if let Some(path)=&self.net.preferences.path {
            ui.label(RichText::new("Valid settings save automatically. Passwords are never saved.").size(11.0).color(MUTED))
                .on_hover_text(path.display().to_string());
        }
        if ui.button("Join directly").clicked() {
            let address = self.net.direct.clone();
            if address.starts_with("quic://") {
                self.join_server(&address, 7777);
            } else if let Some((host, port)) = address.rsplit_once(':').and_then(|(h, p)| p.parse::<u16>().ok().map(|p| (h, p))) {
                self.join_server(host, port);
            } else { self.net.listing = Listing::Failed("Use quic://play.peakrunner.net:7777 or a private IP:port".into()); }
        }
        ui.label(RichText::new("Hosting is handled by the separate PeakRunner Server app.").size(12.0).color(MUTED));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Refresh").clicked() {
                self.refresh_servers();
            }
            if ui.button("Back").clicked() {
                self.net.save_settings();
                self.mode = Mode::Menu;
            }
        });
        ui.add_space(10.0);
        match &self.net.listing {
            Listing::Idle | Listing::Loading => {
                ui.label(RichText::new("Looking for games…").color(MUTED));
            }
            Listing::Failed(err) => {
                ui.label(RichText::new(err).color(EMBER));
            }
            Listing::Ready(servers) if servers.is_empty() => {
                ui.label(RichText::new("No games are advertising. Start a server, then refresh.").color(MUTED));
            }
            Listing::Ready(servers) => {
                let chosen: Vec<_> = servers.clone();
                for server in chosen {
                    let label = format!(
                        "{} · {} / {} · {}{}",
                        server.name, server.players, server.max_players, server.map,
                        if server.host.starts_with("quic://") { " · encrypted UDP" } else { " · LAN" }
                    );
                    if ui.add(egui::Button::new(label).min_size(Vec2::new(420.0, 36.0))).clicked() {
                        self.join_server(&server.host, server.port);
                    }
                }
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn join_server(&mut self, host: &str, port: u16) {
        if peakrunner_core::names::validate(&self.net.name).is_none() {
            self.net.listing = Listing::Failed(format!("Your name: {}", peakrunner_core::names::HELP));
            return;
        }
        match peakrunner_net::connect_private(host, port, &self.net.name, &self.net.password) {
            Ok(session) => {
                self.net.direct = if host.contains("://") { host.into() } else { format!("{host}:{port}") };
                self.net.settings_edited();
                self.net.save_settings();
                self.net.lobby = Default::default();
                self.net.session = Some(session);
                self.net.dropped_in = false;
                self.mode = Mode::Lobby;
            }
            Err(err) => self.net.listing = Listing::Failed(err.to_string()),
        }
    }

    fn lobby_ui(&mut self, ui: &mut egui::Ui) {
        egui::Area::new(egui::Id::new("lobby"))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(12, 18, 28, 236))
                    .corner_radius(10.0)
                    .inner_margin(22.0)
                    .show(ui, |ui| {
                        ui.set_width(360.0);
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            let lobby = self.net.lobby.clone();
                            let title = if lobby.error.is_some() {
                                "Disconnected".to_string()
                            } else if lobby.match_name.is_empty() {
                                "Connecting".to_string()
                            } else {
                                lobby.match_name.clone()
                            };
                            ui.label(RichText::new(if lobby.error.is_some() { "DISCONNECTED" } else { "IN THE RIFT" }).size(12.0).color(GLACIER));
                            ui.label(RichText::new(title).size(28.0).color(FG).strong());
                            if let Some(err) = &lobby.error {
                                ui.label(RichText::new(err).color(EMBER));
                                ui.label(RichText::new("Rejoining starts a new player session.").color(MUTED));
                                if ui.button("Reconnect").clicked() {
                                    let address = self.net.direct.clone();
                                    if address.starts_with("quic://") { self.join_server(&address, 7777); }
                                    else if let Some((host, port)) = address.rsplit_once(':').and_then(|(h, p)| p.parse::<u16>().ok().map(|p| (h, p))) {
                                        self.join_server(host, port);
                                    }
                                }
                            } else if lobby.connected {
                                ui.label(RichText::new(format!("{} · {} skiers", lobby.map, lobby.players.len())).color(MUTED));
                            } else {
                                ui.label(RichText::new("Connecting to the server…").color(MUTED));
                            }
                            ui.add_space(8.0);
                            for name in &lobby.players {
                                ui.label(RichText::new(format!("· {name}")).color(FG));
                            }
                            ui.add_space(12.0);
                        }
                        if big(ui, "Leave", false) {
                            self.leave_lobby();
                        }
                    });
            });
    }

    fn leave_lobby(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        self.net.disconnect();
        self.world.state = MatchState::Flyby;
        self.audio.set_jet(false);
        self.mode = Mode::Browser;
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct NetUi {
    preferences: crate::preferences::Store,
    settings_changed: Option<std::time::Instant>,
    rename_sent: Option<std::time::Instant>,
    password: String,
    direct: String,
    predictor: crate::online::Online,
    directory: String,
    name: String,
    listing: Listing,
    inbox: Option<std::sync::mpsc::Receiver<Result<Vec<peakrunner_net::ServerAdvert>, String>>>,
    session: Option<peakrunner_net::Session>,
    lobby: peakrunner_net::Lobby,
    dropped_in: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl NetUi {
    fn load() -> Self {
        let mut net=Self::new();
        net.preferences=crate::preferences::Store::user();
        net.name=net.preferences.saved.name.clone();
        net.directory=net.preferences.saved.directory.clone();
        net.direct=net.preferences.saved.direct.clone();
        scene::MSAA_WANTED.store(net.preferences.saved.antialiasing,std::sync::atomic::Ordering::Relaxed);
        scene::BLOOM_LEVEL.store(net.preferences.saved.bloom.level(),std::sync::atomic::Ordering::Relaxed);
        net
    }

    fn settings_edited(&mut self) {self.settings_changed=Some(std::time::Instant::now());}

    fn save_settings(&mut self) {
        if self.settings_changed.take().is_some() {
            self.preferences.save(&self.name,&self.directory,&self.direct);
        }
    }

    fn disconnect(&mut self) {
        self.rename_sent = None;
        if let Some(session) = self.session.take() { session.leave(); }
        self.lobby = Default::default();
        self.predictor = Default::default();
        self.dropped_in = false;
    }

    fn new() -> Self {
        Self {
            preferences: crate::preferences::Store::memory(),
            settings_changed: None,
            rename_sent: None,
            password: String::new(),
            direct: "quic://play.peakrunner.net:7777".into(),
            predictor: Default::default(),
            directory: "https://dir.peakrunner.net/servers".into(),
            name: "Skier".into(),
            listing: Listing::Idle,
            inbox: None,
            session: None,
            lobby: peakrunner_net::Lobby::default(),
            dropped_in: false,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for NetUi {
    fn drop(&mut self) {self.save_settings();}
}

#[cfg(not(target_arch = "wasm32"))]
enum Listing {
    Idle,
    Loading,
    Ready(Vec<peakrunner_net::ServerAdvert>),
    Failed(String),
}

#[cfg(not(target_arch = "wasm32"))]
fn rift_roster(ui: &egui::Ui, lobby: &peakrunner_net::Lobby) {
    ui.painter().text(
        ui.max_rect().center_top() + Vec2::new(0.0, 78.0),
        Align2::CENTER_TOP,
        format!("{} · {} / 8 · Tab: scoreboard", lobby.match_name, lobby.players.len()),
        egui::FontId::proportional(12.0),
        MUTED,
    );
}

fn team_button(ui: &mut egui::Ui, label: &str, on: bool, color: Color32, mut set: impl FnMut()) {
    let fill = if on { color } else { Color32::from_rgb(22, 28, 38) };
    let text = if on && label == "Glacier" { BG } else { FG };
    if ui.add(egui::Button::new(RichText::new(label).color(text)).fill(fill).min_size(Vec2::new(96.0, 36.0))).clicked() {
        set();
    }
}

fn hint(ui: &mut egui::Ui, k: &str, v: &str) {
    ui.vertical(|ui| {
        ui.label(RichText::new(k).size(11.0).color(MUTED));
        ui.label(RichText::new(v).color(FG));
    });
}

fn secondary_button(label: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(label).size(18.0).color(FG)).fill(SECONDARY_BUTTON_BG)
}

fn big(ui: &mut egui::Ui, label: &str, primary: bool) -> bool {
    let button = if primary {
        egui::Button::new(RichText::new(label).color(BG)).fill(FG)
    } else {
        secondary_button(label)
    };
    ui.add_sized(Vec2::new(280.0, 40.0), button).clicked()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod exit_tests {
    use super::*;

    #[test]
    fn closing_net_ui_saves_settings_but_never_password() {
        let dir=std::env::temp_dir().join(format!("peakrunner-netui-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let path=dir.join("client.json");
        {
            let mut net=NetUi::new();
            net.preferences=crate::preferences::Store::load(path.clone());
            net.name="Rift Pilot".into();
            net.directory="https://example.net/servers".into();
            net.direct="quic://example.net:7777".into();
            net.password="never-store-this".into();
            net.settings_edited();
        }
        let saved=crate::preferences::Store::load(path.clone());
        assert_eq!(saved.saved.name,"Rift Pilot");
        assert_eq!(saved.saved.directory,"https://example.net/servers");
        assert_eq!(saved.saved.direct,"quic://example.net:7777");
        assert!(!std::fs::read_to_string(path).unwrap().contains("never-store-this"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }
    }
    fn frame(app: &mut PeakRunnerApp, ctx: &egui::Context, events: Vec<egui::Event>, time: f64) {
        let mut output = ctx.run_ui(egui::RawInput { events, time:Some(time), focused:true,
            screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,Vec2::new(1280.,800.))), ..Default::default() }, |ui| {
            app.step(ui.ctx(), 0.0); app.chat_ui(ui.ctx());
        });
        output.textures_delta.clear();
    }
    #[test]
    fn typing_chat_blocks_gameplay_and_enter_sends_escape_cancels() {
        let mut app=PeakRunnerApp::comms_fixture(); let ctx=egui::Context::default();
        app.chat_open=false; app.chat_text.clear();
        frame(&mut app,&ctx,vec![key(egui::Key::Enter),key(egui::Key::Space)],0.);
        assert!(!app.chat_open,"gameplay keys and Enter must not open chat");
        frame(&mut app,&ctx,vec![key(egui::Key::T),egui::Event::Text("t".into())],1.);
        assert!(app.chat_open);
        assert!(!app.chat_team); assert!(app.chat_text.is_empty());
        frame(&mut app,&ctx,vec![key(egui::Key::W),key(egui::Key::Space),key(egui::Key::Num3),egui::Event::Text("hello".into())],2.);
        assert_eq!(app.world.input.move_z,0.); assert!(!app.world.input.fire && !app.world.input.jump);
        assert_eq!(app.world.input.weapon,0); assert!(!app.grabbed);
        assert_eq!(app.chat_text,"hello");
        frame(&mut app,&ctx,vec![key(egui::Key::Enter)],3.);
        assert!(!app.chat_open,"Enter must submit the message");
        assert!(app.world.feed.last().unwrap().line().ends_with(": hello"));
        frame(&mut app,&ctx,vec![key(egui::Key::Y)],4.);
        assert!(app.chat_open);
        assert!(app.chat_team);
        frame(&mut app,&ctx,vec![key(egui::Key::Escape)],5.);
        assert!(!app.chat_open && app.mode==Mode::Play,"Escape closes chat, not the match");
    }

    #[test]
    fn shift_space_and_keyboard_focus_do_not_open_chat() {
        let mut app=PeakRunnerApp::comms_fixture(); let ctx=egui::Context::default();
        app.chat_open=false;
        let shifted_space=egui::Event::Key {key:egui::Key::Space,physical_key:Some(egui::Key::Space),
            pressed:true,repeat:false,modifiers:egui::Modifiers::SHIFT};
        frame(&mut app,&ctx,vec![key(egui::Key::Tab)],1.);
        frame(&mut app,&ctx,vec![shifted_space],2.);
        assert!(!app.chat_open);
        frame(&mut app,&ctx,vec![key(egui::Key::Enter)],3.);
        assert!(!app.chat_open);
    }

    #[test]
    fn leaving_clears_the_snapshot_that_could_reenter_the_match() {
        let mut net = NetUi::new();
        net.lobby.connected = true;
        net.lobby.player_id = 42;
        net.lobby.snapshot = Some(crate::sim::Match::new(crate::terrain::MapId::Valley).snapshot());
        net.lobby.error = Some("old error".into());
        net.dropped_in = true;
        net.predictor.tick = 100;
        net.disconnect();
        assert!(!net.lobby.connected);
        assert!(net.lobby.snapshot.is_none());
        assert!(net.lobby.error.is_none());
        assert_eq!(net.lobby.player_id, 0);
        assert!(!net.dropped_in);
        assert!(net.session.is_none());
        assert_eq!(net.predictor.tick, 0);
        net.disconnect(); // Leaving twice is harmless.
    }
}

fn style_ui(ctx: &egui::Context) {
    // OS light-mode notifications must not select an unstyled light palette
    // underneath the game's explicitly light labels.
    ctx.set_theme(egui::ThemePreference::Dark);
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::TRANSPARENT;
    visuals.window_fill = Color32::from_rgba_unmultiplied(12, 18, 28, 230);
    visuals.override_text_color = Some(FG);
    visuals.widgets.inactive.bg_fill = SECONDARY_BUTTON_BG;
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(36, 44, 58);
    visuals.widgets.active.bg_fill = EMBER;
    ctx.set_visuals_of(egui::Theme::Dark, visuals);
}

#[cfg(test)]
mod theme_tests {
    use super::*;

    #[test]
    fn system_light_mode_cannot_replace_the_game_palette() {
        let ctx=egui::Context::default();
        ctx.set_theme(egui::ThemePreference::System);
        ctx.run_ui(egui::RawInput { system_theme:Some(egui::Theme::Light), ..Default::default() }, |_| {}).textures_delta.clear();
        style_ui(&ctx);
        for theme in [egui::Theme::Light,egui::Theme::Dark,egui::Theme::Light] {
            ctx.run_ui(egui::RawInput { system_theme:Some(theme), ..Default::default() }, |_| {}).textures_delta.clear();
            assert_eq!(ctx.theme(),egui::Theme::Dark);
            assert_eq!(ctx.global_style().visuals.override_text_color,Some(FG));
            assert_eq!(ctx.global_style().visuals.widgets.inactive.bg_fill,SECONDARY_BUTTON_BG);
        }
    }

    #[test]
    fn secondary_button_label_has_readable_contrast() {
        fn luminance(c:Color32)->f32 {
            let linear=|v:u8| {let s=v as f32/255.;if s<=0.04045 {s/12.92}else{((s+0.055)/1.055).powf(2.4)}};
            0.2126*linear(c.r())+0.7152*linear(c.g())+0.0722*linear(c.b())
        }
        assert!((luminance(FG)+0.05)/(luminance(SECONDARY_BUTTON_BG)+0.05)>=4.5);
    }
}
