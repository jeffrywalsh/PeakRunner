//! Original synthesized sound palette, a fixed-voice stereo mixer and the
//! director that turns world state into cues. Everything here is pure and
//! platform-free: `audio.rs` feeds the mixer to rodio natively and to a
//! WebAudio script node in the browser, so both hear the same thing.
//!
//! Every sound is generated from deterministic noise, oscillators and simple
//! filters; nothing is sampled from another game.

use glam::Vec3;
use std::sync::Arc;

use crate::sim::World;

const TAU: f32 = std::f32::consts::TAU;

/// Deterministic white noise in [-1, 1].
#[derive(Clone, Copy)]
struct Rng(u32);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / 16_777_216.0 * 2.0 - 1.0
    }
}

/// One-pole low-pass coefficient for a cutoff in Hz.
fn lp_coef(cutoff: f32, sr: f32) -> f32 { 1.0 - (-TAU * cutoff.clamp(20.0, sr * 0.45) / sr).exp() }

/// Two-pole resonator (band-pass) used for tubes, bells and metal.
#[derive(Clone, Copy, Default)]
struct Reso { a1: f32, a2: f32, y1: f32, y2: f32, g: f32 }
impl Reso {
    fn new(freq: f32, q: f32, sr: f32) -> Self {
        let r = (-std::f32::consts::PI * freq / (q * sr)).exp();
        Reso { a1: 2.0 * r * (TAU * freq / sr).cos(), a2: -r * r, y1: 0.0, y2: 0.0, g: 1.0 - r }
    }
    fn tick(&mut self, x: f32) -> f32 {
        let y = self.g * x + self.a1 * self.y1 + self.a2 * self.y2;
        self.y2 = self.y1; self.y1 = y; y
    }
}

/// Every one-shot cue. Variants give repeated sounds different seeds and
/// pitches so rapid fire never sounds like one sample looping.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cue {
    DiscFire, DiscReady, ChainShot, GrenadeFire, TurretBullet, TurretPlasma,
    BoomNear, BoomFar, GenBlast, ShieldHit, ShieldDown, HullHit, Footstep, Land,
    Switch, RepairKit, Bounce, Hit, Pain, Death, Flag, Drop, Return,
    CaptureWin, CaptureLoss, Start, End,
}

pub const CUES: [Cue; 27] = [
    Cue::DiscFire, Cue::DiscReady, Cue::ChainShot, Cue::GrenadeFire, Cue::TurretBullet,
    Cue::TurretPlasma, Cue::BoomNear, Cue::BoomFar, Cue::GenBlast, Cue::ShieldHit,
    Cue::ShieldDown, Cue::HullHit, Cue::Footstep, Cue::Land, Cue::Switch, Cue::RepairKit,
    Cue::Bounce, Cue::Hit, Cue::Pain, Cue::Death, Cue::Flag, Cue::Drop, Cue::Return,
    Cue::CaptureWin, Cue::CaptureLoss, Cue::Start, Cue::End,
];

impl Cue {
    fn index(self) -> usize { CUES.iter().position(|c| *c == self).unwrap() }
    pub fn variants(self) -> usize {
        match self {
            Cue::ChainShot | Cue::Footstep => 6,
            Cue::DiscFire | Cue::GrenadeFire | Cue::TurretBullet | Cue::BoomNear
            | Cue::HullHit | Cue::Bounce => 3,
            Cue::ShieldHit => 4,
            Cue::TurretPlasma | Cue::BoomFar | Cue::Land => 2,
            _ => 1,
        }
    }
    /// Higher keeps its voice when the pool is full.
    pub fn priority(self) -> u8 {
        match self {
            Cue::CaptureWin | Cue::CaptureLoss | Cue::Flag | Cue::Drop | Cue::Return
            | Cue::Start | Cue::End | Cue::Death => 6,
            Cue::GenBlast | Cue::Hit | Cue::Pain | Cue::RepairKit | Cue::ShieldDown => 5,
            Cue::DiscFire | Cue::GrenadeFire | Cue::DiscReady | Cue::Switch => 4,
            Cue::BoomNear | Cue::BoomFar | Cue::TurretPlasma => 3,
            Cue::ChainShot | Cue::TurretBullet | Cue::ShieldHit | Cue::HullHit | Cue::Land => 2,
            Cue::Footstep | Cue::Bounce => 1,
        }
    }
    /// Distance at which a world sound fades to silence.
    pub fn range(self) -> f32 {
        match self {
            Cue::GenBlast => 420.0,
            Cue::BoomFar => 520.0,
            Cue::BoomNear => 180.0,
            Cue::DiscFire | Cue::GrenadeFire | Cue::TurretPlasma => 140.0,
            Cue::ChainShot | Cue::TurretBullet => 150.0,
            Cue::ShieldHit | Cue::ShieldDown | Cue::HullHit => 110.0,
            Cue::Footstep => 38.0,
            Cue::Land | Cue::Bounce => 60.0,
            _ => 120.0,
        }
    }
    /// Cap on simultaneous copies, so chaingun spam or footsteps can't
    /// crowd out everything else.
    fn max_voices(self) -> usize {
        match self {
            Cue::ChainShot | Cue::TurretBullet => 8,
            Cue::Footstep => 4,
            Cue::ShieldHit | Cue::HullHit | Cue::Bounce => 4,
            Cue::BoomNear | Cue::BoomFar => 6,
            _ => 3,
        }
    }
    /// Sim and network event names (`push_event`, `spatial_sounds`).
    pub fn from_event(name: &str) -> Option<Cue> {
        Some(match name {
            "disc" => Cue::DiscFire,
            "disc_ready" => Cue::DiscReady,
            "chain" => Cue::ChainShot,
            "grenade" => Cue::GrenadeFire,
            "boom" => Cue::BoomNear,
            "hit" => Cue::Hit,
            "pain" => Cue::Pain,
            "death" => Cue::Death,
            "flag" => Cue::Flag,
            "drop" => Cue::Drop,
            "return" => Cue::Return,
            "capture_win" => Cue::CaptureWin,
            "capture_loss" => Cue::CaptureLoss,
            "start" => Cue::Start,
            "end" => Cue::End,
            _ => return None,
        })
    }
}

/// Looping beds whose gain, pitch and pan follow the world every frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Loop { Jet, Ski, Wind, Spin, Hum, DiscHum }
pub const LOOPS: [Loop; 6] = [Loop::Jet, Loop::Ski, Loop::Wind, Loop::Spin, Loop::Hum, Loop::DiscHum];

// ---------------------------------------------------------------- synthesis

/// Attack/release so every clip starts and ends at silence.
fn edges(data: &mut [f32], sr: f32, attack: f32, release: f32) {
    let n = data.len();
    let a = (sr * attack).max(1.0);
    let r = (sr * release).max(1.0);
    for (i, s) in data.iter_mut().enumerate() {
        let from_end = (n - 1 - i) as f32;
        *s *= (i as f32 / a).min(1.0) * (from_end / r).min(1.0);
    }
    if let Some(first) = data.first_mut() { *first = 0.0; }
}

fn normalize(data: &mut [f32], peak: f32) {
    let max = data.iter().fold(0.0_f32, |m, s| m.max(s.abs()));
    if max > 1e-6 { for s in data.iter_mut() { *s *= peak / max; } }
}

fn render(sr: f32, seconds: f32, mut f: impl FnMut(f32) -> f32) -> Vec<f32> {
    (0..(sr * seconds) as usize).map(|i| f(i as f32 / sr)).collect()
}

/// A metallic FM bell tone used by shields, chimes and flag cues.
fn bell(t: f32, freq: f32, ratio: f32, index: f32, decay: f32) -> f32 {
    let m = (TAU * freq * ratio * t).sin() * index * (-t * decay * 1.6).exp();
    (TAU * freq * t + m).sin() * (-t * decay).exp()
}

fn chime(sr: f32, notes: &[(f32, f32)], seconds: f32, peak: f32) -> Vec<f32> {
    let mut d = render(sr, seconds, |t| {
        notes.iter().map(|&(f, start)| {
            let a = t - start;
            if a < 0.0 { 0.0 } else { bell(a, f, 2.0, 1.4, 5.0) * (a / 0.004).min(1.0) }
        }).sum()
    });
    normalize(&mut d, peak);
    edges(&mut d, sr, 0.002, 0.03);
    d
}

