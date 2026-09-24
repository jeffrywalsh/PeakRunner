//! World-anchored HUD: hit bars over destructible equipment, name tags and red
//! arrows over players, and a long-range marker over each flag carrier.
//! Line-of-sight checks here are cosmetic: every position shown already arrives
//! in snapshots, so hiding a tag behind a wall reveals nothing. The carrier
//! marker ignores terrain on purpose: the edge-of-screen flag bearing already
//! gives every flag's position through walls.
use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use glam::{Mat4, Vec3, Vec4};
use peakrunner_core::equipment::{Kind, State};
use peakrunner_core::sim::{Player, World};

pub const HIT_BAR_RANGE: f32 = 120.0;
pub const TEAMMATE_TAG_RANGE: f32 = 150.0;
pub const ENEMY_TAG_RANGE: f32 = 80.0;
/// Red arrows over enemies reach further than their names, but still need a
/// clear line of sight.
pub const ENEMY_ARROW_RANGE: f32 = 250.0;
/// Flag carriers are marked across any current map (the longest flag-to-flag
/// distance is 816 m), through terrain.
pub const CARRIER_MARKER_RANGE: f32 = 1500.0;
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Marker {
    /// Carrying a flag: `flag` is that flag's team; `ally` is relative to the viewer.
    Carrier { flag: peakrunner_core::sim::Team, ally: bool, alpha: f32 },
    /// An enemy in range and in sight; `tag` is the name tag opacity (0 = no name).
    Enemy { alpha: f32, tag: f32 },
    Teammate { tag: f32 },
}

/// What to draw over player `i` and where, as seen from `eye`. Enemies need to
/// be within ENEMY_ARROW_RANGE, on screen and in line of sight; a flag carrier
/// only needs to be on screen and within CARRIER_MARKER_RANGE, and its marker
/// replaces the arrow and name tag.
pub fn visible_marker(world: &World, eye: Vec3, dir: Vec3, fov: f32, rect: Rect, i: usize) -> Option<(Marker, Pos2)> {
    let viewer = world.players.get(world.player_id)?;
    let p = world.players.get(i)?;
    if i == world.player_id || !p.alive { return None; }
    let top = head(p);
    let distance = eye.distance(top);
    if let Some(flag) = world.flags.iter().find(|f| f.carrier == Some(i)) {
        let alpha = fade(distance, CARRIER_MARKER_RANGE);
        if alpha <= 0.0 { return None; }
        let at = project(eye, dir, fov, rect, top)?;
        return Some((Marker::Carrier { flag: flag.team, ally: p.team == viewer.team, alpha }, at));
    }
    let tag = name_tag(viewer, world.player_id, p, i, eye).map_or(0.0, |t| t.1);
    let marker = if p.team == viewer.team {
        if tag <= 0.0 { return None; }
        Marker::Teammate { tag }
    } else {
        let alpha = fade(distance, ENEMY_ARROW_RANGE);
        if alpha <= 0.0 { return None; }
        Marker::Enemy { alpha, tag }
    };
    let at = project(eye, dir, fov, rect, top)?;
    if !world.sight_clear(eye, top) { return None; }
    Some((marker, at))
}

fn team_color(team: peakrunner_core::sim::Team) -> Color32 {
    if team == peakrunner_core::sim::Team::Ember { Color32::from_rgb(237, 91, 57) } else { Color32::from_rgb(54, 206, 226) }
}

/// A downward chevron whose tip sits at `tip`.
fn chevron(painter: &egui::Painter, tip: Pos2, half: f32, fill: Color32, alpha: f32) {
    let h = half * 1.25;
    let pts = vec![tip, Pos2::new(tip.x + half, tip.y - h), Pos2::new(tip.x, tip.y - h * 0.55), Pos2::new(tip.x - half, tip.y - h)];
    painter.add(egui::Shape::convex_polygon(vec![pts[0], pts[1], pts[2]], with_alpha(fill, alpha), Stroke::NONE));
    painter.add(egui::Shape::convex_polygon(vec![pts[0], pts[2], pts[3]], with_alpha(fill, alpha), Stroke::NONE));
    painter.add(egui::Shape::closed_line(pts, Stroke::new(1.2, with_alpha(Color32::BLACK, alpha * 0.85))));
}

