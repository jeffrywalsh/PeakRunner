/// Smooth finite-range attenuation; local UI cues bypass this function.
fn distance_gain(name: &str, distance: f32) -> f32 {
    if !distance.is_finite() { return 0.0; }
    let range = if name == "boom" { 180.0 } else { 120.0 };
    let t = ((distance - 6.0) / (range - 6.0)).clamp(0.0, 1.0);
    (1.0 - t).powi(2)
}

#[cfg(test)]
mod spatial_tests {
    use super::*;
    #[test] fn capture_stings_are_distinct_bounded_and_clean() {
        for sr in [44_100.,48_000.] {
            let win=capture_samples(sr,true);let loss=capture_samples(sr,false);
            assert_ne!(win,loss);
            for samples in [&win,&loss] {
                assert_eq!(samples.len(),(sr*2.4) as usize);
                assert!(samples.iter().all(|s|s.is_finite() && s.abs()<0.8));
                assert!(samples.first().unwrap().abs()<0.001 && samples.last().unwrap().abs()<0.001);
                assert!(samples.iter().map(|s|s.abs()).fold(0.,f32::max)>0.1);
            }
        }
    }
    #[test] fn distance_is_smooth_bounded_and_silent_beyond_range() {
        for name in ["boom", "disc", "chain", "grenade"] {
            assert_eq!(distance_gain(name, 0.0), 1.0);
            assert!(distance_gain(name, 30.0) > distance_gain(name, 90.0));
            assert_eq!(distance_gain(name, 181.0), 0.0);
            assert_eq!(distance_gain(name, f32::NAN), 0.0);
        }
    }
}

/// Short synthesized cues. Native uses the system output; the web build uses WebAudio.
pub struct Audio {
    muted: bool,
    #[cfg(not(target_arch = "wasm32"))]
    _sink: Option<rodio::MixerDeviceSink>,
    #[cfg(not(target_arch = "wasm32"))]
    jet: Option<rodio::Player>,
    #[cfg(not(target_arch = "wasm32"))]
    ambient: Option<rodio::Player>,
    #[cfg(target_arch = "wasm32")]
    ctx: Option<web_sys::AudioContext>,
    #[cfg(target_arch = "wasm32")]
    master: Option<web_sys::GainNode>,
    #[cfg(target_arch = "wasm32")]
    jet: Option<web_sys::AudioBufferSourceNode>,
    #[cfg(target_arch = "wasm32")]
    jet_gain: Option<web_sys::GainNode>,
}

