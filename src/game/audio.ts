let ctx: AudioContext | null = null;
let master: GainNode | null = null;
let sfx: GainNode | null = null;
let jet: { src: AudioBufferSourceNode; gain: GainNode } | null = null;

function ac(): AudioContext | null {
  if (typeof window === "undefined") return null;
  if (!ctx) {
    ctx = new AudioContext({ latencyHint: "interactive" });
    master = ctx.createGain();
    sfx = ctx.createGain();
    sfx.gain.value = 0.7;
    master.gain.value = 0.85;
    sfx.connect(master);
    master.connect(ctx.destination);
  }
  return ctx;
}

export function unlockAudio() {
  const c = ac();
  if (c && c.state === "suspended") void c.resume();
}

export function setMuted(muted: boolean) {
  if (!master || !ctx) return;
  master.gain.setTargetAtTime(muted ? 0 : 0.85, ctx.currentTime, 0.02);
}

function noise(c: AudioContext, dur: number): AudioBuffer {
  const n = Math.floor(c.sampleRate * dur);
  const buf = c.createBuffer(1, n, c.sampleRate);
  const d = buf.getChannelData(0);
  for (let i = 0; i < n; i++) d[i] = Math.random() * 2 - 1;
  return buf;
}

function beep(
  freq: number,
  dur: number,
  type: OscillatorType,
  gain = 0.12,
  slide = 0,
) {
  const c = ac();
  if (!c || !sfx) return;
  const o = c.createOscillator();
  const g = c.createGain();
  o.type = type;
  o.frequency.value = freq;
  if (slide) o.frequency.exponentialRampToValueAtTime(Math.max(40, freq + slide), c.currentTime + dur);
  g.gain.setValueAtTime(gain, c.currentTime);
  g.gain.exponentialRampToValueAtTime(0.001, c.currentTime + dur);
  o.connect(g);
  g.connect(sfx);
  o.start();
  o.stop(c.currentTime + dur);
}

function burst(dur: number, gain: number, hp: number, lp: number) {
  const c = ac();
  if (!c || !sfx) return;
  const src = c.createBufferSource();
  src.buffer = noise(c, dur);
  const filter = c.createBiquadFilter();
  filter.type = "bandpass";
  filter.frequency.value = (hp + lp) * 0.5;
  filter.Q.value = 0.7;
  const g = c.createGain();
  g.gain.setValueAtTime(gain, c.currentTime);
  g.gain.exponentialRampToValueAtTime(0.001, c.currentTime + dur);
  src.connect(filter);
  filter.connect(g);
  g.connect(sfx);
  src.start();
}

export function playEvent(name: string) {
  unlockAudio();
  switch (name) {
    case "disc":
      burst(0.12, 0.28, 200, 900);
      beep(180, 0.16, "sawtooth", 0.08, -80);
      break;
    case "bolt":
      beep(1400, 0.05, "square", 0.05, 400);
      burst(0.04, 0.12, 1200, 3000);
      break;
    case "boom":
      burst(0.28, 0.45, 80, 400);
      beep(70, 0.22, "sine", 0.16, -30);
      break;
    case "hit":
      beep(980, 0.04, "square", 0.07);
      break;
    case "pain":
      burst(0.12, 0.22, 120, 500);
      break;
    case "death":
      beep(220, 0.4, "sawtooth", 0.1, -160);
      break;
    case "flag":
      beep(520, 0.12, "triangle", 0.08);
      beep(780, 0.16, "triangle", 0.07);
      break;
    case "capture":
      beep(392, 0.18, "triangle", 0.1);
      beep(523, 0.22, "triangle", 0.1);
      beep(659, 0.28, "triangle", 0.1);
      break;
    case "drop":
    case "return":
      beep(330, 0.1, "sine", 0.07, -40);
      break;
    case "start":
      beep(196, 0.3, "sine", 0.08, 80);
      break;
    case "end":
      beep(262, 0.4, "triangle", 0.1, -60);
      break;
    default:
      break;
  }
}

export function setJet(on: boolean) {
  const c = ac();
  if (!c || !sfx) return;
  if (on && !jet) {
    const src = c.createBufferSource();
    src.buffer = noise(c, 1);
    src.loop = true;
    const filter = c.createBiquadFilter();
    filter.type = "highpass";
    filter.frequency.value = 900;
    const g = c.createGain();
    g.gain.value = 0.0001;
    g.gain.setTargetAtTime(0.06, c.currentTime, 0.05);
    src.connect(filter);
    filter.connect(g);
    g.connect(sfx);
    src.start();
    jet = { src, gain: g };
  } else if (!on && jet) {
    jet.gain.gain.setTargetAtTime(0.0001, c.currentTime, 0.04);
    const old = jet;
    jet = null;
    window.setTimeout(() => {
      try {
        old.src.stop();
      } catch {
        /* already stopped */
      }
    }, 120);
  }
}
