/// Short synthesized cues. Native uses the system output; the web build uses WebAudio.
pub struct Audio {
    muted: bool,
    #[cfg(not(target_arch = "wasm32"))]
    _sink: Option<rodio::MixerDeviceSink>,
    #[cfg(not(target_arch = "wasm32"))]
    jet: Option<rodio::Player>,
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
                let noise = noise_buffer(44_100, 0.06);
                player.append(noise);
                player.set_volume(0.0);
                Some(player)
            });
            Self {
                muted: false,
                _sink: sink,
                jet,
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

    pub fn play(&mut self, name: &str) {
        if self.muted || name.is_empty() {
            return;
        }
        self.unlock();
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(sink) = &self._sink else { return };
            let samples = synth(name);
            if samples.is_empty() {
                return;
            }
            let buf = rodio::buffer::SamplesBuffer::new(ch(), rate(), samples);
            sink.mixer().add(buf);
        }
        #[cfg(target_arch = "wasm32")]
        self.play_web(name);
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
fn noise_buffer(frames: usize, gain: f32) -> impl rodio::Source<Item = f32> + Send + 'static {
    use rodio::Source;
    let mut data = Vec::with_capacity(frames);
    let mut n = 0x1234u32;
    for _ in 0..frames {
        n = n.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let s = (n >> 8) as f32 / 16_777_216.0 * 2.0 - 1.0;
        data.push(s * gain);
    }
    rodio::buffer::SamplesBuffer::new(ch(), rate(), data).repeat_infinite()
}

#[cfg(not(target_arch = "wasm32"))]
fn synth(name: &str) -> Vec<f32> {
    let sr = 44_100.0;
    match name {
        "disc" => mix(&[tone(180.0, 0.16, sr, 0.16, -80.0), burst(0.12, sr, 0.22)]),
        "bolt" => mix(&[tone(1400.0, 0.05, sr, 0.08, 400.0), burst(0.04, sr, 0.1)]),
        "boom" => mix(&[tone(70.0, 0.22, sr, 0.2, -30.0), burst(0.28, sr, 0.35)]),
        "hit" => tone(980.0, 0.04, sr, 0.12, 0.0),
        "pain" => burst(0.12, sr, 0.2),
        "death" => tone(220.0, 0.4, sr, 0.16, -160.0),
        "flag" => mix(&[tone(520.0, 0.12, sr, 0.12, 0.0), tone(780.0, 0.16, sr, 0.1, 0.0)]),
        "capture" => mix(&[
            tone(392.0, 0.18, sr, 0.12, 0.0),
            tone(523.0, 0.22, sr, 0.12, 0.0),
            tone(659.0, 0.28, sr, 0.12, 0.0),
        ]),
        "drop" | "return" => tone(330.0, 0.1, sr, 0.12, -40.0),
        "start" => tone(196.0, 0.3, sr, 0.14, 80.0),
        "end" => tone(262.0, 0.4, sr, 0.14, -60.0),
        _ => Vec::new(),
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

    fn play_web(&mut self, name: &str) {
        self.ensure();
        let (Some(ctx), Some(dest)) = (&self.ctx, &self.master) else {
            return;
        };
        let now = ctx.current_time();
        match name {
            "disc" => {
                beep(ctx, dest, 180.0, 0.16, now, -80.0);
                burst(ctx, dest, 0.12, 0.22, now);
            }
            "bolt" => {
                beep(ctx, dest, 1400.0, 0.05, now, 400.0);
                burst(ctx, dest, 0.04, 0.1, now);
            }
            "boom" => {
                beep(ctx, dest, 70.0, 0.22, now, -30.0);
                burst(ctx, dest, 0.28, 0.35, now);
            }
            "hit" => beep(ctx, dest, 980.0, 0.04, now, 0.0),
            "pain" => burst(ctx, dest, 0.12, 0.2, now),
            "death" => beep(ctx, dest, 220.0, 0.4, now, -160.0),
            "flag" => {
                beep(ctx, dest, 520.0, 0.12, now, 0.0);
                beep(ctx, dest, 780.0, 0.16, now, 0.0);
            }
            "capture" => {
                beep(ctx, dest, 392.0, 0.18, now, 0.0);
                beep(ctx, dest, 523.0, 0.22, now, 0.0);
                beep(ctx, dest, 659.0, 0.28, now, 0.0);
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
            let rate = ctx.sample_rate() as u32;
            let Ok(buffer) = ctx.create_buffer(1, rate.max(1), ctx.sample_rate()) else {
                return;
            };
            let mut data = vec![0.0f32; rate.max(1) as usize];
            let mut n = 0x51u32;
            for s in &mut data {
                n = n.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                *s = ((n >> 8) as f32 / 16_777_216.0) * 2.0 - 1.0;
            }
            let _ = buffer.copy_to_channel(&data, 0);
            let Ok(src) = ctx.create_buffer_source() else { return };
            src.set_buffer(Some(&buffer));
            src.set_loop(true);
            let Ok(gain) = ctx.create_gain() else { return };
            gain.gain().set_value(0.0001);
            let _ = gain.gain().set_target_at_time(0.06, ctx.current_time(), 0.05);
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
                let _ = src.stop_with_when(ctx.current_time());
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
