use eframe::egui::{self, Align2, Color32, FontId, RichText, Sense, Vec2};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
use eframe::egui_wgpu::Callback;
use serde::Deserialize;

use crate::audio::Audio;
use crate::drawlist::build_frame;
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
        Self { chat_team:false, world, audio: Audio::silent(), mode: Mode::Play, ember: true, map: MapId::Valley,
            hud: None, frame_aspect: 1.6, stick: [0.;2], touch: false, grabbed: false,
            touch_jump: false, touch_jet: false, touch_fire: false, touch_interact: false,
            touch_swap: false, look_pending: Vec2::ZERO, wait_fire_release: false,
            pads: None, net: NetUi::new(), chat_open: true, chat_text: "Nice shot!".into(), chat_error: String::new(), chat_next: 0.0 }
    }

    #[cfg(all(test, not(target_arch = "wasm32")))]
    pub(crate) fn capture_comms(&mut self, ctx: &egui::Context) { style_ui(ctx); self.chat_ui(ctx); }

    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        scene::install(cc)?;
        style_ui(&cc.egui_ctx);
        #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
        let mut app = Self {
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
            #[cfg(not(target_arch = "wasm32"))]
            pads: gilrs::Gilrs::new().ok(),
            #[cfg(not(target_arch = "wasm32"))]
            net: NetUi::load(),
        };
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
        if self.mode != Mode::Play { self.chat_open = false; }
        if self.mode == Mode::Play && !self.chat_open {
            let public = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::T));
            let team = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Y));
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
        let gameplay_input = self.mode == Mode::Play && !self.chat_open && !was_chat && ctx.input(|i| i.focused);
        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
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
            self.world.input.weapon = (self.world.input.weapon + 1) % 3;
            self.touch_swap = false;
        }

        let pad = self.poll_pad();
        let mut mx = self.stick[0] + pad.x;
        let mut mz = self.stick[1] + pad.z;
        ctx.input(|i| {
            if !gameplay_input { return; }
            if i.key_down(egui::Key::A) || i.key_down(egui::Key::ArrowLeft) {
                mx -= 1.0;
            }
            if i.key_down(egui::Key::D) || i.key_down(egui::Key::ArrowRight) {
                mx += 1.0;
            }
            if i.key_down(egui::Key::W) || i.key_down(egui::Key::ArrowUp) {
                mz += 1.0;
            }
            if i.key_down(egui::Key::S) || i.key_down(egui::Key::ArrowDown) {
                mz -= 1.0;
            }
            if i.key_pressed(egui::Key::Num1) {
                self.world.input.weapon = 0;
            }
            if i.key_pressed(egui::Key::Num2) {
                self.world.input.weapon = 1;
            }
            if i.key_pressed(egui::Key::Num3) { self.world.input.weapon = 2; }
        });
        let jump = ctx.input(|i| i.key_down(egui::Key::Space)) || pad.jump || self.touch_jump;
        let jet = mouse.jet || pad.jet || self.touch_jet;
        let mut mouse_fire = self.mode == Mode::Play && mouse.fire;
        if self.wait_fire_release {
            if mouse_fire {
                mouse_fire = false;
            } else {
                self.wait_fire_release = false;
            }
        }
        let fire = mouse_fire || pad.fire || self.touch_fire;
        self.world.input.move_x = mx.clamp(-1.0, 1.0);
        self.world.input.move_z = mz.clamp(-1.0, 1.0);
        self.world.input.jump = jump;
        self.world.input.jet = jet && self.mode == Mode::Play;
        self.world.input.fire = fire;
        self.world.input.interact = self.mode==Mode::Play && (ctx.input(|i|i.key_down(egui::Key::E)) || self.touch_interact);
        self.world.input.look_stick_x = pad.lx;
        self.world.input.look_stick_y = pad.ly;
        if !gameplay_input {
            let weapon = self.world.input.weapon;
            self.world.input = crate::sim::Input::default();
            self.world.input.weapon = weapon;
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
        let raw = self.world.hud_json();
        let listener = self.world.camera().0;
        for (name, position) in self.world.spatial_sounds.drain(..) {
            self.audio.play_at(name, position.distance(listener));
        }
        if let Ok(hud) = serde_json::from_str::<Hud>(&raw) {
            if !hud.events.is_empty() {
                for event in hud.events.split(',') {
                    self.audio.play(event);
                }
            }
            self.audio.set_jet(hud.jet == 1 && self.mode == Mode::Play);
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
        ctx.request_repaint();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let rect = ui.max_rect();
        let pixels = ui.ctx().pixels_per_point();
        let frame = build_frame(&self.world, self.frame_aspect, ui.input(|i| i.stable_dt).max(1.0 / 120.0));
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

        match self.mode {
            Mode::Menu => self.menu_ui(ui),
            Mode::Play => {
                crate::flag_hud::draw(ui, &self.world);
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
            }
            Mode::Pause => self.pause_ui(ui),
            Mode::End => self.end_ui(ui),
            Mode::Browser => self.browser_ui(ui),
            Mode::Lobby => self.lobby_ui(ui),
        }
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
                        if !self.chat_open { ui.label(RichText::new("Public · T   Team · Y").size(11.0).color(MUTED)); }
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
                                let frag = matches!(entry, peakrunner_core::feed::Entry::Frag {..});
                                let prefix = if frag { "FRAG  " } else { "CHAT  " };
                                ui.add(egui::Label::new(RichText::new(format!("{prefix}{}", entry.line()))
                                    .color(if frag { FG } else { GLACIER }).size(12.0)).wrap());
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
                        ui.label(RichText::new(format!("{} · input ack {:.0} ms", self.net.lobby.map,
                            self.net.predictor.latency_ms)).color(MUTED));
                        egui::Grid::new("match-scores").striped(true).show(ui, |ui| {
                            ui.label("Player"); ui.label("Team"); ui.label("K / D"); ui.label("Ping"); ui.end_row();
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
        egui::Area::new(egui::Id::new("menu"))
            .anchor(Align2::LEFT_BOTTOM, Vec2::new(36.0, -28.0))
            .show(ui.ctx(), |ui| {
                ui.set_width(560.0);
                ui.label(RichText::new("SKI THE RIDGELINE").color(GLACIER).size(13.0));
                ui.add_space(4.0);
                ui.label(RichText::new("PEAKRUNNER").color(FG).size(64.0).strong());
                ui.add_space(6.0);
                ui.label(RichText::new("Ascend-inspired movement. Hold Space to ski, carry speed downhill, then jet over the next ridge. WASD steers on slopes and in the air. Right click gives lift; release it to recharge. The disc launcher takes its look from Tribes 1.").color(MUTED));
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    team_button(ui, "Ember", self.ember, EMBER, || self.ember = true);
                    team_button(ui, "Glacier", !self.ember, GLACIER, || self.ember = false);
                });
                ui.add_space(12.0);
                ui.label(RichText::new("MAP").color(GLACIER).size(13.0));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    for spec in terrain::maps() {
                        let on = self.map == spec.id;
                        let label = format!("{} · {}", spec.name, if spec.size >= 1000.0 { format!("{:.1} km", spec.size / 1000.0) } else { format!("{:.0} m", spec.size) });
                        let fill = if on { FG } else { Color32::from_rgb(22, 28, 38) };
                        let text = if on { BG } else { FG };
                        if ui.add(egui::Button::new(RichText::new(label).color(text)).fill(fill).min_size(Vec2::new(168.0, 36.0))).clicked() {
                            self.map = spec.id;
                            self.world.set_map(spec.id);
                        }
                    }
                });
                if let Some(spec) = terrain::maps().iter().find(|m| m.id == self.map) {
                    ui.label(RichText::new(spec.note).color(MUTED).size(13.0));
                }
                ui.add_space(12.0);
                if ui.add(egui::Button::new(RichText::new("Start match").size(20.0).color(BG)).fill(FG).min_size(Vec2::new(180.0, 44.0))).clicked() {
                    self.start(ui.ctx());
                }
                if ui.add(secondary_button("Find match").min_size(Vec2::new(180.0, 40.0))).clicked() {
                    self.open_browser();
                }
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    hint(ui, "Move", "WASD");
                    hint(ui, "Look", "Mouse");
                    hint(ui, "Jump / ski", "Space");
                    hint(ui, "Jet", "Right click");
                    hint(ui, "Jet steer", "WASD");
                    hint(ui, "Fire", "Click · 1/2/3");
                });
            });
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
        format!("{}    {mm}:{ss:02}    {}", hud.ember, hud.glacier),
        FontId::proportional(26.0),
        FG,
    );
    painter.text(
        rect.center_top() + Vec2::new(0.0, 50.0),
        Align2::CENTER_TOP,
        "First to 3 captures",
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
            "Your flag is on the snow",
            FontId::proportional(15.0),
            EMBER,
        );
    }

    let c = rect.center();
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
    painter.text(
        rect.center_bottom() + Vec2::new(0.0, -56.0),
        Align2::CENTER_BOTTOM,
        match hud.weapon { 0 => "Disc", 1 => "Chaingun", _ => "Grenade launcher" },
        FontId::proportional(22.0),
        FG,
    );
    painter.text(
        rect.center_bottom() + Vec2::new(0.0, -32.0),
        Align2::CENTER_BOTTOM,
        format!("{} frag · {} down", hud.kills, hud.deaths),
        FontId::proportional(13.0),
        MUTED,
    );
    if !touch {
        painter.text(
            rect.right_bottom() + Vec2::new(-22.0, -40.0),
            Align2::RIGHT_BOTTOM,
            format!("Hold Space to ski · Right click jet · Esc\n{}", if hud.team == 0 { "Ember" } else { "Glacier" }),
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