/// Deterministic mono PCM for one variant of a cue.
pub fn synth(cue: Cue, variant: usize, sr: f32) -> Vec<f32> {
    let v = variant as f32;
    let mut n = Rng(0x5EED_0000 ^ ((cue.index() as u32) << 8) ^ variant as u32 * 7919);
    let mut data = match cue {
        Cue::DiscFire => {
            let detune = 1.0 + (v - 1.0) * 0.035;
            let (mut lp, mut phase) = (0.0, 0.0);
            let c = lp_coef(6500.0, sr);
            render(sr, 0.55, |t| {
                let x = n.next();
                lp += (x - lp) * c;
                phase += TAU * (140.0 + 640.0 * (-t * 18.0).exp()) * detune / sr;
                let click = x * (-t * 170.0).exp() * 0.25;
                let body = (TAU * 82.0 * detune * t).sin() * (-t * 15.0).exp() * 0.36;
                let sub = (TAU * (58.0 - 20.0 * t) * t).sin() * (-t * 9.0).exp() * 0.22;
                let whirr = (phase.sin() + (phase * 2.03).sin() * 0.3) * (0.78 + 0.22 * (TAU * 43.0 * t).sin());
                click + body + sub + lp * (-t * 20.0).exp() * 0.28 + whirr * (-t * 8.0).exp() * 0.17
            })
        }
        Cue::DiscReady => {
            let (mut lp, mut phase) = (0.0, 0.0);
            let c = lp_coef(6500.0, sr);
            render(sr, 0.2, |t| {
                let x = n.next();
                lp += (x - lp) * c;
                phase += TAU * (480.0 + 500.0 * t) / sr;
                let latch = if t > 0.055 { lp * (-(t - 0.055) * 90.0).exp() * 0.2 } else { 0.0 };
                x * (-t * 180.0).exp() * 0.2 + latch + phase.sin() * (-t * 30.0).exp() * 0.07
            })
        }
        Cue::ChainShot | Cue::TurretBullet => {
            let turret = cue == Cue::TurretBullet;
            let body_f = if turret { 118.0 } else { 168.0 } * (1.0 + (v - 2.5) * 0.025);
            let mut hp = 0.0;
            let c = lp_coef(2400.0, sr);
            let mut rings = [Reso::new(2150.0 + v * 37.0, 18.0, sr), Reso::new(3330.0 - v * 23.0, 22.0, sr),
                Reso::new(4720.0, 26.0, sr)];
            render(sr, if turret { 0.16 } else { 0.12 }, |t| {
                let x = n.next();
                hp += (x - hp) * c;
                let crack = (x - hp) * (-t * 115.0).exp() * 0.42;
                let body = (TAU * body_f * t * (1.0 + 0.4 * (-t * 60.0).exp())).sin() * (-t * 42.0).exp() * 0.22;
                let thump = (TAU * 88.0 * t).sin() * (-t * 38.0).exp() * if turret { 0.3 } else { 0.18 };
                let ring: f32 = rings.iter_mut().map(|r| r.tick(x * (-t * 400.0).exp())).sum::<f32>() * 0.9;
                crack + body + thump + ring * (-t * 45.0).exp()
            })
        }
        Cue::GrenadeFire => {
            let pitch = 1.0 + (v - 1.0) * 0.04;
            let mut tube = Reso::new(410.0 * pitch, 7.0, sr);
            let mut clack = Reso::new(1900.0, 14.0, sr);
            render(sr, 0.42, |t| {
                let x = n.next();
                let thoonk = (TAU * (140.0 * pitch - 150.0 * t) * t).sin() * (-t * 13.0).exp() * 0.5;
                let bore = tube.tick(x) * (-t * 17.0).exp() * 1.6;
                let latch = if t > 0.2 { clack.tick(x * (-(t - 0.2) * 300.0).exp()) * 1.2 } else { 0.0 };
                thoonk + bore + x * (-t * 160.0).exp() * 0.2 + latch
            })
        }
        Cue::TurretPlasma => {
            let mut phase = 0.0;
            let base = 210.0 + v * 25.0;
            render(sr, 0.5, |t| {
                let x = n.next();
                phase += TAU * (base + 520.0 * (1.0 - (-t * 12.0).exp())) / sr;
                let index = 3.2 * (-t * 7.0).exp();
                let zap = (phase + index * (phase * 1.5).sin()).sin() * (-t * 6.0).exp() * 0.35;
                zap + (TAU * 110.0 * t).sin() * (-t * 5.0).exp() * 0.18 + x * (-t * 40.0).exp() * 0.12
            })
        }
        Cue::BoomNear => {
            let mut lp = 0.0;
            let mut debris = Rng(0xDEB2 ^ variant as u32);
            let drop = 1.0 + (v - 1.0) * 0.06;
            render(sr, 1.45, |t| {
                let x = n.next();
                let cut = lp_coef(3200.0 * (-t * 2.4).exp() + 140.0, sr);
                lp += (x - lp) * cut;
                let crack = x * (-t * 75.0).exp() * 0.55;
                let body = (TAU * (95.0 * drop - 70.0 * t.min(0.8)) * t).sin() * (-t * 5.0).exp() * 0.5;
                let rumble = lp * (-t * 2.8).exp() * 1.4;
                let grit = if t > 0.12 && t < 1.1 && debris.next() > 0.9985 - t * 0.0006 { 0.25 * (1.1 - t) } else { 0.0 };
                crack + body + rumble + grit
            })
        }
        Cue::BoomFar => {
            let mut lp = 0.0;
            let c = lp_coef(260.0 + v * 40.0, sr);
            render(sr, 2.3, |t| {
                lp += (n.next() - lp) * c;
                let roll = (-t * 1.9).exp() + 0.45 * (-(t - 0.38).max(0.0) * 3.0).exp() * (t > 0.38) as u8 as f32
                    + 0.25 * (-(t - 0.85).max(0.0) * 3.0).exp() * (t > 0.85) as u8 as f32;
                let thud = (TAU * 42.0 * t).sin() * (-t * 4.0).exp() * 0.15;
                lp * roll * 3.0 + thud
            })
        }
        Cue::GenBlast => {
            let mut lp = 0.0;
            let mut groan = [Reso::new(170.0, 30.0, sr), Reso::new(233.0, 34.0, sr)];
            let mut arc = Rng(0xA2C);
            let mut arc_on = 0.0_f32;
            render(sr, 3.0, |t| {
                let x = n.next();
                lp += (x - lp) * lp_coef(1800.0 * (-t * 1.6).exp() + 90.0, sr);
                let sub = (TAU * (62.0 - 38.0 * t.min(1.2)) * t).sin() * (-t * 1.4).exp() * 0.55;
                let crack = x * (-t * 45.0).exp() * 0.5;
                let metal: f32 = groan.iter_mut().map(|r| r.tick(x * 0.02)).sum::<f32>() * (-t * 1.1).exp() * 9.0;
                if arc.next() > 0.9992 && t < 1.4 { arc_on = 0.04; }
                arc_on = (arc_on - 1.0 / sr).max(0.0);
                let zap = if arc_on > 0.0 { (TAU * 1320.0 * t).sin().signum() * 0.12 } else { 0.0 };
                sub + crack + lp * (-t * 1.2).exp() * 1.5 + metal + zap
            })
        }
        Cue::ShieldHit => {
            let f = 1380.0 * (1.0 + v * 0.07);
            render(sr, 0.32, |t| {
                let shimmer = 1.0 + 0.25 * (TAU * 31.0 * t).sin();
                bell(t, f, 1.41, 3.0, 13.0) * shimmer * 0.4 + n.next() * (-t * 260.0).exp() * 0.2
            })
        }
        Cue::ShieldDown => {
            let mut phase = 0.0;
            let mut crackle = Rng(0xC7AC);
            render(sr, 1.0, |t| {
                phase += TAU * (1250.0 * (-t * 2.4).exp() + 110.0) / sr;
                let sweep = phase.sin() + 0.4 * (phase * 2.0).sin() + 0.2 * (phase * 3.0).sin();
                let spit = if crackle.next() > 0.96 { n.next() * 0.35 } else { 0.0 };
                sweep * 0.3 * (1.0 - t).max(0.0) + spit * (-t * 3.0).exp()
                    + if t > 0.86 { n.next() * (-(t - 0.86) * 60.0).exp() * 0.4 } else { 0.0 }
            })
        }
        Cue::HullHit => {
            let mut r = [Reso::new(380.0 + v * 20.0, 16.0, sr), Reso::new(910.0 - v * 30.0, 20.0, sr), Reso::new(1750.0, 24.0, sr)];
            render(sr, 0.26, |t| {
                let x = n.next() * (-t * 180.0).exp();
                r.iter_mut().map(|r| r.tick(x)).sum::<f32>() * (-t * 16.0).exp() * 2.2 + x * 0.3
            })
        }
        Cue::Footstep => {
            let mut lp = 0.0;
            let c = lp_coef(900.0 + v * 90.0, sr);
            let mut tick = Reso::new(2450.0 + v * 60.0, 20.0, sr);
            let pitch = 72.0 + v * 4.0;
            render(sr, 0.13, |t| {
                let x = n.next();
                lp += (x - lp) * c;
                (TAU * pitch * t).sin() * (-t * 42.0).exp() * 0.45 + lp * (-t * 55.0).exp() * 0.9
                    + tick.tick(x * (-t * 500.0).exp()) * 0.5
            })
        }
        Cue::Land => {
            let mut lp = 0.0;
            let c = lp_coef(700.0, sr);
            let mut clank = [Reso::new(610.0 + v * 40.0, 18.0, sr), Reso::new(1340.0, 22.0, sr)];
            render(sr, 0.38, |t| {
                let x = n.next();
                lp += (x - lp) * c;
                (TAU * (58.0 - 20.0 * t) * t).sin() * (-t * 16.0).exp() * 0.55 + lp * (-t * 22.0).exp() * 1.3
                    + clank.iter_mut().map(|r| r.tick(x * (-t * 220.0).exp())).sum::<f32>() * 0.9
            })
        }
        Cue::Switch => {
            let mut r = [Reso::new(1850.0, 16.0, sr), Reso::new(1350.0, 16.0, sr)];
            render(sr, 0.32, |t| {
                let x = n.next();
                let a = r[0].tick(x * (-t * 400.0).exp());
                let b = if t > 0.14 { r[1].tick(x * (-(t - 0.14) * 400.0).exp()) } else { 0.0 };
                let slide = if t > 0.03 && t < 0.13 { n.next() * 0.04 } else { 0.0 };
                (a + b) * 1.6 + slide
            })
        }
        Cue::RepairKit => {
            let mut hiss = 0.0;
            render(sr, 1.25, |t| {
                let x = n.next();
                hiss += (x - hiss) * 0.2;
                let glide = 380.0 + 820.0 * (t / 1.25);
                let env = (t / 0.08).min(1.0) * (1.0 - t / 1.25).max(0.0);
                let trem = 0.7 + 0.3 * (TAU * 14.0 * t).sin();
                ((TAU * glide * t).sin() * 0.25 + (TAU * glide * 1.5 * t).sin() * 0.12) * env * trem
                    + (x - hiss) * env * 0.08
            })
        }
        Cue::Bounce => {
            let mut r = [Reso::new(1300.0 + v * 110.0, 22.0, sr), Reso::new(2900.0 - v * 90.0, 26.0, sr)];
            render(sr, 0.2, |t| {
                let x = n.next() * (-t * 300.0).exp();
                r.iter_mut().map(|r| r.tick(x)).sum::<f32>() * 2.0 + (TAU * 140.0 * t).sin() * (-t * 50.0).exp() * 0.2
            })
        }
        Cue::Hit => render(sr, 0.07, |t| {
            ((TAU * 1900.0 * t).sin() + 0.5 * (TAU * 2850.0 * t).sin()) * (-t * 55.0).exp() * 0.3
        }),
        Cue::Pain => {
            let mut lp = 0.0;
            render(sr, 0.2, |t| {
                lp += (n.next() - lp) * 0.08;
                (TAU * (130.0 - 60.0 * t) * t).sin() * (-t * 18.0).exp() * 0.4 + lp * (-t * 14.0).exp() * 1.2
            })
        }
        Cue::Death => render(sr, 0.65, |t| {
            let f = 330.0 * (-t * 2.3).exp() + 70.0;
            ((TAU * f * t).sin() * 0.3 + n.next() * 0.08) * (1.0 - t / 0.65).max(0.0)
        }),
        Cue::Flag => chime(sr, &[(659.25, 0.0), (987.77, 0.11)], 0.55, 0.45),
        Cue::Drop => chime(sr, &[(523.25, 0.0), (392.0, 0.12)], 0.45, 0.4),
        Cue::Return => chime(sr, &[(392.0, 0.0), (587.33, 0.1)], 0.5, 0.4),
        Cue::Start => chime(sr, &[(392.0, 0.0), (493.88, 0.1), (587.33, 0.2)], 0.7, 0.4),
        Cue::End => chime(sr, &[(587.33, 0.0), (493.88, 0.14), (392.0, 0.28)], 0.85, 0.4),
        Cue::CaptureWin => capture(sr, true),
        Cue::CaptureLoss => capture(sr, false),
    };
    let peak = match cue {
        Cue::GenBlast => 0.88, Cue::BoomNear => 0.82, Cue::BoomFar => 0.6,
        Cue::DiscFire | Cue::GrenadeFire => 0.6, Cue::ChainShot | Cue::TurretBullet => 0.45,
        Cue::Footstep => 0.3, Cue::Hit => 0.3, Cue::ShieldHit => 0.4,
        _ => 0.5,
    };
    normalize(&mut data, peak);
    edges(&mut data, sr, 0.0015, 0.02);
    data
}

