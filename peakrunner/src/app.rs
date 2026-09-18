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
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        scene::install(cc)?;
        style_ui(&cc.egui_ctx);
        Ok(Self {
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
            touch_swap: false,
            look_pending: egui::Vec2::ZERO,
            wait_fire_release: false,
            #[cfg(not(target_arch = "wasm32"))]
            pads: gilrs::Gilrs::new().ok(),
            #[cfg(not(target_arch = "wasm32"))]
            net: NetUi::new(),
        })
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
        self.poll_net(ctx);

        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if escape && self.mode == Mode::Play {
            self.pause(ctx);
        } else if escape && self.mode == Mode::Pause {
            self.resume(ctx);
        }

        if self.look_pending != egui::Vec2::ZERO {
            self.world.add_look(self.look_pending.x, self.look_pending.y);
            self.look_pending = egui::Vec2::ZERO;
        }
        let mouse = mouse::sample(ctx, self.mode == Mode::Play);
        if self.mode == Mode::Play {
            self.grab(ctx, true);
            self.world.add_look(mouse.dx, mouse.dy);
        } else {
            self.grab(ctx, false);
        }
        if self.touch_swap {
            self.world.input.weapon = if self.world.input.weapon == 0 { 1 } else { 0 };
            self.touch_swap = false;
        }

        let pad = self.poll_pad();
        let mut mx = self.stick[0] + pad.x;
        let mut mz = self.stick[1] + pad.z;
        ctx.input(|i| {
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
        self.world.input.look_stick_x = pad.lx;
        self.world.input.look_stick_y = pad.ly;

        self.world.tick(dt.min(0.1).max(0.0));
        let raw = self.world.hud_json();
        if let Ok(hud) = serde_json::from_str::<Hud>(&raw) {
            if !hud.events.is_empty() {
                for event in hud.events.split(',') {
                    self.audio.play(event);
                }
            }
            self.audio.set_jet(hud.jet == 1 && self.mode == Mode::Play);
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
        self.world.set_paused(true);
        self.mode = Mode::Pause;
        self.grab(ctx, false);
    }

    fn resume(&mut self, ctx: &egui::Context) {
        self.world.set_paused(false);
        self.mode = Mode::Play;
        self.grab(ctx, true);
    }

    fn menu(&mut self, ctx: &egui::Context) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(session) = self.net.session.take() {
                session.leave();
            }
            self.net.dropped_in = false;
        }
        self.world.state = MatchState::Flyby;
        self.mode = Mode::Menu;
        self.grab(ctx, false);
        self.audio.set_jet(false);
    }

    fn grab(&mut self, ctx: &egui::Context, lock: bool) {
        self.grabbed = lock && self.mode == Mode::Play;
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
            }
            Mode::Pause => self.pause_ui(ui),
            Mode::End => self.end_ui(ui),
            Mode::Browser => self.browser_ui(ui),
            Mode::Lobby => self.lobby_ui(ui),
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.03, 0.05, 0.08, 1.0]
    }
}