fn outlined(painter: &egui::Painter, at: Pos2, text: &str, size: f32, color: Color32, alpha: f32) {
    let font = FontId::proportional(size);
    painter.text(at + Vec2::new(1.0, 1.0), Align2::CENTER_BOTTOM, text, font.clone(), with_alpha(Color32::BLACK, alpha * 0.85));
    painter.text(at, Align2::CENTER_BOTTOM, text, font, with_alpha(color, alpha));
}

fn draw_player(painter: &egui::Painter, p: &Player, marker: Marker, at: Pos2, distance: f32) {
    match marker {
        Marker::Teammate { tag } => outlined(painter, at, &p.name, 14.0, TEAMMATE_NAME, tag),
        Marker::Enemy { alpha, tag } => {
            let half = (9.0 - distance * 0.014).clamp(5.5, 9.0);
            let tip = at + Vec2::new(0.0, -2.0);
            chevron(painter, tip, half, ENEMY_NAME, alpha);
            if tag > 0.0 { outlined(painter, tip - Vec2::new(0.0, half * 1.25 + 3.0), &p.name, 14.0, ENEMY_NAME, tag.min(alpha)); }
        }
        Marker::Carrier { flag, ally, alpha } => {
            let tip = at + Vec2::new(0.0, -2.0);
            chevron(painter, tip, 14.0, team_color(flag), alpha);
            // Pennant above the chevron, in the carried flag's colour.
            let base = tip - Vec2::new(0.0, 22.0);
            painter.line_segment([base, base - Vec2::new(0.0, 18.0)], Stroke::new(2.0, with_alpha(Color32::from_gray(230), alpha)));
            painter.add(egui::Shape::convex_polygon(vec![base - Vec2::new(0.0, 18.0), base - Vec2::new(-13.0, 13.0), base - Vec2::new(0.0, 8.0)],
                with_alpha(team_color(flag), alpha), Stroke::new(1.0, with_alpha(Color32::BLACK, alpha))));
            let name_color = if ally { TEAMMATE_NAME } else { ENEMY_NAME };
            let caption = if ally { "FLAG CARRIER" } else { "HAS YOUR FLAG" };
            let label = base - Vec2::new(0.0, 21.0);
            let detail = if distance > 60.0 { format!("{caption} · {distance:.0} m") } else { caption.to_string() };
            outlined(painter, label, &detail, 12.0, Color32::from_gray(225), alpha);
            outlined(painter, label - Vec2::new(0.0, 14.0), &p.name, 18.0, name_color, alpha);
        }
    }
}

/// Top of the kit model above its entity point (generator cap, sensor vane,
/// turret head), so the ceiling probe starts clear of the model itself.
fn model_top(kind: Kind) -> f32 {
    match kind { Kind::Generator => 3.3, Kind::Sensor => 2.1, _ => 1.6 }
}