/// Original capture motif: impact, sequenced notes and a sustained chord.
fn capture(sr: f32, victory: bool) -> Vec<f32> {
    let notes = if victory { [261.63, 329.63, 392.0, 523.25] } else { [392.0, 349.23, 311.13, 261.63] };
    let chord = if victory { [261.63, 329.63, 392.0] } else { [130.81, 155.56, 196.0] };
    render(sr, 2.4, |t| {
        let mut s = 0.16 * (TAU * (90.0 * t - 22.0 * t * t)).sin() * (-t * 13.0).exp();
        for (i, f) in notes.iter().enumerate() {
            let a = t - i as f32 * 0.22;
            if a >= 0.0 {
                let p = TAU * f * a;
                s += 0.14 * (a / 0.012).min(1.0) * (-a * 4.0).exp() * (p.sin() + 0.22 * (p * 2.0).sin());
            }
        }
        if t > 0.72 {
            let a = t - 0.72;
            for f in chord { s += 0.045 * (a / 0.08).min(1.0) * (-a * 1.8).exp() * (TAU * f * a).sin(); }
        }
        s
    })
}

/// Seamless mono loops. Tonal parts use whole periods; noise crossfades at the seam.
pub fn synth_loop(kind: Loop, sr: f32) -> Vec<f32> {
    let seconds = match kind { Loop::Wind => 4.0, Loop::Jet | Loop::Ski | Loop::Hum => 2.0, _ => 1.0 };
    let len = (sr * seconds) as usize;
    let overlap = (sr * 0.05) as usize;
    let mut n = Rng(0x100F ^ kind as u32 * 131);
    let (mut a, mut b) = (0.0_f32, 0.0_f32);
    let mut gust = Reso::new(900.0, 6.0, sr);
    let mut data: Vec<f32> = (0..len + overlap).map(|i| {
        let t = (i % len) as f32 / sr;
        let x = n.next();
        match kind {
            Loop::Jet => {
                a += (x - a) * lp_coef(1800.0, sr);
                b += (x - b) * lp_coef(9000.0, sr);
                a * 0.65 + (b - a) * 0.18 + (TAU * 148.0 * t + (TAU * 3.0 * t).sin() * 0.3).sin() * 0.05
            }
            Loop::Ski => {
                a += (x - a) * lp_coef(2600.0, sr);
                b += ((x - a) - b) * lp_coef(9500.0, sr);
                b * (0.75 + 0.25 * (TAU * 19.0 * t).sin())
            }
            Loop::Wind => {
                a += (x - a) * lp_coef(650.0, sr);
                let swell = 0.6 + 0.25 * (TAU * 0.5 * t).sin() + 0.15 * (TAU * 1.25 * t).sin();
                a * swell * 2.0 + gust.tick(x) * 0.05 * swell
            }
            Loop::Spin => {
                let w: f32 = [(90.0, 1.0), (180.0, 0.6), (270.0, 0.35), (360.0, 0.25), (540.0, 0.12)]
                    .iter().map(|&(f, g)| (TAU * f * t).sin() * g).sum();
                w * 0.3 + x * 0.03
            }
            Loop::Hum => {
                let w = (TAU * 55.0 * t).sin() + 0.5 * (TAU * 110.0 * t).sin() + 0.25 * (TAU * 165.0 * t).sin();
                w * 0.3 * (1.0 + 0.03 * (TAU * 6.0 * t).sin()) + x * 0.015
            }
            Loop::DiscHum => {
                let w = (TAU * 240.0 * t).sin() + 0.4 * (TAU * 480.0 * t).sin();
                w * 0.3 * (0.7 + 0.3 * (TAU * 38.0 * t).sin())
            }
        }
    }).collect();
    for i in 0..overlap {
        let blend = i as f32 / overlap as f32;
        data[i] = data[len + i] * (1.0 - blend) + data[i] * blend;
    }
    data.truncate(len);
    normalize(&mut data, 0.5);
    data
}

