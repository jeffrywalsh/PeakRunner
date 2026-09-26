//! Football HUD: end-zone rings, the ball (or its carrier) marked in the world
//! and at the screen edge, a status line, and play announcements. Everything
//! comes from snapshot state; announcements diff frames, so a replayed
//! snapshot announces nothing.
use egui::{Align2, Color32, FontId, Stroke, Vec2};
use glam::Vec3;
use peakrunner_core::sim::football::Ball;
use peakrunner_core::sim::{Player, Team, World};

const BALL: Color32 = Color32::from_rgb(255, 214, 120);
/// The loose ball and the carrier stay marked this far away.
const BALL_RANGE: f32 = 900.0;

fn team_color(t: Team) -> Color32 { crate::control_hud::team_color(Some(t.idx() as u8)) }
fn team_name(t: Team) -> &'static str { if t == Team::Ember { "EMBER" } else { "GLACIER" } }

pub const GOAL_LINE: &str = "Carry the ball into their end zone · two 15-minute halves";

fn outlined(painter: &egui::Painter, at: egui::Pos2, align: Align2, text: &str, size: f32, color: Color32) {
    painter.text(at + Vec2::splat(1.0), align, text, FontId::proportional(size), Color32::from_black_alpha(color.a()));
    painter.text(at, align, text, FontId::proportional(size), color);
}

/// Screen-edge arrow toward `target` when it is off screen.
fn edge_arrow(ui: &egui::Ui, eye: Vec3, dir: Vec3, fov: f32, target: Vec3, label: &str, color: Color32) {
    let rect = ui.max_rect();
    let inset = rect.shrink2(Vec2::new(82.0, 100.0));
    if inset.width() <= 0.0 || inset.height() <= 0.0 { return; }
    let Some(direction) = crate::flag_hud::bearing(target - eye, dir, fov, rect.aspect_ratio()) else { return };
    let half = inset.size() * 0.5;
    let distance = (half.x / direction.x.abs().max(0.0001)).min(half.y / direction.y.abs().max(0.0001));
    let tip = inset.center() + direction * distance;
    let side = Vec2::new(-direction.y, direction.x);
    ui.painter().add(egui::Shape::convex_polygon(vec![tip, tip - direction * 17.0 + side * 7.0, tip - direction * 17.0 - side * 7.0],
        color, Stroke::new(1.5, Color32::BLACK)));
    outlined(ui.painter(), tip - direction * 40.0, Align2::CENTER_CENTER,
        &format!("{label}\n{:.0} m", eye.distance(target)), 12.0, color);
}

pub fn draw(ui: &egui::Ui, world: &World) {
    let ball = &world.ball;
    if !ball.active { return; }
    let rect = ui.max_rect();
    let painter = ui.painter();
    let (eye, dir, fov) = crate::drawlist::view_camera(world);
    let Some(me) = world.players.get(world.player_id) else { return };
    let radius = world.end_zone_radius();
    // End zones. Stadiums paint theirs on the field; elsewhere (QA) a ring
    // is drawn. Your own zone is labelled only up close.
    let painted = peakrunner_core::sim::football::has_field(world.map);
    for team in [Team::Ember, Team::Glacier] {
        let zone = world.end_zone(team);
        let distance = eye.distance(zone);
        let alpha = crate::world_overlay::fade(distance, 1200.0);
        if alpha <= 0.0 || (team == me.team && distance > 80.0) { continue; }
        let color = crate::control_hud::fade(team_color(team), alpha);
        if !painted {
            for (r, w) in [(radius, 3.0), (radius * 0.7, 1.5)] {
                crate::control_hud::ring(painter, eye, dir, fov, rect, zone - Vec3::Y * 1.0, r, 0.0, 1.0, Stroke::new(w, color));
            }
        }
        if let Some(at) = crate::world_overlay::project(eye, dir, fov, rect, zone + Vec3::Y * 7.0) {
            let label = if team == me.team { "YOUR END ZONE" } else { "SCORE HERE" };
            outlined(painter, at, Align2::CENTER_BOTTOM, &format!("{label}\n{distance:.0} m"), 13.0, color);
        }
    }
    // The ball, loose or carried (not by you: you see it in your hands).
    let carrier = ball.carrier.and_then(|c| world.players.get(c).map(|p| (c, p)));
    if ball.in_play && carrier.is_none_or(|(c, _)| c != world.player_id) {
        let (at, label, color) = match carrier {
            Some((c, p)) => (p.pos + Vec3::Y * 2.6,
                if p.team == me.team { format!("{} · BALL", world.display_name(c)) } else { "BALL CARRIER".to_string() },
                team_color(p.team)),
            None => (ball.pos + Vec3::Y * 0.9, "BALL".to_string(), BALL),
        };
        let alpha = crate::world_overlay::fade(eye.distance(at), BALL_RANGE);
        if alpha > 0.0 {
            if let Some(p) = crate::world_overlay::project(eye, dir, fov, rect, at) {
                let pulse = 0.75 + 0.25 * (world.time * 6.0).sin();
                painter.circle_stroke(p, 9.0 * pulse + 4.0, Stroke::new(2.0, crate::control_hud::fade(BALL, alpha)));
                outlined(painter, p - Vec2::new(0.0, 16.0), Align2::CENTER_BOTTOM,
                    &format!("{label} · {:.0} m", eye.distance(at)), 12.0, crate::control_hud::fade(color, alpha));
            }
        }
    }
    if !me.alive { return; }
    if ball.carried_by(world.player_id) {
        edge_arrow(ui, eye, dir, fov, world.end_zone(me.team.other()), "SCORE HERE", team_color(me.team.other()));
    } else if ball.in_play {
        let target = carrier.map_or(ball.pos, |(_, p)| p.pos);
        edge_arrow(ui, eye, dir, fov, target, "BALL", BALL);
    }
    let bottom = rect.center_bottom() - Vec2::new(0.0, 150.0);
    let status = if me.stun > 0.0 { Some(("TACKLED — GETTING UP".to_string(), Color32::from_rgb(255, 120, 90))) }
        else if ball.carried_by(world.player_id) { Some(("YOU HAVE THE BALL · FIRE TO PASS".to_string(), BALL)) }
        else if !ball.in_play && ball.respawn > 0.0 {
            Some((format!("NEXT BALL IN {:.0}", ball.respawn.ceil()), Color32::from_rgb(220, 226, 236)))
        } else { None };
    if let Some((text, color)) = status { outlined(painter, bottom, Align2::CENTER_BOTTOM, &text, 17.0, color); }
}

