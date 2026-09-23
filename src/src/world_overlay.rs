//! World-anchored HUD: hit bars over destructible equipment and name tags
//! over players. Line-of-sight checks here are cosmetic: every position shown
//! already arrives in snapshots, so hiding a tag behind a wall reveals nothing.
use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use glam::{Mat4, Vec3, Vec4};
use peakrunner_core::equipment::{Kind, State};
use peakrunner_core::sim::{Player, World};

pub const HIT_BAR_RANGE: f32 = 120.0;
pub const TEAMMATE_TAG_RANGE: f32 = 150.0;
pub const ENEMY_TAG_RANGE: f32 = 80.0;
/// Fraction of a range over which an overlay fades out.
const FADE_BAND: f32 = 0.2;

const TEAMMATE_NAME: Color32 = Color32::from_rgb(96, 164, 255);
const ENEMY_NAME: Color32 = Color32::from_rgb(255, 84, 72);
const SHIELD: Color32 = Color32::from_rgb(200, 206, 255);
const OFFLINE: Color32 = Color32::from_rgb(120, 124, 132);

/// Projects a world point with the render camera. `None` behind the camera
/// or outside the viewport (plus a small margin so tags don't pop at edges).
pub fn project(eye: Vec3, dir: Vec3, fov: f32, rect: Rect, point: Vec3) -> Option<Pos2> {
    if !point.is_finite() || rect.width() <= 0.0 || rect.height() <= 0.0 { return None; }
    let view = Mat4::look_to_rh(eye, dir, Vec3::Y);
    let proj = Mat4::perspective_rh(fov.to_radians(), rect.aspect_ratio(), 0.14, 10_000.0);
    let clip = proj * view * Vec4::new(point.x, point.y, point.z, 1.0);
    if clip.w <= 0.2 { return None; }
    let ndc = clip.truncate() / clip.w;
    if ndc.x.abs() > 1.05 || ndc.y.abs() > 1.05 { return None; }
    Some(Pos2::new(rect.left() + (ndc.x + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - ndc.y) * 0.5 * rect.height()))
}

/// 1 inside the range, fading linearly to 0 over the last `FADE_BAND`.
pub fn fade(distance: f32, range: f32) -> f32 {
    if !distance.is_finite() || distance > range { return 0.0; }
    ((range - distance) / (range * FADE_BAND)).clamp(0.0, 1.0)
}

/// Name tag colour and opacity for `target` as seen by `viewer`, before the
/// on-screen and line-of-sight checks. Colour is relative to the viewer:
/// teammates blue, enemies red, whichever side the viewer plays.
pub fn name_tag(viewer: &Player, viewer_idx: usize, target: &Player, target_idx: usize, eye: Vec3) -> Option<(Color32, f32)> {
    if viewer_idx == target_idx || !target.alive || target.name.is_empty() { return None; }
    let friendly = target.team == viewer.team;
    let range = if friendly { TEAMMATE_TAG_RANGE } else { ENEMY_TAG_RANGE };
    let alpha = fade(eye.distance(head(target)), range);
    (alpha > 0.0).then_some((if friendly { TEAMMATE_NAME } else { ENEMY_NAME }, alpha))
}

fn head(p: &Player) -> Vec3 { p.pos + Vec3::Y * 2.35 }

fn with_alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (c.a() as f32 * a.clamp(0.0, 1.0)) as u8)
}

fn health_color(fraction: f32) -> Color32 {
    if fraction > 0.6 { Color32::from_rgb(96, 214, 120) } else if fraction > 0.3 { Color32::from_rgb(240, 190, 70) } else { Color32::from_rgb(236, 82, 60) }
}

/// Client-only memory for the hit flash: last seen shield + health per object.
#[derive(Default)]
pub struct OverlayState { last: Vec<f32>, flash: Vec<f32> }

impl OverlayState {
    fn note(&mut self, states: &[State], dt: f32) {
        if self.last.len() != states.len() { self.last = states.iter().map(|s| s.health + s.shield).collect(); self.flash = vec![0.0; states.len()]; }
        for (i, s) in states.iter().enumerate() {
            let total = s.health + s.shield;
            self.flash[i] = if total < self.last[i] - 0.01 { 0.25 } else { (self.flash[i] - dt).max(0.0) };
            self.last[i] = total;
        }
    }
}