impl PeakRunnerApp {
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
                if ui.add(egui::Button::new(RichText::new("Find match").size(18.0).color(FG)).min_size(Vec2::new(180.0, 40.0))).clicked() {
                    self.open_browser();
                }
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    hint(ui, "Move", "WASD");
                    hint(ui, "Look", "Mouse");
                    hint(ui, "Jump / ski", "Space");
                    hint(ui, "Jet", "Right click");
                    hint(ui, "Jet steer", "WASD");
                    hint(ui, "Fire", "Click · 1/2");
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
                        ui.label(RichText::new("Paused").size(28.0).color(FG).strong());
                        ui.add_space(12.0);
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
                        if big(ui, "Start match", true) {
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
            FontId::proportional(26.0),
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
        if hud.weapon == 0 { "Disc" } else { "Repeater" },
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
    fn poll_net(&mut self, ctx: &egui::Context) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(inbox) = &self.net.inbox {
                if let Ok(result) = inbox.try_recv() {
                    self.net.listing = match result {
                        Ok(servers) => Listing::Ready(servers),
                        Err(err) => Listing::Failed(err),
                    };
                    self.net.inbox = None;
                }
            }
            let connected = self
                .net
                .session
                .as_ref()
                .map(|session| {
                    self.net.lobby = session.lobby();
                    self.net.lobby.connected
                })
                .unwrap_or(false);
            if connected && self.mode == Mode::Lobby && !self.net.dropped_in {
                self.drop_into_rift(ctx);
            }
            if self.mode == Mode::Play && self.net.dropped_in {
                if let Some(me) = self.world.players.get(self.world.player_id) {
                    let pose = (me.pos.x, me.pos.y, me.pos.z, me.yaw);
                    if let Some(session) = &self.net.session {
                        session.send_pose(pose.0, pose.1, pose.2, pose.3);
                    }
                }
                let mine = self.net.name.clone();
                let poses: Vec<_> = self
                    .net
                    .lobby
                    .poses
                    .iter()
                    .filter(|pose| pose.name != mine)
                    .map(|pose| {
                        let y = crate::terrain::height_on(self.world.map, pose.x, pose.z) + 1.2;
                        (pose.name.clone(), pose.x, y, pose.z, pose.yaw)
                    })
                    .collect();
                self.world.sync_remotes(&poses);
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn drop_into_rift(&mut self, ctx: &egui::Context) {
        self.net.dropped_in = true;
        self.world.set_map(self.map);
        self.world.start_rift(self.ember);
        self.mode = Mode::Play;
        self.wait_fire_release = true;
        self.grab(ctx, true);
    }

    fn open_browser(&mut self) {
        self.mode = Mode::Browser;
        #[cfg(not(target_arch = "wasm32"))]
        self.refresh_servers();
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn refresh_servers(&mut self) {
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
        ui.label(RichText::new("Directory").size(12.0).color(MUTED));
        ui.text_edit_singleline(&mut self.net.directory);
        ui.label(RichText::new("Your name").size(12.0).color(MUTED));
        ui.text_edit_singleline(&mut self.net.name);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Refresh").clicked() {
                self.refresh_servers();
            }
            if ui.button("Back").clicked() {
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
                        "{}   {} of {} connected   {}:{}",
                        server.name, server.players, server.max_players, server.host, server.port
                    );
                    if ui.add(egui::Button::new(label).min_size(Vec2::new(420.0, 36.0))).clicked() {
                        match peakrunner_net::connect(&server.host, server.port, &self.net.name) {
                            Ok(session) => {
                                self.net.session = Some(session);
                                self.mode = Mode::Lobby;
                            }
                            Err(err) => self.net.listing = Listing::Failed(err.to_string()),
                        }
                    }
                }
            }
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
                            let title = if lobby.match_name.is_empty() {
                                "Connecting".to_string()
                            } else {
                                lobby.match_name.clone()
                            };
                            ui.label(RichText::new("IN THE RIFT").size(12.0).color(GLACIER));
                            ui.label(RichText::new(title).size(28.0).color(FG).strong());
                            if let Some(err) = &lobby.error {
                                ui.label(RichText::new(err).color(EMBER));
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
        {
            if let Some(session) = self.net.session.take() {
                session.leave();
            }
            self.net.dropped_in = false;
        }
        self.mode = Mode::Browser;
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct NetUi {
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
    fn new() -> Self {
        Self {
            directory: "192.168.1.64:7780".into(),
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
enum Listing {
    Idle,
    Loading,
    Ready(Vec<peakrunner_net::ServerAdvert>),
    Failed(String),
}

#[cfg(not(target_arch = "wasm32"))]
fn rift_roster(ui: &egui::Ui, lobby: &peakrunner_net::Lobby) {
    let names = if lobby.players.is_empty() {
        "Connecting".to_string()
    } else {
        lobby.players.join("  ·  ")
    };
    ui.painter().text(
        ui.max_rect().center_top() + Vec2::new(0.0, 18.0),
        Align2::CENTER_TOP,
        format!("{}   {}", lobby.match_name, names),
        egui::FontId::proportional(16.0),
        FG,
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

fn big(ui: &mut egui::Ui, label: &str, primary: bool) -> bool {
    let button = if primary {
        egui::Button::new(RichText::new(label).color(BG)).fill(FG)
    } else {
        egui::Button::new(RichText::new(label).color(FG))
    };
    ui.add_sized(Vec2::new(280.0, 40.0), button).clicked()
}

fn style_ui(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::TRANSPARENT;
    visuals.window_fill = Color32::from_rgba_unmultiplied(12, 18, 28, 230);
    visuals.override_text_color = Some(FG);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(22, 28, 38);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(36, 44, 58);
    visuals.widgets.active.bg_fill = EMBER;
    ctx.set_visuals(visuals);
}