/// Announcements from state changes, worded for the viewer.
#[derive(Default)]
pub struct FootballWatch { last: Option<(Ball, [u32; 2], f32)> }

impl FootballWatch {
    pub fn update(&mut self, world: &World) -> Vec<String> {
        let Some(me) = world.players.get(world.player_id) else { self.last = None; return Vec::new() };
        let now = (world.ball.clone(), world.score, me.stun);
        let mut out = Vec::new();
        if let Some((was, score, stun)) = &self.last {
            if was.active && now.0.active {
                for t in [Team::Ember, Team::Glacier] {
                    if now.1[t.idx()] > score[t.idx()] { out.push(format!("TOUCHDOWN {}", team_name(t))); }
                }
                if now.0.half > was.half { out.push("HALF TIME".into()); }
                if now.0.carrier != was.carrier {
                    if now.0.carrier == Some(world.player_id) { out.push("YOU HAVE THE BALL".into()); }
                    else if let Some(c) = now.0.carrier.filter(|&c| was.thrown_by.is_some_and(|k|
                        world.players.get(k).is_some_and(|p| world.players.get(c).is_some_and(|q| q.team != p.team)))) {
                        let name = world.display_name(c);
                        out.push(format!("{name} INTERCEPTS"));
                    }
                }
                if now.2 > 0.0 && *stun <= 0.0 { out.push("TACKLED".into()); }
            }
        }
        self.last = Some(now);
        out
    }
}

/// Sounds for an online client, from one snapshot to the next (offline play
/// raises the same names directly from the sim).
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub fn network_events(old: &Ball, new: &Ball, old_players: &[Player], players: &[Player], me: usize,
                      listener: Vec3) -> (Vec<&'static str>, Vec<(&'static str, Vec3)>) {
    let (mut events, mut spatial) = (Vec::new(), Vec::new());
    if !new.active || !old.active { return (events, spatial); }
    if new.in_play && !old.in_play { events.push("whistle"); }
    if new.half > old.half { events.push("halftime"); }
    if new.carrier != old.carrier {
        if let Some(c) = new.carrier {
            if c == me || players.get(c).is_some_and(|p| p.pos.distance(listener) < 80.0) {
                events.push("catch");
            }
        }
    }
    if new.thrown_by.is_some() && old.thrown_by != new.thrown_by && old.carrier.is_some() {
        if old.carrier == Some(me) { events.push("pass"); } else { spatial.push(("pass", new.pos)); }
    }
    for (i, p) in players.iter().enumerate() {
        let Some(o) = old_players.get(i).filter(|o| o.net_id == p.net_id && p.net_id != 0) else { continue };
        if p.stun > 0.0 && o.stun <= 0.0 {
            if i == me { events.push("tackle"); } else { spatial.push(("tackle", p.pos)); }
        }
    }
    (events, spatial)
}