pub fn draw(ui: &egui::Ui, world: &World, state: &mut OverlayState, dt: f32) {
    let rect = ui.max_rect();
    let (eye, dir, fov) = crate::drawlist::view_camera(world);
    let painter = ui.painter();
    let defs = peakrunner_core::equipment::definitions(world.map);
    state.note(&world.equipment, dt);
    for (i, (d, s)) in defs.iter().zip(&world.equipment).enumerate() {
        if !matches!(d.kind, Kind::Generator | Kind::Turret | Kind::Sensor) { continue; }
        let anchor = d.pos() + Vec3::Y * (d.radius + 1.1);
        let distance = eye.distance(anchor);
        let alpha = fade(distance, HIT_BAR_RANGE);
        if alpha <= 0.0 { continue; }
        let Some(at) = project(eye, dir, fov, rect, anchor) else { continue; };
        if !world.sight_clear(eye, anchor) { continue; }
        let width = (80.0 - distance * 0.25).clamp(48.0, 80.0);
        let flash = state.flash.get(i).copied().unwrap_or(0.0);
        draw_bar(painter, at, width, d, s, alpha, flash, distance < 45.0);
    }
    let Some(viewer) = world.players.get(world.player_id) else { return; };
    for (i, p) in world.players.iter().enumerate() {
        let Some((color, alpha)) = name_tag(viewer, world.player_id, p, i, eye) else { continue; };
        let top = head(p);
        let Some(at) = project(eye, dir, fov, rect, top) else { continue; };
        if !world.sight_clear(eye, top) { continue; }
        let font = FontId::proportional(14.0);
        painter.text(at + Vec2::new(1.0, 1.0), Align2::CENTER_BOTTOM, &p.name, font.clone(), with_alpha(Color32::BLACK, alpha * 0.8));
        painter.text(at, Align2::CENTER_BOTTOM, &p.name, font, with_alpha(color, alpha));
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_bar(painter: &egui::Painter, at: Pos2, width: f32, d: &peakrunner_core::equipment::Definition, s: &State, alpha: f32, flash: f32, label: bool) {
    let team = if d.team == 0 { Color32::from_rgb(237, 91, 57) } else { Color32::from_rgb(54, 206, 226) };
    let max_shield = d.max_shield();
    let hull = (s.health / d.max_health()).clamp(0.0, 1.0);
    let x0 = at.x - width * 0.5;
    let mut y = at.y;
    let frame = |y: f32, h: f32| Rect::from_min_size(Pos2::new(x0 - 1.0, y - 1.0), Vec2::new(width + 2.0, h + 2.0));
    if max_shield > 0.0 {
        let r = frame(y, 4.0);
        painter.rect_filled(r, 1.0, with_alpha(Color32::from_black_alpha(170), alpha));
        if s.powered && s.health > 0.0 {
            let f = (s.shield / max_shield).clamp(0.0, 1.0);
            painter.rect_filled(Rect::from_min_size(Pos2::new(x0, y), Vec2::new(width * f, 4.0)), 0.0, with_alpha(SHIELD, alpha));
        } else {
            for k in 0..((width / 6.0) as i32) {
                let x = x0 + k as f32 * 6.0;
                painter.line_segment([Pos2::new(x, y + 2.0), Pos2::new(x + 3.0, y + 2.0)], Stroke::new(1.5, with_alpha(OFFLINE, alpha)));
            }
        }
        y += 6.0;
    }
    let r = frame(y, 7.0);
    painter.rect_filled(r, 1.0, with_alpha(Color32::from_black_alpha(170), alpha));
    painter.rect_stroke(r, 1.0, Stroke::new(1.0, with_alpha(team, alpha)), egui::StrokeKind::Outside);
    painter.rect_filled(Rect::from_min_size(Pos2::new(x0, y), Vec2::new(width * hull, 7.0)), 0.0, with_alpha(health_color(hull), alpha));
    if flash > 0.0 {
        painter.rect_filled(frame(at.y, y + 7.0 - at.y), 1.0, with_alpha(Color32::WHITE, alpha * flash * 2.4));
    }
    let status = if s.health <= 0.0 { Some(("DESTROYED", Color32::from_rgb(236, 82, 60))) }
        else if !s.powered { Some(("OFFLINE", OFFLINE)) } else { None };
    let font = FontId::proportional(11.0);
    if let Some((text, color)) = status {
        painter.text(Pos2::new(at.x, y + 10.0), Align2::CENTER_TOP, text, font.clone(), with_alpha(color, alpha));
    }
    if label {
        let name = match d.kind { Kind::Generator => "GENERATOR", Kind::Turret => "TURRET", _ => "SENSOR" };
        painter.text(Pos2::new(at.x, at.y - 3.0), Align2::CENTER_BOTTOM, name, font, with_alpha(Color32::from_gray(210), alpha * 0.9));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use peakrunner_core::sim::{Team, World};
    use peakrunner_core::terrain::MapId;

    fn players() -> (World, usize, usize, usize) {
        let mut w = World::new(); w.set_map(MapId::Raindance); w.start_rift(true);
        let base = w.players[0].clone();
        w.players.truncate(1);
        let mut friend = base.clone(); friend.name = "Friend".into(); friend.team = base.team;
        let mut foe = base.clone(); foe.name = "Foe".into(); foe.team = if base.team == Team::Ember { Team::Glacier } else { Team::Ember };
        w.players[0].name = "Me".into();
        w.players.push(friend); w.players.push(foe);
        (w, 0, 1, 2)
    }

    #[test]
    fn tags_are_coloured_by_relation_to_the_viewer() {
        let (mut w, me, friend, foe) = players();
        let eye = w.players[me].pos;
        for p in [friend, foe] { w.players[p].pos = eye + Vec3::new(20.0, -2.35, 0.0); }
        assert_eq!(name_tag(&w.players[me], me, &w.players[friend], friend, eye).map(|t| t.0), Some(TEAMMATE_NAME));
        assert_eq!(name_tag(&w.players[me], me, &w.players[foe], foe, eye).map(|t| t.0), Some(ENEMY_NAME));
        // Switching the viewer's side swaps the colours: red always means enemy.
        let mut viewer = w.players[me].clone(); viewer.team = w.players[foe].team;
        assert_eq!(name_tag(&viewer, me, &w.players[foe], foe, eye).map(|t| t.0), Some(TEAMMATE_NAME));
        assert_eq!(name_tag(&viewer, me, &w.players[friend], friend, eye).map(|t| t.0), Some(ENEMY_NAME));
    }

    #[test]
    fn tags_skip_self_the_dead_and_the_out_of_range() {
        let (mut w, me, friend, foe) = players();
        let eye = w.players[me].pos;
        assert!(name_tag(&w.players[me], me, &w.players[me], me, eye).is_none(), "no tag on yourself");
        w.players[friend].pos = eye + Vec3::X * (TEAMMATE_TAG_RANGE - 20.0) - Vec3::Y * 2.35;
        w.players[foe].pos = eye + Vec3::X * (TEAMMATE_TAG_RANGE - 20.0) - Vec3::Y * 2.35;
        assert!(name_tag(&w.players[me], me, &w.players[friend], friend, eye).is_some(), "teammates show further out");
        assert!(name_tag(&w.players[me], me, &w.players[foe], foe, eye).is_none(), "enemies beyond their shorter range");
        w.players[friend].alive = false;
        assert!(name_tag(&w.players[me], me, &w.players[friend], friend, eye).is_none(), "no tags on the dead");
        let edge = fade(ENEMY_TAG_RANGE - 1.0, ENEMY_TAG_RANGE);
        assert!(edge > 0.0 && edge < 0.2, "tags fade near the range limit");
        assert_eq!(fade(10.0, ENEMY_TAG_RANGE), 1.0);
    }

    #[test]
    fn projection_rejects_points_behind_or_off_screen() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0));
        let eye = Vec3::ZERO; let dir = -Vec3::Z;
        let centre = project(eye, dir, 70.0, rect, -Vec3::Z * 50.0).unwrap();
        assert!((centre - rect.center()).length() < 0.5);
        assert!(project(eye, dir, 70.0, rect, Vec3::Z * 50.0).is_none(), "behind the camera");
        assert!(project(eye, dir, 70.0, rect, Vec3::new(500.0, 0.0, -50.0)).is_none(), "off the side of the screen");
        assert!(project(eye, dir, 70.0, rect, Vec3::new(10.0, 0.0, -50.0)).unwrap().x > centre.x, "right is right");
    }

    #[test]
    fn walls_hide_tags_and_bars() {
        // Line of sight runs through the same map query the server uses.
        let mut w = World::new(); w.set_map(MapId::Raindance);
        let top = peakrunner_core::terrain::height_on(MapId::Raindance, 1024.0, 1024.0) + 200.0;
        assert!(w.sight_clear(Vec3::new(1024.0, top, 1000.0), Vec3::new(1024.0, top, 1050.0)));
        assert!(!w.sight_clear(Vec3::new(1024.0, top, 1024.0), Vec3::new(1024.0, top - 400.0, 1024.0)), "the ground blocks sight");
    }
}