// -------------------------------------------------------------------- mixer

/// One request for the mixer. `Copy`, so queuing never allocates.
#[derive(Clone, Copy, Debug)]
pub struct Play {
    pub cue: Cue,
    pub variant: usize,
    pub gain: f32,
    /// -1 left .. 1 right.
    pub pan: f32,
    /// Low-pass cutoff in Hz; distance makes sounds duller.
    pub cutoff: f32,
    /// Seconds before the sound starts (sound travel for far explosions).
    pub delay: f32,
    /// Playback rate: small pitch variation.
    pub rate: f32,
}

impl Play {
    pub fn local(cue: Cue, variant: usize) -> Self {
        Play { cue, variant, gain: 1.0, pan: 0.0, cutoff: 18_000.0, delay: 0.0, rate: 1.0 }
    }
}

/// Per-frame targets for every loop, smoothed inside the mixer.
#[derive(Clone, Copy, Debug, Default)]
pub struct LoopTarget { pub gain: f32, pub rate: f32, pub pan: f32, pub cutoff: f32 }

pub const VOICES: usize = 32;
const QUEUE: usize = 96;

#[derive(Clone, Default)]
struct Voice {
    clip: Option<Arc<[f32]>>,
    cue_index: usize,
    priority: u8,
    pos: f32,
    rate: f32,
    gl: f32,
    gr: f32,
    lp_a: f32,
    lp_z: f32,
    delay: u32,
}

#[derive(Clone)]
struct LoopVoice {
    clip: Arc<[f32]>,
    pos: f32,
    gain: f32,
    rate: f32,
    pan: f32,
    lp_a: f32,
    lp_z: f32,
    target: LoopTarget,
}

/// Small Schroeder room: four combs into two all-passes, sized for rooms and
/// tunnels. Buffers are allocated once.
struct Room { combs: [(Vec<f32>, usize); 4], passes: [(Vec<f32>, usize); 2], damp: [f32; 4] }
impl Room {
    fn new(sr: f32) -> Self {
        let buf = |ms: f32| (vec![0.0; (sr * ms / 1000.0) as usize], 0);
        Room { combs: [buf(29.7), buf(37.1), buf(41.1), buf(43.7)], passes: [buf(5.0), buf(1.7)], damp: [0.0; 4] }
    }
    fn tick(&mut self, x: f32) -> f32 {
        let mut y = 0.0;
        for (i, (b, p)) in self.combs.iter_mut().enumerate() {
            let out = b[*p];
            self.damp[i] += (out - self.damp[i]) * 0.45;
            b[*p] = x + self.damp[i] * 0.72;
            *p = (*p + 1) % b.len();
            y += out;
        }
        y *= 0.25;
        for (b, p) in self.passes.iter_mut() {
            let d = b[*p];
            b[*p] = y + d * 0.5;
            *p = (*p + 1) % b.len();
            y = d - y * 0.5;
        }
        y
    }
}

/// Fixed-size stereo mixer. After construction, playing sounds and rendering
/// never allocate: clips are shared `Arc`s and the queue has fixed capacity.
pub struct Mixer {
    pub sr: f32,
    clips: Vec<Vec<Arc<[f32]>>>,
    voices: Vec<Voice>,
    loops: Vec<LoopVoice>,
    ambient: Option<LoopVoice>,
    ambient_target: f32,
    queue: Vec<Play>,
    master: f32,
    master_target: f32,
    room: Room,
    room_level: f32,
    room_target: f32,
    smooth: f32,
}

impl Mixer {
    pub fn new(sr: f32) -> Self {
        let clips = CUES.iter().map(|&c| (0..c.variants()).map(|v| Arc::<[f32]>::from(synth(c, v, sr))).collect()).collect();
        let loops = LOOPS.iter().map(|&l| LoopVoice {
            clip: Arc::from(synth_loop(l, sr)), pos: 0.0, gain: 0.0, rate: 1.0, pan: 0.0,
            lp_a: 1.0, lp_z: 0.0, target: LoopTarget { gain: 0.0, rate: 1.0, pan: 0.0, cutoff: 18_000.0 },
        }).collect();
        Mixer {
            sr, clips, voices: vec![Voice::default(); VOICES], loops, ambient: None, ambient_target: 0.0,
            queue: Vec::with_capacity(QUEUE), master: 0.85, master_target: 0.85, room: Room::new(sr),
            room_level: 0.05, room_target: 0.05, smooth: 1.0 - (-1.0 / (0.04 * sr)).exp(),
        }
    }

    /// Queue a sound; applied at the start of the next render block.
    pub fn play(&mut self, p: Play) {
        if self.queue.len() < QUEUE { self.queue.push(p); }
    }
    pub fn set_loop(&mut self, which: Loop, t: LoopTarget) {
        if let Some(i) = LOOPS.iter().position(|l| *l == which) { self.loops[i].target = t; }
    }
    pub fn set_master(&mut self, gain: f32) { self.master_target = gain; }
    pub fn set_room(&mut self, indoor: bool) { self.room_target = if indoor { 0.32 } else { 0.05 }; }
    /// Swap the map's ambient bed (mono, `source_rate` Hz); resampled once here.
    pub fn set_ambient(&mut self, samples: Option<(Vec<f32>, f32)>) {
        self.ambient = samples.filter(|(s, _)| !s.is_empty()).map(|(s, rate)| {
            let ratio = rate / self.sr;
            let len = (s.len() as f32 / ratio) as usize;
            let resampled: Vec<f32> = (0..len).map(|i| {
                let x = i as f32 * ratio;
                let j = x as usize;
                let f = x - j as f32;
                s[j.min(s.len() - 1)] * (1.0 - f) + s[(j + 1) % s.len()] * f
            }).collect();
            LoopVoice { clip: Arc::from(resampled), pos: 0.0, gain: 0.0, rate: 1.0, pan: 0.0, lp_a: 1.0, lp_z: 0.0,
                target: LoopTarget::default() }
        });
    }
    pub fn set_ambient_gain(&mut self, gain: f32) { self.ambient_target = gain; }
    #[cfg(test)]
    pub fn active_voices(&self) -> usize { self.voices.iter().filter(|v| v.clip.is_some()).count() }

    fn start(&mut self, p: Play) {
        let ci = p.cue.index();
        let Some(clip) = self.clips[ci].get(p.variant % self.clips[ci].len()).cloned() else { return };
        let mut same_count = 0;
        let mut oldest_same: Option<(usize, f32)> = None;
        for (i, v) in self.voices.iter().enumerate() {
            if v.clip.is_some() && v.cue_index == ci {
                same_count += 1;
                let progress = v.pos / v.clip.as_ref().map_or(1.0, |c| c.len() as f32);
                if oldest_same.is_none_or(|(_, best)| progress > best) { oldest_same = Some((i, progress)); }
            }
        }
        let slot = if same_count >= p.cue.max_voices() {
            oldest_same.map(|(i, _)| i)
        } else if let Some(free) = self.voices.iter().position(|v| v.clip.is_none()) {
            Some(free)
        } else {
            // Steal the least important, most finished voice, but never one
            // that outranks the newcomer.
            let mut victim: Option<(usize, u8, f32)> = None;
            for (i, v) in self.voices.iter().enumerate() {
                let progress = v.pos / v.clip.as_ref().map_or(1.0, |c| c.len() as f32);
                let better = match victim {
                    None => true,
                    Some((_, pr, pg)) => v.priority < pr || (v.priority == pr && progress > pg),
                };
                if better { victim = Some((i, v.priority, progress)); }
            }
            victim.filter(|&(_, pr, _)| pr <= p.cue.priority()).map(|(i, _, _)| i)
        };
        let Some(i) = slot else { return };
        let pan = p.pan.clamp(-1.0, 1.0);
        let angle = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
        let g = p.gain.clamp(0.0, 2.0);
        self.voices[i] = Voice {
            clip: Some(clip), cue_index: ci, priority: p.cue.priority(), pos: 0.0,
            rate: p.rate.clamp(0.25, 4.0), gl: g * angle.cos(), gr: g * angle.sin(),
            lp_a: lp_coef(p.cutoff, self.sr), lp_z: 0.0, delay: (p.delay.max(0.0) * self.sr) as u32,
        };
    }