/// Where a hit bar hangs: above the equipment when there is open air over it.
/// Under a low ceiling (a generator in a basement) a bar squeezed between the
/// model and the ceiling would poke into the floor above or be hidden behind
/// the model from anywhere a player stands, so within CEILING_ROOM of the
/// usual spot it hangs beside the equipment on the viewer's side instead.
pub fn bar_anchor(map: peakrunner_core::terrain::MapId, d: &peakrunner_core::equipment::Definition, eye: Vec3) -> Vec3 {
    const CEILING_ROOM: f32 = 1.0;
    let base = d.pos();
    let lift = d.radius + 1.1;
    let top = model_top(d.kind).min(lift - 0.2) + 0.05;
    let span = lift + CEILING_ROOM - top;
    let from = base + Vec3::Y * top;
    let ceiling = peakrunner_core::map_pack::on(map)
        .and_then(|pack| pack.sweep(from, from + Vec3::Y * span, 0.0));
    match ceiling {
        None => base + Vec3::Y * lift,
        Some(_) => {
            let side = (eye - base).with_y(0.0).normalize_or(Vec3::X);
            base + side * (d.radius + 0.4) + Vec3::Y * 1.0
        }
    }
}

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
        let anchor = bar_anchor(world.map, d, eye);
        let distance = eye.distance(anchor);
        let alpha = fade(distance, HIT_BAR_RANGE);
        if alpha <= 0.0 { continue; }
        let Some(at) = project(eye, dir, fov, rect, anchor) else { continue; };
        if !world.sight_clear(eye, anchor) { continue; }
        let width = (80.0 - distance * 0.25).clamp(48.0, 80.0);
        let flash = state.flash.get(i).copied().unwrap_or(0.0);
        draw_bar(painter, at, width, d, s, alpha, flash, distance < 45.0);
    }
    // Carriers last, so their larger marker draws on top.
    let mut order: Vec<usize> = (0..world.players.len()).collect();
    order.sort_by_key(|&i| world.flags.iter().any(|f| f.carrier == Some(i)));
    for i in order {
        let Some((marker, at)) = visible_marker(world, eye, dir, fov, rect, i) else { continue; };
        let p = &world.players[i];
        draw_player(painter, p, marker, at, eye.distance(head(p)));
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

    /// Generator bars in basements: visible from inside the generator room,
    /// never from the floor above. Tower Complex's keel level and Dustreach's
    /// cistern have a low ceiling over the generator, so their bars hang
    /// beside it.
    #[test]
    fn basement_bars_stay_in_their_room() {
        // (map, a viewpoint in the room, a viewpoint on the floor above), in
        // base-local coordinates (team 1 is unrotated; team 0 is turned 180).
        let cases = [(MapId::StonehengeClone, Vec3::new(13.2, -1.65, -3.0), Vec3::new(6.8, 1.7, 7.7)),
                     (MapId::DesertOfDeathClone, Vec3::new(-12.4, -0.2, -15.4), Vec3::new(0.0, 6.7, -5.0)),
                     (MapId::BroadsideClone, Vec3::new(0.5, -6.3, -6.8), Vec3::new(-5.2, 1.7, -5.2))];
        for (map, inside, above) in cases {
            let info = peakrunner_core::terrain::info(map);
            let mut w = World::new(); w.set_map(map);
            for d in peakrunner_core::equipment::definitions(map).iter().filter(|d| d.kind == Kind::Generator) {
                let home = if d.team == 0 { info.ember } else { info.glacier };
                let s = if d.team == 0 { -1.0 } else { 1.0 };
                let world = |l: Vec3| Vec3::new(home.x + s * l.x, home.y + l.y, home.z + s * l.z);
                let (inside, above) = (world(inside), world(above));
                let seen = bar_anchor(map, d, inside);
                assert!(w.sight_clear(inside, seen), "{map:?} generator {}: bar {seen} hidden from its room", d.id);
                let from_above = bar_anchor(map, d, above);
                assert!(!w.sight_clear(above, from_above), "{map:?} generator {}: bar {from_above} visible from above", d.id);
            }
        }
    }


    /// Viewer high in open sky over the middle of the map, looking at `target`.
    fn sky_view(w: &mut World, me: usize, target: Vec3) -> (Vec3, Vec3, Rect) {
        let ground = peakrunner_core::terrain::height_on(MapId::Raindance, 1024.0, 1024.0);
        let eye = Vec3::new(1024.0, ground + 220.0, 1024.0);
        w.players[me].pos = eye - Vec3::Y * 2.35;
        (eye, (target - eye).normalize(), Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0)))
    }

    #[test]
    fn enemy_arrows_need_range_screen_and_sight() {
        let (mut w, me, friend, foe) = players();
        let ground = peakrunner_core::terrain::height_on(MapId::Raindance, 1024.0, 1024.0);
        let place = |w: &mut World, i: usize, d: f32| { w.players[i].pos = Vec3::new(1024.0 + d, ground + 220.0, 1024.0) - Vec3::Y * 2.35; };
        place(&mut w, foe, 200.0);
        let target = w.players[foe].pos + Vec3::Y * 2.35;
        let (eye, dir, rect) = sky_view(&mut w, me, target);
        match visible_marker(&w, eye, dir, 70.0, rect, foe) {
            Some((Marker::Enemy { alpha, tag }, _)) => { assert_eq!(alpha, 1.0); assert_eq!(tag, 0.0, "no name beyond {ENEMY_TAG_RANGE} m"); }
            other => panic!("expected a red arrow at 200 m, got {other:?}"),
        }
        place(&mut w, foe, 40.0);
        assert!(matches!(visible_marker(&w, eye, dir, 70.0, rect, foe), Some((Marker::Enemy { tag, .. }, _)) if tag == 1.0), "close enemies get arrow and name");
        place(&mut w, foe, ENEMY_ARROW_RANGE + 5.0);
        assert!(visible_marker(&w, eye, dir, 70.0, rect, foe).is_none(), "no arrow beyond {ENEMY_ARROW_RANGE} m");
        place(&mut w, foe, 100.0);
        assert!(visible_marker(&w, eye, -dir, 70.0, rect, foe).is_none(), "no arrow off screen");
        w.players[foe].alive = false;
        assert!(visible_marker(&w, eye, dir, 70.0, rect, foe).is_none(), "no arrow on the dead");
        w.players[foe].alive = true;
        // Inside the hill below the viewer: terrain blocks the sight line.
        w.players[foe].pos = Vec3::new(1034.0, ground - 40.0, 1024.0);
        let down = (w.players[foe].pos - eye).normalize();
        assert!(visible_marker(&w, eye, down, 70.0, rect, foe).is_none(), "never through terrain");
        assert!(visible_marker(&w, eye, dir, 70.0, rect, me).is_none(), "nothing over yourself");
        place(&mut w, friend, 40.0);
        assert!(matches!(visible_marker(&w, eye, dir, 70.0, rect, friend), Some((Marker::Teammate { .. }, _))), "teammates get a blue name, never a red arrow");
        place(&mut w, friend, 200.0);
        assert!(visible_marker(&w, eye, dir, 70.0, rect, friend).is_none(), "teammates have no arrow past their name range");
    }

    #[test]
    fn flag_carriers_are_marked_far_and_through_terrain() {
        let (mut w, me, friend, foe) = players();
        let ground = peakrunner_core::terrain::height_on(MapId::Raindance, 1024.0, 1024.0);
        let ours = w.flags.iter().position(|f| f.team == w.players[me].team).unwrap();
        w.flags[ours].carrier = Some(foe);
        // Buried in the hill, far below the viewer: no arrow could show, the carrier marker does.
        w.players[foe].pos = Vec3::new(1034.0, ground - 40.0, 1024.0);
        let target = w.players[foe].pos + Vec3::Y * 2.35;
        let (eye, dir, rect) = sky_view(&mut w, me, target);
        match visible_marker(&w, eye, dir, 70.0, rect, foe) {
            Some((Marker::Carrier { flag, ally, alpha }, _)) => { assert_eq!(flag, w.players[me].team); assert!(!ally); assert_eq!(alpha, 1.0); }
            other => panic!("expected the carrier marker through terrain, got {other:?}"),
        }
        assert!(visible_marker(&w, eye, -dir, 70.0, rect, foe).is_none(), "still only on screen");
        // Far away: beyond any arrow or tag range, inside the carrier range.
        w.players[foe].pos = eye + dir * 700.0;
        assert!(matches!(visible_marker(&w, eye, dir, 70.0, rect, foe), Some((Marker::Carrier { .. }, _))));
        w.players[foe].pos = eye + dir * (CARRIER_MARKER_RANGE + 10.0);
        assert!(visible_marker(&w, eye, dir, 70.0, rect, foe).is_none());
        // Our own carrier: the ally variant, carrying the enemy's flag.
        let theirs = 1 - ours;
        w.flags[theirs].carrier = Some(friend);
        w.players[friend].pos = eye + dir * 300.0;
        assert!(matches!(visible_marker(&w, eye, dir, 70.0, rect, friend), Some((Marker::Carrier { ally: true, flag, .. }, _)) if flag != w.players[me].team));
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