impl Audio {
    #[cfg(all(test, not(target_arch = "wasm32")))]
    pub(crate) fn silent() -> Self { Self { muted: true, _sink: None, jet: None, ambient: None } }
    pub fn new() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let sink = rodio::DeviceSinkBuilder::open_default_sink()
                .map_err(|err| {
                    log::warn!("audio output unavailable: {err}");
                    err
                })
                .ok();
            let jet = sink.as_ref().and_then(|sink| {
                let player = rodio::Player::connect_new(sink.mixer());
                let noise = noise_buffer();
                player.append(noise);
                player.set_volume(0.0);
                Some(player)
            });
            let ambient=peakrunner_core::map_pack::active().and_then(|pack| {
                use rodio::Source;
                let sink=sink.as_ref()?;
                let bytes=pack.asset("ambient.f32").ok()?;
                let samples:Vec<f32>=bytes.chunks_exact(4).map(|b|f32::from_le_bytes(b.try_into().unwrap())).collect();
                let player=rodio::Player::connect_new(sink.mixer());
                player.append(rodio::buffer::SamplesBuffer::new(ch(),rate(),samples).repeat_infinite());
                player.set_volume(0.0);Some(player)
            });
            Self {
                muted: false,
                _sink: sink,
                jet,
                ambient,
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            Self {
                muted: false,
                ctx: None,
                master: None,
                jet: None,
                jet_gain: None,
            }
        }
    }

    pub fn unlock(&mut self) {
        #[cfg(target_arch = "wasm32")]
        {
            self.ensure();
            if let Some(ctx) = &self.ctx {
                if ctx.state() == web_sys::AudioContextState::Suspended {
                    let _ = ctx.resume();
                }
            }
        }
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        #[cfg(not(target_arch = "wasm32"))]
        if muted {
            if let Some(ambient)=&self.ambient {ambient.set_volume(0.0);}
            if let Some(jet) = &self.jet {
                jet.set_volume(0.0);
            }
        }
        #[cfg(target_arch = "wasm32")]
        if let (Some(ctx), Some(master)) = (&self.ctx, &self.master) {
            let _ = master.gain().set_value_at_time(if muted { 0.0 } else { 0.85 }, ctx.current_time());
        }
    }

    pub fn muted(&self) -> bool {
        self.muted
    }

    pub fn set_map_ambience(&mut self,map:crate::terrain::MapId,eye:glam::Vec3,playing:bool) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(player)=&self.ambient {
            let volume=peakrunner_core::map_pack::on(map).filter(|_|playing&&!self.muted).map_or(0.0,|pack| {
                pack.manifest.ambient_emitters.iter().map(|e| {
                    let distance=eye.distance(glam::Vec3::new(e[0],e[1],e[2]));
                    e[3]*(e[4]/distance.max(e[4])).min(1.0)*(1.0-distance/e[5]).clamp(0.0,1.0)
                }).fold(0.0_f32,f32::max)
            });
            player.set_volume(volume);
        }
        #[cfg(target_arch = "wasm32")]
        let _=(map,eye,playing);
    }

    pub fn play(&mut self, name: &str) {
        self.play_gain(name, 1.0);
    }

    pub fn play_at(&mut self, name: &str, distance: f32) {
        self.play_gain(name, distance_gain(name, distance));
    }

    fn play_gain(&mut self, name: &str, gain: f32) {
        if gain <= 0.0 { return; }
        if self.muted || name.is_empty() {
            return;
        }
        self.unlock();
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(sink) = &self._sink else { return };
            let samples = synth(name).into_iter().map(|s| s * gain).collect::<Vec<_>>();
            if samples.is_empty() {
                return;
            }
            let buf = rodio::buffer::SamplesBuffer::new(ch(), rate(), samples);
            sink.mixer().add(buf);
        }
        #[cfg(target_arch = "wasm32")]
        self.play_web(name, gain);
    }

    pub fn set_jet(&mut self, on: bool) {
        if self.muted {
            on_mute_jet(self);
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(jet) = &self.jet {
            jet.set_volume(if on { 0.22 } else { 0.0 });
        }
        #[cfg(target_arch = "wasm32")]
        self.set_jet_web(on);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn on_mute_jet(audio: &Audio) {
    if let Some(jet) = &audio.jet {
        jet.set_volume(0.0);
    }
}

#[cfg(target_arch = "wasm32")]
fn on_mute_jet(audio: &mut Audio) {
    audio.set_jet_web(false);
}

#[cfg(not(target_arch = "wasm32"))]
fn ch() -> std::num::NonZero<u16> {
    std::num::NonZero::new(1).expect("nonzero")
}

#[cfg(not(target_arch = "wasm32"))]
fn rate() -> std::num::NonZero<u32> {
    std::num::NonZero::new(44_100).expect("nonzero")
}

#[cfg(not(target_arch = "wasm32"))]
fn noise_buffer() -> impl rodio::Source<Item = f32> + Send + 'static {
    use rodio::Source;
    let data = jet_samples(44_100.0);
    rodio::buffer::SamplesBuffer::new(ch(), rate(), data).repeat_infinite()
}

#[cfg(not(target_arch = "wasm32"))]
fn synth(name: &str) -> Vec<f32> {
    let sr = 44_100.0;
    match name {
        "disc" => disc_samples(sr, false),
        "disc_ready" => disc_samples(sr, true),
        "chain" => firearm_samples(sr, false),
        "grenade" => firearm_samples(sr, true),
        "boom" => explosion_samples(sr),
        "hit" => tone(980.0, 0.04, sr, 0.12, 0.0),
        "pain" => burst(0.12, sr, 0.2),
        "death" => tone(220.0, 0.4, sr, 0.16, -160.0),
        "flag" => mix(&[tone(520.0, 0.12, sr, 0.12, 0.0), tone(780.0, 0.16, sr, 0.1, 0.0)]),
        "capture_win" => capture_samples(sr, true),
        "capture_loss" => capture_samples(sr, false),
        "drop" | "return" => tone(330.0, 0.1, sr, 0.12, -40.0),
        "start" => tone(196.0, 0.3, sr, 0.14, 80.0),
        "end" => tone(262.0, 0.4, sr, 0.14, -60.0),
        _ => Vec::new(),
    }
}

/// Original 2.4-second capture motif: impact, sequenced synth notes and a
/// sustained chord. Shared PCM keeps native and browser timing identical.
fn capture_samples(sr:f32, victory:bool)->Vec<f32> {
    let notes = if victory { [261.63,329.63,392.0,523.25] } else { [392.0,349.23,311.13,261.63] };
    let chord = if victory { [261.63,329.63,392.0] } else { [130.81,155.56,196.0] };
    let duration=2.4;
    (0..(sr*duration) as usize).map(|i| {
        let t=i as f32/sr;
        let mut sample=0.16*(std::f32::consts::TAU*(90.*t-22.*t*t)).sin()*(-t*13.).exp();
        for (n,freq) in notes.iter().enumerate() {
            let age=t-n as f32*0.22;
            if age>=0. {
                let env=(age/0.012).min(1.)*(-age*4.0).exp();
                let phase=std::f32::consts::TAU*freq*age;
                sample+=0.14*env*(phase.sin()+0.22*(phase*2.).sin());
            }
        }
        if t>0.72 {
            let age=t-0.72;
            let env=(age/0.08).min(1.)*(-age*1.8).exp();
            for freq in chord {sample+=0.045*env*(std::f32::consts::TAU*freq*age).sin();}
        }
        sample*(t/0.006).min(1.)*((duration-t)/0.12).clamp(0.,1.)
    }).collect()
}

/// Shared native/WebAudio PCM: a short mechanical attack, resonant body and
/// descending spin tail. Original synthesis, not a sample from the Tribes game.
fn disc_samples(sr: f32, ready: bool) -> Vec<f32> {
    let duration = if ready { 0.18 } else { 0.48 };
    let count = (sr * duration) as usize;
    let mut samples = Vec::with_capacity(count);
    let mut seed = 0xD15C_u32;
    let mut phase = 0.0_f32;
    let mut filtered = 0.0_f32;
    for i in 0..count {
        let t = i as f32 / sr;
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = (seed >> 8) as f32 / 16_777_216.0 * 2.0 - 1.0;
        filtered += (noise - filtered) * (1.0 - (-6500.0 / sr).exp());
        let frequency = if ready { 480.0 + 500.0 * t } else { 135.0 + 620.0 * (-t * 19.0).exp() };
        phase += std::f32::consts::TAU * frequency / sr;
        let click = noise * (-t * 180.0).exp() * 0.2;
        let sample = if ready {
            let latch = (t - 0.055).max(0.0);
            let second_click = if t > 0.055 { filtered * (-latch * 90.0).exp() * 0.18 } else { 0.0 };
            click + second_click + phase.sin() * (-t * 30.0).exp() * 0.065
        } else {
            let body = (std::f32::consts::TAU * 85.0 * t).sin() * (-t * 16.0).exp() * 0.32;
            let whirr = phase.sin() + (phase * 2.03).sin() * 0.28;
            let spin = 0.78 + 0.22 * (std::f32::consts::TAU * 43.0 * t).sin();
            click + body + filtered * (-t * 20.0).exp() * 0.28
                + whirr * spin * (-t * 9.0).exp() * 0.16
        };
        let attack = (t / 0.0015).min(1.0);
        let release = ((duration - t) / 0.015).clamp(0.0, 1.0);
        samples.push(sample * attack * release);
    }
    samples
}

/// Low exhaust rumble with a restrained turbine layer, crossfaded at the seam.
fn jet_samples(sr: f32) -> Vec<f32> {
    let count = (sr * 2.0) as usize;
    let overlap = (sr * 0.04) as usize;
    let mut data = Vec::with_capacity(count + overlap);
    let mut seed = 0x4A37_u32;
    let mut low = 0.0;
    let mut mid = 0.0;
    for i in 0..count + overlap {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = (seed >> 8) as f32 / 16_777_216.0 * 2.0 - 1.0;
        low += (noise - low) * (1.0 - (-1800.0 / sr).exp());
        mid += (noise - mid) * (1.0 - (-9000.0 / sr).exp());
        let t = i as f32 / sr;
        let turbine = (std::f32::consts::TAU * 148.0 * t
            + (std::f32::consts::TAU * 3.0 * t).sin() * 0.3).sin();
        data.push(low * 0.65 + mid * 0.13 + turbine * 0.055);
    }
    for i in 0..overlap {
        let blend = i as f32 / overlap as f32;
        data[i] = data[count + i] * (1.0 - blend) + data[i] * blend;
    }
    data.truncate(count);
    data
}

fn explosion_samples(sr: f32) -> Vec<f32> {
    let duration = 0.65;
    let count = (sr * duration) as usize;
    let mut data = Vec::with_capacity(count);
    let mut seed = 0xB00B_u32;
    let mut low = 0.0;
    for i in 0..count {
        let t = i as f32 / sr;
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = (seed >> 8) as f32 / 16_777_216.0 * 2.0 - 1.0;
        low += (noise - low) * (1.0 - (-2400.0 / sr).exp());
        let thump = (std::f32::consts::TAU * (68.0 * t - 18.0 * t * t)).sin();
        let body = thump * (-t * 12.0).exp() * 0.34 + low * (-t * 7.0).exp() * 0.65;
        let crack = noise * (-t * 95.0).exp() * 0.16;
        data.push((body + crack) * (t / 0.002).min(1.0)
            * ((duration - t) / 0.03).clamp(0.0, 1.0));
    }
    data
}

fn firearm_samples(sr: f32, grenade: bool) -> Vec<f32> {
    let duration = if grenade { 0.25 } else { 0.065 };
    let mut seed = 0xC4A1_u32;
    let mut low = 0.0;
    (0..(sr * duration) as usize).map(|i| {
        let t = i as f32 / sr;
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = (seed >> 8) as f32 / 16_777_216.0 * 2.0 - 1.0;
        low += (noise - low) * (1.0 - (-3500.0 / sr).exp());
        let body = (std::f32::consts::TAU * if grenade { 92.0 } else { 175.0 } * t).sin();
        let sample = if grenade { (body * 0.3 + low * 0.3) * (-t * 20.0).exp() }
            else { (noise * 0.16 + body * 0.13) * (-t * 65.0).exp() };
        sample * (t / 0.001).min(1.0) * ((duration - t) / 0.008).clamp(0.0, 1.0)
    }).collect()
}

#[cfg(test)]
mod disc_audio_tests {
    #[test]
    fn jet_and_explosion_are_finite_and_have_headroom() {
        for sr in [44_100.0, 48_000.0] {
            let jet = super::jet_samples(sr);
            let boom = super::explosion_samples(sr);
            let chain = super::firearm_samples(sr, false);
            let grenade = super::firearm_samples(sr, true);
            for samples in [&jet, &boom, &chain, &grenade] {
                let peak = samples.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
                assert!(peak > 0.02 && peak < 0.9);
                assert!(samples.iter().all(|s| s.is_finite()));
            }
            assert!((jet[0] - jet.last().unwrap()).abs() < 0.1);
            assert_eq!(boom[0], 0.0);
            assert!(boom.last().unwrap().abs() < 0.001);
        }
    }

    #[test]
    fn disc_cues_have_clean_edges_and_headroom_at_both_sample_rates() {
        for sr in [44_100.0, 48_000.0] {
            for ready in [false, true] {
                let samples = super::disc_samples(sr, ready);
                assert!(!samples.is_empty());
                assert_eq!(samples[0], 0.0);
                assert!(samples.last().unwrap().abs() < 0.001);
                let peak = samples.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
                assert!(peak > 0.02 && peak < 0.9);
                assert!(samples.iter().all(|s| s.is_finite()));
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn tone(freq: f32, dur: f32, sr: f32, gain: f32, slide: f32) -> Vec<f32> {
    let n = (sr * dur) as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let k = t / dur;
        let f = freq + slide * k;
        let env = (1.0 - k).max(0.0);
        out.push((t * f * std::f32::consts::TAU).sin() * gain * env);
    }
    out
}

#[cfg(not(target_arch = "wasm32"))]
fn burst(dur: f32, sr: f32, gain: f32) -> Vec<f32> {
    let n = (sr * dur) as usize;
    let mut out = Vec::with_capacity(n);
    let mut s = 0xA5A5u32;
    for i in 0..n {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = (s >> 8) as f32 / 16_777_216.0 * 2.0 - 1.0;
        let k = i as f32 / n as f32;
        out.push(noise * gain * (1.0 - k));
    }
    out
}

#[cfg(not(target_arch = "wasm32"))]
fn mix(parts: &[Vec<f32>]) -> Vec<f32> {
    let n = parts.iter().map(Vec::len).max().unwrap_or(0);
    let mut out = vec![0.0; n];
    for part in parts {
        for (i, s) in part.iter().enumerate() {
            out[i] += s;
        }
    }
    out
}

#[cfg(target_arch = "wasm32")]
impl Audio {
    fn ensure(&mut self) {
        if self.ctx.is_some() {
            return;
        }
        let Ok(ctx) = web_sys::AudioContext::new() else {
            return;
        };
        let Ok(master) = ctx.create_gain() else { return };
        master.gain().set_value(0.85);
        let Ok(sfx) = ctx.create_gain() else { return };
        sfx.gain().set_value(0.7);
        if sfx.connect_with_audio_node(&master).is_err() {
            return;
        }
        if master.connect_with_audio_node(&ctx.destination()).is_err() {
            return;
        }
        self.ctx = Some(ctx);
        self.master = Some(sfx);
    }

    fn play_web(&mut self, name: &str, gain: f32) {
        self.ensure();
        let (Some(ctx), Some(dest)) = (&self.ctx, &self.master) else {
            return;
        };
        let now = ctx.current_time();
        match name {
            "disc" | "disc_ready" | "boom" | "chain" | "grenade" | "capture_win" | "capture_loss" => {
                let rate = ctx.sample_rate();
                let mut samples = if name == "capture_win" || name == "capture_loss" {capture_samples(rate,name=="capture_win")}
                    else if name == "boom" { explosion_samples(rate) }
                    else if name == "chain" || name == "grenade" { firearm_samples(rate, name == "grenade") }
                    else { disc_samples(rate, name == "disc_ready") };
                for sample in &mut samples { *sample *= gain; }
                let Ok(buffer) = ctx.create_buffer(1, samples.len() as u32, rate) else { return };
                if buffer.copy_to_channel(&samples, 0).is_err() { return; }
                let Ok(src) = ctx.create_buffer_source() else { return };
                src.set_buffer(Some(&buffer));
                if src.connect_with_audio_node(dest).is_ok() { let _ = src.start(); }
            }
            "hit" => beep(ctx, dest, 980.0, 0.04, now, 0.0),
            "pain" => burst(ctx, dest, 0.12, 0.2, now),
            "death" => beep(ctx, dest, 220.0, 0.4, now, -160.0),
            "flag" => {
                beep(ctx, dest, 520.0, 0.12, now, 0.0);
                beep(ctx, dest, 780.0, 0.16, now, 0.0);
            }
            "drop" | "return" => beep(ctx, dest, 330.0, 0.1, now, -40.0),
            "start" => beep(ctx, dest, 196.0, 0.3, now, 80.0),
            "end" => beep(ctx, dest, 262.0, 0.4, now, -60.0),
            _ => {}
        }
    }

    fn set_jet_web(&mut self, on: bool) {
        self.ensure();
        let (Some(ctx), Some(dest)) = (&self.ctx, &self.master) else {
            return;
        };
        if on && self.jet.is_none() {
            let data = jet_samples(ctx.sample_rate());
            let Ok(buffer) = ctx.create_buffer(1, data.len() as u32, ctx.sample_rate()) else {
                return;
            };
            let _ = buffer.copy_to_channel(&data, 0);
            let Ok(src) = ctx.create_buffer_source() else { return };
            src.set_buffer(Some(&buffer));
            src.set_loop(true);
            let Ok(gain) = ctx.create_gain() else { return };
            gain.gain().set_value(0.0001);
            let _ = gain.gain().set_target_at_time(0.22, ctx.current_time(), 0.05);
            if src.connect_with_audio_node(&gain).is_err() {
                return;
            }
            if gain.connect_with_audio_node(dest).is_err() {
                return;
            }
            let _ = src.start();
            self.jet = Some(src);
            self.jet_gain = Some(gain);
        } else if !on {
            if let Some(gain) = &self.jet_gain {
                let _ = gain.gain().set_target_at_time(0.0001, ctx.current_time(), 0.04);
            }
            if let Some(src) = self.jet.take() {
                #[allow(deprecated)]
                let _ = src.stop_with_when(ctx.current_time() + 0.16);
            }
            self.jet_gain = None;
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn beep(ctx: &web_sys::AudioContext, dest: &web_sys::GainNode, freq: f32, dur: f64, now: f64, slide: f32) {
    let Ok(osc) = ctx.create_oscillator() else { return };
    let Ok(gain) = ctx.create_gain() else { return };
    osc.frequency().set_value(freq.max(40.0));
    if slide != 0.0 {
        let _ = osc.frequency().exponential_ramp_to_value_at_time((freq + slide).max(40.0), now + dur);
    }
    gain.gain().set_value(0.12);
    let _ = gain.gain().exponential_ramp_to_value_at_time(0.001, now + dur);
    if osc.connect_with_audio_node(&gain).is_err() {
        return;
    }
    if gain.connect_with_audio_node(dest).is_err() {
        return;
    }
    let _ = osc.start();
    let _ = osc.stop_with_when(now + dur);
}

#[cfg(target_arch = "wasm32")]
fn burst(ctx: &web_sys::AudioContext, dest: &web_sys::GainNode, dur: f64, gain_v: f32, now: f64) {
    let rate = ctx.sample_rate();
    let frames = (rate * dur as f32) as u32;
    let Ok(buffer) = ctx.create_buffer(1, frames.max(1), rate) else { return };
    let mut data = vec![0.0f32; frames.max(1) as usize];
    let mut n = 0xBEu32;
    for s in &mut data {
        n = n.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        *s = ((n >> 8) as f32 / 16_777_216.0) * 2.0 - 1.0;
    }
    let _ = buffer.copy_to_channel(&data, 0);
    let Ok(src) = ctx.create_buffer_source() else { return };
    src.set_buffer(Some(&buffer));
    let Ok(gain) = ctx.create_gain() else { return };
    gain.gain().set_value(gain_v);
    let _ = gain.gain().exponential_ramp_to_value_at_time(0.001, now + dur);
    if src.connect_with_audio_node(&gain).is_err() {
        return;
    }
    if gain.connect_with_audio_node(dest).is_err() {
        return;
    }
    let _ = src.start();
}