    /// Render interleaved stereo into `out` (length must be even).
    pub fn render(&mut self, out: &mut [f32]) {
        let mut queue = std::mem::take(&mut self.queue);
        for p in queue.drain(..) { self.start(p); }
        self.queue = queue;
        let k = self.smooth;
        for frame in out.chunks_exact_mut(2) {
            let (mut l, mut r, mut send) = (0.0_f32, 0.0_f32, 0.0_f32);
            for v in self.voices.iter_mut() {
                let Some(clip) = &v.clip else { continue };
                if v.delay > 0 { v.delay -= 1; continue; }
                let i = v.pos as usize;
                if i + 1 >= clip.len() { v.clip = None; continue; }
                let f = v.pos - i as f32;
                let s = clip[i] * (1.0 - f) + clip[i + 1] * f;
                v.lp_z += (s - v.lp_z) * v.lp_a;
                v.pos += v.rate;
                l += v.lp_z * v.gl;
                r += v.lp_z * v.gr;
                send += v.lp_z * (v.gl + v.gr) * 0.5;
            }
            for lv in self.loops.iter_mut().chain(self.ambient.iter_mut()) {
                let t = lv.target;
                lv.gain += (t.gain - lv.gain) * k;
                lv.rate += (t.rate.clamp(0.25, 4.0) - lv.rate) * k;
                lv.pan += (t.pan.clamp(-1.0, 1.0) - lv.pan) * k;
                lv.lp_a += (lp_coef(t.cutoff.max(60.0), self.sr) - lv.lp_a) * k;
                if lv.gain < 1e-4 { continue; }
                let n = lv.clip.len();
                let i = lv.pos as usize % n;
                let f = lv.pos.fract();
                let s = lv.clip[i] * (1.0 - f) + lv.clip[(i + 1) % n] * f;
                lv.pos = (lv.pos + lv.rate) % n as f32;
                lv.lp_z += (s - lv.lp_z) * lv.lp_a;
                let angle = (lv.pan + 1.0) * std::f32::consts::FRAC_PI_4;
                l += lv.lp_z * lv.gain * angle.cos();
                r += lv.lp_z * lv.gain * angle.sin();
                send += lv.lp_z * lv.gain * 0.3;
            }
            if let Some(a) = &mut self.ambient { a.target.gain = self.ambient_target; a.target.rate = 1.0; a.target.cutoff = 18_000.0; }
            self.room_level += (self.room_target - self.room_level) * k;
            let wet = self.room.tick(send) * self.room_level;
            self.master += (self.master_target - self.master) * k;
            frame[0] = soft_clip((l + wet) * self.master);
            frame[1] = soft_clip((r + wet) * self.master);
        }
    }
}

fn soft_clip(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    x * (27.0 + x * x) / (27.0 + 9.0 * x * x)
}

// -------------------------------------------------------------- spatial math

#[derive(Clone, Copy, Debug)]
pub struct Listener { pub pos: Vec3, pub forward: Vec3 }

/// Gain, pan and low-pass for a world sound, or `None` beyond its range.
/// Gain falls smoothly to zero at `range`; far sounds lose their highs; sounds
/// behind you are slightly quieter and duller.
pub fn spatialize(l: &Listener, src: Vec3, range: f32) -> Option<(f32, f32, f32, f32)> {
    let d = l.pos.distance(src);
    if !d.is_finite() || d >= range { return None; }
    let t = ((d - 5.0) / (range - 5.0)).clamp(0.0, 1.0);
    let mut gain = (1.0 - t).powi(2);
    let fwd = Vec3::new(l.forward.x, 0.0, l.forward.z).normalize_or(Vec3::NEG_Z);
    let right = fwd.cross(Vec3::Y);
    let dir = Vec3::new(src.x - l.pos.x, 0.0, src.z - l.pos.z).normalize_or_zero();
    let pan = if d < 1.5 { 0.0 } else { dir.dot(right).clamp(-1.0, 1.0) * 0.85 };
    let behind = (-dir.dot(fwd)).max(0.0);
    gain *= 1.0 - 0.2 * behind;
    let cutoff = (16_000.0 * (1.0 - t).powf(1.6) + 700.0) * (1.0 - 0.3 * behind);
    Some((gain, pan, cutoff, d))
}

/// Speed of sound used for far explosions, so a distant blast arrives late.
pub const SOUND_SPEED: f32 = 340.0;

// ----------------------------------------------------------------- director

/// Turns world changes into cues and loop targets. Holds only small,
/// fixed-size state between frames.
pub struct Director {
    rng: Rng,
    map: Option<crate::terrain::MapId>,
    weapon: Option<u8>,
    was_ground: bool,
    last_vy: f32,
    step: f32,
    step_side: f32,
    remote_steps: [f32; 16],
    last_heal: f32,
    alive: bool,
    spin_rate: f32,
    shields: Vec<f32>,
    healths: Vec<f32>,
    equip_cool: Vec<f32>,
    grenades: Vec<(Vec3, f32)>,
    grenades_next: Vec<(Vec3, f32)>,
    bounce_cool: f32,
    indoor: bool,
    indoor_check: f32,
    variant: u32,
}

impl Default for Director { fn default() -> Self { Self::new() } }

impl Director {
    pub fn new() -> Self {
        Director {
            rng: Rng(0x50D1), map: None, weapon: None, was_ground: true, last_vy: 0.0, step: 0.0,
            step_side: 1.0, remote_steps: [0.0; 16], last_heal: 0.0, alive: false, spin_rate: 0.0,
            shields: Vec::new(), healths: Vec::new(), equip_cool: Vec::new(),
            grenades: Vec::with_capacity(64), grenades_next: Vec::with_capacity(64), bounce_cool: 0.0, indoor: false, indoor_check: 0.0, variant: 0,
        }
    }

    pub fn indoor(&self) -> bool { self.indoor }

    fn pick(&mut self, cue: Cue) -> usize {
        self.variant = self.variant.wrapping_add(1 + (self.rng.next().abs() * 3.0) as u32);
        self.variant as usize % cue.variants()
    }
    fn jitter(&mut self) -> f32 { 1.0 + self.rng.next() * 0.04 }

    /// A cue at a world position, or `None` when out of range.
    pub fn world(&mut self, l: &Listener, cue: Cue, pos: Vec3, gain: f32) -> Option<Play> {
        let (g, pan, cutoff, d) = spatialize(l, pos, cue.range())?;
        let delay = if matches!(cue, Cue::BoomNear | Cue::BoomFar | Cue::GenBlast) && d > 40.0 { d / SOUND_SPEED } else { 0.0 };
        let variant = self.pick(cue);
        let rate = self.jitter();
        Some(Play { cue, variant, gain: g * gain, pan, cutoff, delay, rate })
    }

    /// A non-positional cue for the local player (own weapon, HUD, UI).
    pub fn local(&mut self, cue: Cue) -> Play {
        let variant = self.pick(cue);
        let mut p = Play::local(cue, variant);
        if matches!(cue, Cue::ChainShot | Cue::DiscFire | Cue::GrenadeFire) { p.rate = self.jitter(); }
        p
    }

    /// An explosion reported by the sim: near/far layer by distance, and the
    /// big generator blast when a generator explosion is at that spot.
    pub fn explosion(&mut self, l: &Listener, world: &World, pos: Vec3) -> Option<Play> {
        let generator = world.explosions.iter().any(|e| e.kind == 4 && e.pos.distance(pos) < 4.0 && e.age < 0.5);
        let d = l.pos.distance(pos);
        let cue = if generator { Cue::GenBlast } else if d > 75.0 { Cue::BoomFar } else { Cue::BoomNear };
        self.world(l, cue, pos, 1.0)
    }

    /// Per-frame scan. Pushes one-shots into `out` (cleared by the caller)
    /// and returns loop targets.
    pub fn update(&mut self, world: &World, l: &Listener, dt: f32, playing: bool, out: &mut Vec<Play>) -> [LoopTarget; 6] {
        let mut loops = [LoopTarget { gain: 0.0, rate: 1.0, pan: 0.0, cutoff: 18_000.0 }; 6];
        let dt = dt.clamp(0.0, 0.1);
        if self.map != Some(world.map) {
            self.map = Some(world.map);
            self.shields.clear();
            self.healths.clear();
            self.equip_cool.clear();
            self.weapon = None;
        }
        self.indoor_check -= dt;
        if self.indoor_check <= 0.0 {
            self.indoor_check = 0.25;
            self.indoor = indoor_at(world.map, l.pos);
        }
        let Some(me) = world.players.get(world.player_id) else { return loops };
        if !playing { self.weapon = Some(me.weapon); self.alive = me.alive; return loops; }

        // Local body: steps, landings, weapon swaps, kits, loops.
        let speed = me.vel.length();
        let flat = Vec3::new(me.vel.x, 0.0, me.vel.z).length();
        if me.alive {
            if me.on_ground && !me.skiing && flat > 1.2 {
                self.step += dt * flat / 2.2;
                if self.step >= 1.0 {
                    self.step -= 1.0;
                    self.step_side = -self.step_side;
                    let mut p = self.local(Cue::Footstep);
                    p.gain = 0.55 * (flat / 10.0).clamp(0.4, 1.0);
                    p.pan = 0.12 * self.step_side;
                    out.push(p);
                }
            } else { self.step = 0.9; }
            if me.on_ground && !self.was_ground && self.last_vy < -8.0 {
                let mut p = self.local(Cue::Land);
                p.gain = 0.4 + ((-self.last_vy - 8.0) / 25.0).clamp(0.0, 0.6);
                out.push(p);
            }
            if self.weapon.is_some_and(|w| w != me.weapon) { out.push(self.local(Cue::Switch)); }
            if me.kit_heal > 0.0 && self.last_heal <= 0.0 && self.alive { out.push(self.local(Cue::RepairKit)); }
            if me.jetting {
                loops[0] = LoopTarget { gain: 0.24, rate: 0.9 + (me.vel.y / 40.0).clamp(-0.1, 0.25) + speed / 220.0, pan: 0.0, cutoff: 9_000.0 };
            }
            if me.skiing && me.on_ground {
                let g = (speed / 38.0).clamp(0.0, 1.0);
                loops[1] = LoopTarget { gain: 0.3 * g, rate: 0.8 + speed / 90.0, pan: 0.0, cutoff: 4_000.0 + speed * 150.0 };
            }
            let wind = ((speed - 8.0) / 55.0).clamp(0.0, 1.0) * if self.indoor { 0.2 } else { 1.0 };
            loops[2] = LoopTarget { gain: 0.34 * wind, rate: 0.9 + wind * 0.3, pan: 0.0, cutoff: 500.0 + speed * 70.0 };
        } else { self.step = 0.9; }
        // Mirrors the viewmodel spin in effects.rs so sound and barrels agree.
        let firing = me.alive && me.weapon == 1 && me.cooldown > 0.0;
        let target = if firing { 42.0 } else { 0.0 };
        let accel = if target > self.spin_rate { 110.0 } else { 20.0 };
        self.spin_rate += (target - self.spin_rate).clamp(-accel * dt, accel * dt);
        if self.spin_rate > 0.5 {
            let s = self.spin_rate / 42.0;
            loops[3] = LoopTarget { gain: 0.17 * s, rate: 0.45 + 0.9 * s, pan: 0.0, cutoff: 6_000.0 };
        }
        self.was_ground = me.on_ground;
        self.last_vy = me.vel.y;
        self.last_heal = me.kit_heal;
        self.weapon = Some(me.weapon);
        self.alive = me.alive;

        // Other bodies: footsteps for the three nearest walkers.
        let mut near = [(usize::MAX, f32::INFINITY); 3];
        for (i, p) in world.players.iter().enumerate() {
            if i == world.player_id || !p.alive || !p.on_ground || p.skiing { continue; }
            let d = p.pos.distance(l.pos);
            if d >= Cue::Footstep.range() { continue; }
            if let Some(slot) = near.iter().position(|n| d < n.1) {
                near[slot..].rotate_right(1);
                near[slot] = (i, d);
            }
        }
        for &(i, _) in near.iter().filter(|n| n.0 != usize::MAX) {
            let p = &world.players[i];
            let flat = Vec3::new(p.vel.x, 0.0, p.vel.z).length();
            let slot = i % self.remote_steps.len();
            if flat < 1.2 { self.remote_steps[slot] = 0.9; continue; }
            self.remote_steps[slot] += dt * flat / 2.2;
            if self.remote_steps[slot] >= 1.0 {
                self.remote_steps[slot] -= 1.0;
                if let Some(p) = self.world(l, Cue::Footstep, p.pos, 0.8) { out.push(p); }
            }
        }

        // Equipment: shield pings, shield collapse, hull clanks, generator hum.
        let defs = peakrunner_core::equipment::definitions(world.map);
        if self.shields.len() != world.equipment.len() {
            self.shields = world.equipment.iter().map(|s| s.shield).collect();
            self.healths = world.equipment.iter().map(|s| s.health).collect();
            self.equip_cool = vec![0.0; world.equipment.len()];
        }
        let mut hum: Option<(f32, Vec3)> = None;
        for (i, (d, s)) in defs.iter().zip(&world.equipment).enumerate() {
            self.equip_cool[i] = (self.equip_cool[i] - dt).max(0.0);
            let pos = d.pos();
            if s.shield < self.shields[i] - 0.5 && self.equip_cool[i] <= 0.0 {
                let amount = ((self.shields[i] - s.shield) / 60.0).clamp(0.35, 1.0);
                if let Some(p) = self.world(l, Cue::ShieldHit, pos, amount) { out.push(p); }
                self.equip_cool[i] = 0.09;
            }
            if self.shields[i] > 0.0 && s.shield <= 0.0 && s.health > 0.0 {
                if let Some(p) = self.world(l, Cue::ShieldDown, pos, 1.0) { out.push(p); }
            }
            if s.health < self.healths[i] - 0.5 && s.shield <= 0.0 && s.health > 0.0 && self.equip_cool[i] <= 0.0 {
                if let Some(p) = self.world(l, Cue::HullHit, pos, 1.0) { out.push(p); }
                self.equip_cool[i] = 0.09;
            }
            self.shields[i] = s.shield;
            self.healths[i] = s.health;
            if d.kind == peakrunner_core::equipment::Kind::Generator && s.powered && !s.offline && s.health > 0.0 {
                let dist = pos.distance(l.pos);
                if dist < 32.0 && hum.is_none_or(|(best, _)| dist < best) { hum = Some((dist, pos)); }
            }
        }
        if let Some((dist, pos)) = hum {
            if let Some((_, pan, cutoff, _)) = spatialize(l, pos, 32.0) {
                let g = (1.0 - dist / 32.0).powi(2);
                loops[4] = LoopTarget { gain: 0.24 * g, rate: 1.0, pan, cutoff: cutoff.min(4_000.0) };
            }
        }

        // Projectiles: turret shots, grenade bounces, the hum of a passing disc.
        let step = dt.max(1.0 / 120.0) * 1.6;
        let mut disc_hum: Option<(f32, Vec3, Vec3)> = None;
        self.bounce_cool = (self.bounce_cool - dt).max(0.0);
        let grenades = std::mem::take(&mut self.grenades);
        let mut next = std::mem::take(&mut self.grenades_next);
        next.clear();
        for disc in &world.discs {
            if disc.owner >= crate::sim::MAX_PLAYERS {
                let (cue, life) = if disc.kind == 3 { (Cue::TurretPlasma, 3.0) } else { (Cue::TurretBullet, 1.0) };
                if disc.life > life - step {
                    if let Some(p) = self.world(l, cue, disc.pos, 1.0) { out.push(p); }
                }
            }
            if disc.kind == 2 {
                let prev = grenades.iter().find(|(p, _)| p.distance(disc.pos) < 6.0).map(|(_, vy)| *vy);
                if prev.is_some_and(|vy| vy < -3.0) && disc.vel.y > 1.0 && self.bounce_cool <= 0.0 {
                    if let Some(p) = self.world(l, Cue::Bounce, disc.pos, 1.0) { out.push(p); }
                    self.bounce_cool = 0.05;
                }
                next.push((disc.pos, disc.vel.y));
            }
            if disc.kind == 0 && !(disc.owner == world.player_id && disc.pos.distance(l.pos) < 5.0) {
                let dist = disc.pos.distance(l.pos);
                if dist < 26.0 && disc_hum.is_none_or(|(best, _, _)| dist < best) { disc_hum = Some((dist, disc.pos, disc.vel)); }
            }
        }
        self.grenades = next;
        self.grenades_next = grenades;
        if let Some((dist, pos, vel)) = disc_hum {
            if let Some((_, pan, cutoff, _)) = spatialize(l, pos, 26.0) {
                let toward = (l.pos - pos).normalize_or_zero().dot(vel);
                let doppler = (1.0 + toward / SOUND_SPEED).clamp(0.8, 1.25);
                loops[5] = LoopTarget { gain: 0.22 * (1.0 - dist / 26.0).powi(2), rate: doppler, pan, cutoff };
            }
        }
        loops
    }
}

/// Under a roof or below the terrain surface.
fn indoor_at(map: crate::terrain::MapId, eye: Vec3) -> bool {
    if crate::terrain::height_on(map, eye.x, eye.z) > eye.y + 0.5 { return true; }
    peakrunner_core::map_pack::on(map).is_some_and(|pack| pack.sweep(eye, eye + Vec3::Y * 24.0, 0.0).is_some())
}

/// Linear PCM WAV bytes (mono or stereo, 16-bit) for listening to renders.
#[cfg(test)]
pub fn wav(samples: &[f32], sr: u32, channels: u16) -> Vec<u8> {
    let mut b = Vec::with_capacity(44 + samples.len() * 2);
    let data = (samples.len() * 2) as u32;
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&channels.to_le_bytes());
    b.extend_from_slice(&sr.to_le_bytes());
    b.extend_from_slice(&(sr * channels as u32 * 2).to_le_bytes());
    b.extend_from_slice(&(channels * 2).to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for s in samples { b.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()); }
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::MapId;

    #[test]
    fn every_cue_is_deterministic_bounded_and_clean_at_both_rates() {
        for sr in [44_100.0, 48_000.0] {
            for &cue in &CUES {
                for v in 0..cue.variants() {
                    let a = synth(cue, v, sr);
                    assert_eq!(a, synth(cue, v, sr), "{cue:?} not deterministic");
                    assert!(!a.is_empty());
                    assert!(a.iter().all(|s| s.is_finite()), "{cue:?} non-finite");
                    let peak = a.iter().fold(0.0_f32, |m, s| m.max(s.abs()));
                    assert!(peak > 0.05 && peak <= 0.9, "{cue:?} peak {peak}");
                    assert_eq!(a[0], 0.0);
                    assert!(a.last().unwrap().abs() < 0.01, "{cue:?} ends with a click");
                }
            }
        }
    }

    #[test]
    fn variants_actually_differ() {
        for &cue in &CUES {
            if cue.variants() > 1 { assert_ne!(synth(cue, 0, 44_100.0), synth(cue, 1, 44_100.0), "{cue:?}"); }
        }
    }

    #[test]
    fn loops_are_seamless_and_bounded() {
        for &l in &LOOPS {
            let d = synth_loop(l, 44_100.0);
            assert!(d.iter().all(|s| s.is_finite() && s.abs() <= 0.51));
            // The wrap-around step must look like any other step in the loop.
            let steepest = d.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0_f32, f32::max);
            assert!((d[0] - d[d.len() - 1]).abs() <= steepest, "{l:?} seam");
        }
    }

    #[test]
    fn every_sim_and_network_event_has_a_cue() {
        // Scan the real sources so a new event name can't slip in silently.
        for src in [include_str!("../crates/core/src/sim.rs"), include_str!("online.rs")] {
            for part in src.split("push_event(").skip(1).chain(src.split("spatial_sounds.push((").skip(1)) {
                let mut names = Vec::new();
                for quoted in part.split('"').skip(1).step_by(2).take(3) {
                    if quoted.chars().all(|c| c.is_ascii_lowercase() || c == '_') && !quoted.is_empty() { names.push(quoted); }
                }
                let line = part.lines().next().unwrap_or("");
                for name in names.into_iter().filter(|n| line.contains(&format!("\"{n}\""))) {
                    assert!(Cue::from_event(name).is_some(), "event {name:?} has no cue");
                }
            }
            for word in ["capture_win", "capture_loss", "flag"] {
                if src.contains(&format!("\"{word},\"")) { assert!(Cue::from_event(word).is_some()); }
            }
        }
    }

    #[test]
    fn voice_cap_and_priority_stealing() {
        let mut m = Mixer::new(22_050.0);
        let mut buf = vec![0.0; 64];
        for i in 0..40 { m.play(Play::local(Cue::Footstep, i)); }
        m.render(&mut buf);
        assert_eq!(m.active_voices(), Cue::Footstep.max_voices(), "per-cue cap");
        // Fill the pool with low-priority voices of many kinds.
        let fillers = [Cue::ChainShot, Cue::TurretBullet, Cue::ShieldHit, Cue::HullHit, Cue::Bounce, Cue::BoomNear, Cue::BoomFar, Cue::Land];
        for _ in 0..8 { for &c in &fillers { m.play(Play::local(c, 0)); } }
        m.render(&mut buf);
        assert!(m.active_voices() <= VOICES);
        // A high-priority cue always gets a voice even when the pool is full.
        for c in [Cue::Flag, Cue::CaptureWin, Cue::GenBlast] {
            m.play(Play::local(c, 0));
            m.render(&mut buf);
            assert!(m.voices.iter().any(|v| v.clip.is_some() && v.cue_index == c.index()), "{c:?} dropped");
        }
        assert!(m.active_voices() <= VOICES);
        assert!(buf.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
    }

    #[test]
    fn low_priority_never_steals_from_high_priority() {
        let mut m = Mixer::new(22_050.0);
        let mut buf = vec![0.0; 2];
        for v in m.voices.iter_mut() {
            *v = Voice { clip: Some(Arc::from(vec![0.1; 1000])), cue_index: Cue::Flag.index(), priority: 6, rate: 1.0, lp_a: 1.0, ..Voice::default() };
        }
        m.play(Play::local(Cue::Footstep, 0));
        m.render(&mut buf);
        assert!(!m.voices.iter().any(|v| v.cue_index == Cue::Footstep.index() && v.clip.is_some()));
    }

    #[test]
    fn distance_pan_and_filter_behave() {
        let l = Listener { pos: Vec3::ZERO, forward: Vec3::NEG_Z };
        let (near, _, near_cut, _) = spatialize(&l, Vec3::new(0.0, 0.0, -10.0), 150.0).unwrap();
        let (far, _, far_cut, _) = spatialize(&l, Vec3::new(0.0, 0.0, -100.0), 150.0).unwrap();
        assert!(near > far && near_cut > far_cut);
        assert!(spatialize(&l, Vec3::new(0.0, 0.0, -151.0), 150.0).is_none());
        assert!(spatialize(&l, Vec3::splat(f32::NAN), 150.0).is_none());
        let (_, right, ..) = spatialize(&l, Vec3::new(30.0, 0.0, 0.0), 150.0).unwrap();
        let (_, left, ..) = spatialize(&l, Vec3::new(-30.0, 0.0, 0.0), 150.0).unwrap();
        assert!(right > 0.5 && left < -0.5);
        let (front_g, _, front_c, _) = spatialize(&l, Vec3::new(0.0, 0.0, -30.0), 150.0).unwrap();
        let (back_g, _, back_c, _) = spatialize(&l, Vec3::new(0.0, 0.0, 30.0), 150.0).unwrap();
        assert!(back_g < front_g && back_c < front_c);
    }

    #[test]
    fn far_explosions_arrive_late_and_generators_get_the_big_blast() {
        let mut d = Director::new();
        let mut w = World::new();
        w.set_map(MapId::Raindance);
        let l = Listener { pos: Vec3::ZERO, forward: Vec3::NEG_Z };
        let near = d.explosion(&l, &w, Vec3::new(0.0, 0.0, -20.0)).unwrap();
        let far = d.explosion(&l, &w, Vec3::new(0.0, 0.0, -300.0)).unwrap();
        assert_eq!(near.cue, Cue::BoomNear);
        assert_eq!(far.cue, Cue::BoomFar);
        assert!((far.delay - 300.0 / SOUND_SPEED).abs() < 0.01 && near.delay == 0.0);
        w.explosions.push(crate::sim::Explosion { pos: Vec3::new(0.0, 0.0, -60.0), age: 0.0, max_r: 20.0, kind: 4 });
        assert_eq!(d.explosion(&l, &w, Vec3::new(0.0, 0.0, -60.0)).unwrap().cue, Cue::GenBlast);
    }

    fn walker() -> (World, Director) {
        let mut w = World::new();
        w.set_map(MapId::Raindance);
        w.start_rift(true);
        (w, Director::new())
    }

    #[test]
    fn walking_makes_footsteps_and_skiing_hisses() {
        let (mut w, mut d) = walker();
        let me = w.player_id;
        w.players[me].alive = true;
        w.players[me].on_ground = true;
        w.players[me].skiing = false;
        w.players[me].vel = Vec3::new(8.0, 0.0, 0.0);
        let l = Listener { pos: w.players[me].pos, forward: Vec3::NEG_Z };
        let mut out = Vec::new();
        for _ in 0..60 { d.update(&w, &l, 1.0 / 60.0, true, &mut out); }
        let steps = out.iter().filter(|p| p.cue == Cue::Footstep).count();
        assert!((3..=5).contains(&steps), "{steps} steps in 1 s at 8 m/s");
        w.players[me].skiing = true;
        w.players[me].vel = Vec3::new(35.0, 0.0, 0.0);
        let loops = d.update(&w, &l, 1.0 / 60.0, true, &mut out);
        assert!(loops[1].gain > 0.2, "ski hiss");
        assert!(loops[2].gain > 0.0, "wind at speed");
        let silent = d.update(&w, &l, 1.0 / 60.0, false, &mut out);
        assert!(silent.iter().all(|t| t.gain == 0.0), "nothing plays outside a match");
    }

    #[test]
    fn landing_switching_and_kits_cue_once() {
        let (mut w, mut d) = walker();
        let me = w.player_id;
        w.players[me].alive = true;
        let l = Listener { pos: w.players[me].pos, forward: Vec3::NEG_Z };
        let mut out = Vec::new();
        w.players[me].on_ground = false;
        w.players[me].vel = Vec3::new(0.0, -20.0, 0.0);
        d.update(&w, &l, 1.0 / 60.0, true, &mut out);
        w.players[me].on_ground = true;
        w.players[me].vel = Vec3::ZERO;
        d.update(&w, &l, 1.0 / 60.0, true, &mut out);
        assert_eq!(out.iter().filter(|p| p.cue == Cue::Land).count(), 1);
        w.players[me].weapon = (w.players[me].weapon + 1) % 3;
        w.players[me].kit_heal = 60.0;
        d.update(&w, &l, 1.0 / 60.0, true, &mut out);
        d.update(&w, &l, 1.0 / 60.0, true, &mut out);
        assert_eq!(out.iter().filter(|p| p.cue == Cue::Switch).count(), 1);
        assert_eq!(out.iter().filter(|p| p.cue == Cue::RepairKit).count(), 1);
    }

    #[test]
    fn shield_damage_pings_and_collapse_and_generator_hums() {
        for map in [MapId::Raindance, MapId::BroadsideClone, MapId::StonehengeClone] {
            let mut w = World::new();
            w.set_map(map);
            w.start_rift(true);
            let defs = peakrunner_core::equipment::definitions(map);
            let Some(ti) = defs.iter().position(|d| d.kind == peakrunner_core::equipment::Kind::Turret) else { continue };
            let gi = defs.iter().position(|d| d.kind == peakrunner_core::equipment::Kind::Generator).unwrap();
            let mut d = Director::new();
            let l = Listener { pos: defs[ti].pos() + Vec3::new(8.0, 1.0, 0.0), forward: Vec3::NEG_Z };
            let mut out = Vec::new();
            d.update(&w, &l, 1.0 / 60.0, true, &mut out);
            w.equipment[ti].shield -= 50.0;
            d.update(&w, &l, 1.0 / 60.0, true, &mut out);
            assert!(out.iter().any(|p| p.cue == Cue::ShieldHit), "{map:?} shield ping");
            w.equipment[ti].shield = 0.0;
            for _ in 0..10 { d.update(&w, &l, 1.0 / 60.0, true, &mut out); }
            assert_eq!(out.iter().filter(|p| p.cue == Cue::ShieldDown).count(), 1, "{map:?}");
            let lg = Listener { pos: defs[gi].pos() + Vec3::new(3.0, 1.0, 0.0), forward: Vec3::NEG_Z };
            let loops = d.update(&w, &lg, 1.0 / 60.0, true, &mut out);
            assert!(loops[4].gain > 0.05, "{map:?} generator hum");
            w.equipment[gi].offline = true;
            let loops = d.update(&w, &lg, 1.0 / 60.0, true, &mut out);
            assert_eq!(loops[4].gain, 0.0, "{map:?} dead generator is silent");
        }
    }

    #[test]
    fn chaingun_spin_tracks_the_trigger() {
        let (mut w, mut d) = walker();
        let me = w.player_id;
        w.players[me].alive = true;
        w.players[me].weapon = 1;
        w.players[me].cooldown = 0.1;
        let l = Listener { pos: w.players[me].pos, forward: Vec3::NEG_Z };
        let mut out = Vec::new();
        let mut loops = [LoopTarget::default(); 6];
        for _ in 0..30 { loops = d.update(&w, &l, 1.0 / 60.0, true, &mut out); }
        assert!(loops[3].gain > 0.1 && loops[3].rate > 1.0);
        w.players[me].cooldown = 0.0;
        for _ in 0..240 { loops = d.update(&w, &l, 1.0 / 60.0, true, &mut out); }
        assert_eq!(loops[3].gain, 0.0, "spin coasts to silence");
    }

    #[test]
    fn mixer_renders_finite_panned_audio() {
        let mut m = Mixer::new(44_100.0);
        let mut buf = vec![0.0; 44_100];
        m.play(Play { pan: 1.0, ..Play::local(Cue::DiscFire, 0) });
        m.render(&mut buf);
        let (l, r) = buf.chunks_exact(2).fold((0.0_f32, 0.0_f32), |(l, r), f| (l + f[0].abs(), r + f[1].abs()));
        assert!(r > l * 3.0, "hard right pan");
        assert!(buf.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
    }

    #[test]
    fn indoor_detects_roofs_and_tunnels() {
        assert!(!indoor_at(MapId::Raindance, Vec3::new(1024.0, 2000.0, 1024.0)));
        // Deep under the terrain surface counts as a tunnel.
        let y = crate::terrain::height_on(MapId::Raindance, 1024.0, 1024.0);
        assert!(indoor_at(MapId::Raindance, Vec3::new(1024.0, y - 10.0, 1024.0)));
    }

    /// Writes listenable samples to research/audio-samples/ (ignored in git).
    /// Run: cargo test -p peakrunner --lib render_audio_samples -- --ignored
    #[test]
    #[ignore]
    fn render_audio_samples() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../research/audio-samples");
        std::fs::create_dir_all(&dir).unwrap();
        let sr = 44_100.0;
        let mono = |name: &str, s: Vec<f32>| std::fs::write(dir.join(format!("{name}.wav")), wav(&s, 44_100, 1)).unwrap();
        mono("disc-fire", synth(Cue::DiscFire, 0, sr));
        mono("grenade-fire", synth(Cue::GrenadeFire, 0, sr));
        mono("shield-hit", synth(Cue::ShieldHit, 1, sr));
        mono("shield-down", synth(Cue::ShieldDown, 0, sr));
        mono("generator-explosion", synth(Cue::GenBlast, 0, sr));
        mono("repair-kit", synth(Cue::RepairKit, 0, sr));
        mono("turret-plasma", synth(Cue::TurretPlasma, 0, sr));
        mono("footsteps", (0..6).flat_map(|v| { let mut s = synth(Cue::Footstep, v, sr); s.resize(11_000, 0.0); s }).collect());
        // A chaingun burst through the mixer: rotating variants over the spin loop.
        let mut m = Mixer::new(sr);
        let mut burst = vec![0.0; 44_100 * 2];
        m.set_loop(Loop::Spin, LoopTarget { gain: 0.17, rate: 1.35, pan: 0.0, cutoff: 6000.0 });
        for (i, chunk) in burst.chunks_mut(4_410 * 2 / 2).enumerate() {
            if i < 14 { m.play(Play { rate: 1.0 + (i % 3) as f32 * 0.02, ..Play::local(Cue::ChainShot, i) }); }
            m.render(chunk);
        }
        std::fs::write(dir.join("chaingun-burst.wav"), wav(&burst, 44_100, 2)).unwrap();
        let l = Listener { pos: Vec3::ZERO, forward: Vec3::NEG_Z };
        let mut d = Director::new();
        let w = World::new();
        for (name, dist) in [("explosion-near", 25.0), ("explosion-far", 260.0)] {
            let mut m = Mixer::new(sr);
            m.play(d.explosion(&l, &w, Vec3::new(dist * 0.4, 0.0, -dist)).unwrap());
            let mut s = vec![0.0; 44_100 * 2 * 3];
            m.render(&mut s);
            std::fs::write(dir.join(format!("{name}.wav")), wav(&s, 44_100, 2)).unwrap();
        }
        for (name, lp, target) in [
            ("jet-loop", Loop::Jet, LoopTarget { gain: 0.24, rate: 1.1, pan: 0.0, cutoff: 9000.0 }),
            ("ski-hiss", Loop::Ski, LoopTarget { gain: 0.3, rate: 1.2, pan: 0.0, cutoff: 8000.0 }),
            ("wind-at-speed", Loop::Wind, LoopTarget { gain: 0.34, rate: 1.1, pan: 0.0, cutoff: 3500.0 }),
            ("generator-hum", Loop::Hum, LoopTarget { gain: 0.24, rate: 1.0, pan: -0.4, cutoff: 3000.0 }),
        ] {
            let mut m = Mixer::new(sr);
            m.set_loop(lp, target);
            let mut s = vec![0.0; 44_100 * 2 * 3];
            m.render(&mut s);
            std::fs::write(dir.join(format!("{name}.wav")), wav(&s, 44_100, 2)).unwrap();
        }
    }
}
