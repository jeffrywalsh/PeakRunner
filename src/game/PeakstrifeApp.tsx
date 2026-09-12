import { useEffect, useRef, useState } from "react";
import { Flag, Pause, Volume2, VolumeX, Zap } from "lucide-react";
import { playEvent, setJet, setMuted, unlockAudio } from "./audio";
import type { Game } from "../wasm/peakstrife";

type Mode = "menu" | "play" | "pause" | "end";

export type Hud = {
  health: number;
  energy: number;
  speed: number;
  yaw: number;
  ember: number;
  glacier: number;
  time: number;
  state: number;
  weapon: number;
  flag: number;
  ownFlag: number;
  hit: number;
  flash: number;
  msg: string;
  kills: number;
  deaths: number;
  winner: number;
  team: number;
  ski: number;
  jet: number;
  alive: number;
  cd: number;
  events: string;
  blips: string;
};

const EMPTY: Hud = {
  health: 100,
  energy: 100,
  speed: 0,
  yaw: 0,
  ember: 0,
  glacier: 0,
  time: 480,
  state: 0,
  weapon: 0,
  flag: -1,
  ownFlag: 1,
  hit: 0,
  flash: 0,
  msg: "",
  kills: 0,
  deaths: 0,
  winner: -2,
  team: 0,
  ski: 0,
  jet: 0,
  alive: 1,
  cd: 0,
  events: "",
  blips: "",
};

declare global {
  interface Window {
    __controlsTest?: {
      getYaw: () => number;
      getSpeed: () => number;
      getPos: () => { x: number; y: number; z: number };
      setKeys: (codes: string[]) => void;
    };
  }
}

export function PeakstrifeApp() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const gameRef = useRef<Game | null>(null);
  const keysRef = useRef<Set<string>>(new Set());
  const fireRef = useRef(false);
  const weaponRef = useRef(0);
  const stickRef = useRef({ x: 0, z: 0 });
  const lookStickRef = useRef({ x: 0, y: 0 });
  const touchLookRef = useRef<{ id: number; x: number; y: number } | null>(null);
  const moveTouchRef = useRef<{ id: number; x: number; y: number } | null>(null);
  const hadLockRef = useRef(false);
  const modeRef = useRef<Mode>("menu");
  const hudRef = useRef<Hud>(EMPTY);
  const [mode, setMode] = useState<Mode>("menu");
  const [ready, setReady] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [ember, setEmber] = useState(true);
  const [muted, setMutedState] = useState(false);
  const [hud, setHud] = useState<Hud>(EMPTY);
  const [isTouch, setIsTouch] = useState(false);

  useEffect(() => {
    modeRef.current = mode;
  }, [mode]);

  useEffect(() => {
    const syncTouch = () => {
      const coarse = window.matchMedia("(pointer: coarse)").matches;
      const small = window.matchMedia("(max-width: 640px)").matches;
      setIsTouch(coarse || small);
    };
    syncTouch();
    let cancelled = false;
    let raf = 0;
    let last = performance.now();

    async function boot() {
      const canvas = canvasRef.current;
      if (!canvas) return;
      try {
        const wasmUrl = (await import("../wasm/peakstrife_bg.wasm?url")).default;
        const mod = await import("../wasm/peakstrife.js");
        await mod.default({ module_or_path: wasmUrl });
        if (cancelled) return;
        const game = new mod.Game(canvas);
        gameRef.current = game;
        sizeToCanvas();
        setReady(true);

        window.__controlsTest = {
          getYaw: () => game.get_yaw(),
          getSpeed: () => game.get_speed(),
          getPos: () => ({ x: game.get_px(), y: game.get_py(), z: game.get_pz() }),
          setKeys: (codes: string[]) => {
            game.set_keys(codes);
          },
        };

        const qa = new URLSearchParams(window.location.search).has("qa");
        if (qa) {
          game.start_match(true);
          setMode("play");
          modeRef.current = "play";
        }

        const loop = (t: number) => {
          raf = requestAnimationFrame(loop);
          const dt = Math.min((t - last) / 1000, 0.1);
          last = t;
          const g = gameRef.current;
          if (!g) return;
          pollGamepad();
          const keys = keysRef.current;
          let mx = stickRef.current.x + gpMove.x;
          let mz = stickRef.current.z + gpMove.z;
          if (keys.has("KeyA") || keys.has("ArrowLeft")) mx -= 1;
          if (keys.has("KeyD") || keys.has("ArrowRight")) mx += 1;
          if (keys.has("KeyW") || keys.has("ArrowUp")) mz += 1;
          if (keys.has("KeyS") || keys.has("ArrowDown")) mz -= 1;
          const jump = keys.has("Space") || keys.has("JumpTouch") || gpMove.jump;
          if (keys.has("Digit1")) weaponRef.current = 0;
          if (keys.has("Digit2")) weaponRef.current = 1;
          lookStickRef.current = { x: gpMove.lx, y: gpMove.ly };
          g.set_input(
            Math.max(-1, Math.min(1, mx)),
            Math.max(-1, Math.min(1, mz)),
            jump,
            fireRef.current || gpMove.fire,
            weaponRef.current,
            lookStickRef.current.x,
            lookStickRef.current.y,
          );
          const raw = g.frame(dt);
          const next = JSON.parse(raw) as Hud;
          hudRef.current = next;
          if (next.events) {
            for (const e of next.events.split(",")) playEvent(e);
          }
          setJet(next.jet === 1);
          if (next.state === 3 && modeRef.current === "play") {
            setMode("end");
            modeRef.current = "end";
          }
          const now = performance.now();
          if (now - lastHud > 80) {
            lastHud = now;
            setHud(next);
          }
        };
        raf = requestAnimationFrame(loop);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        setLoadError(msg);
      }
    }

    let lastHud = 0;
    void boot();

    const onResize = () => {
      sizeToCanvas();
      syncTouch();
    };
    window.addEventListener("resize", onResize);
    return () => {
      cancelled = true;
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", onResize);
      window.__controlsTest = undefined;
      gameRef.current?.free();
      gameRef.current = null;
    };
  }, []);

  function sizeToCanvas() {
    const canvas = canvasRef.current;
    const wrap = wrapRef.current;
    const game = gameRef.current;
    if (!canvas || !wrap) return;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const w = Math.max(1, Math.floor(wrap.clientWidth * dpr));
    const h = Math.max(1, Math.floor(wrap.clientHeight * dpr));
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
      game?.resize(w, h);
    }
  }

    const gpMove = { x: 0, z: 0, lx: 0, ly: 0, jump: false, fire: false };

    function pollGamepad() {
      gpMove.x = 0;
      gpMove.z = 0;
      gpMove.lx = 0;
      gpMove.ly = 0;
      gpMove.jump = false;
      gpMove.fire = false;
      const pads = navigator.getGamepads?.() ?? [];
      const pad = pads.find((p) => p && p.mapping === "standard");
      if (!pad) return;
      const dz = (x: number, y: number) => {
        const m = Math.hypot(x, y);
        if (m < 0.15) return { x: 0, y: 0 };
        const s = (m - 0.15) / 0.85 / m;
        return { x: x * s, y: y * s };
      };
      const l = dz(pad.axes[0] ?? 0, pad.axes[1] ?? 0);
      const r = dz(pad.axes[2] ?? 0, pad.axes[3] ?? 0);
      gpMove.x = l.x;
      gpMove.z = -l.y;
      gpMove.lx = r.x;
      gpMove.ly = r.y;
      gpMove.jump = !!(pad.buttons[0]?.pressed || pad.buttons[6]?.pressed);
      gpMove.fire = !!(pad.buttons[7]?.pressed || pad.buttons[5]?.pressed);
    }

  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.code === "Escape") {
        if (modeRef.current === "play") pause();
        else if (modeRef.current === "pause") resume();
        return;
      }
      keysRef.current.add(e.code);
      if (["Space", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(e.code)) {
        e.preventDefault();
      }
    };
    const up = (e: KeyboardEvent) => {
      keysRef.current.delete(e.code);
    };
    const blur = () => {
      keysRef.current.clear();
      fireRef.current = false;
    };
    const move = (e: MouseEvent) => {
      if (document.pointerLockElement && gameRef.current && modeRef.current === "play") {
        gameRef.current.add_look(e.movementX, e.movementY);
      }
    };
    const mouseDown = (e: MouseEvent) => {
      if (e.button === 0 && modeRef.current === "play") fireRef.current = true;
    };
    const mouseUp = () => {
      fireRef.current = false;
    };
    const lockChange = () => {
      if (document.pointerLockElement) {
        hadLockRef.current = true;
        return;
      }
      if (hadLockRef.current && modeRef.current === "play") {
        hadLockRef.current = false;
        pause();
      }
    };
    window.addEventListener("keydown", down);
    window.addEventListener("keyup", up);
    window.addEventListener("blur", blur);
    document.addEventListener("visibilitychange", blur);
    window.addEventListener("mousemove", move);
    window.addEventListener("mousedown", mouseDown);
    window.addEventListener("mouseup", mouseUp);
    document.addEventListener("pointerlockchange", lockChange);
    return () => {
      window.removeEventListener("keydown", down);
      window.removeEventListener("keyup", up);
      window.removeEventListener("blur", blur);
      document.removeEventListener("visibilitychange", blur);
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mousedown", mouseDown);
      window.removeEventListener("mouseup", mouseUp);
      document.removeEventListener("pointerlockchange", lockChange);
    };
  }, []);

  function requestLock() {
    const canvas = canvasRef.current;
    if (!canvas || isTouch) return;
    const req = canvas.requestPointerLock as (opts?: { unadjustedMovement?: boolean }) => Promise<void> | void;
    try {
      const p = req.call(canvas, { unadjustedMovement: true });
      if (p && typeof (p as Promise<void>).catch === "function") {
        (p as Promise<void>).catch(() => canvas.requestPointerLock());
      }
    } catch {
      try {
        canvas.requestPointerLock();
      } catch {
        /* ignore */
      }
    }
  }

  function startMatch() {
    unlockAudio();
    const g = gameRef.current;
    if (!g) return;
    g.start_match(ember);
    playEvent("start");
    setMode("play");
    modeRef.current = "play";
    requestLock();
  }

  function pause() {
    if (modeRef.current !== "play") return;
    hadLockRef.current = false;
    gameRef.current?.set_paused(true);
    document.exitPointerLock?.();
    setMode("pause");
    modeRef.current = "pause";
  }

  function resume() {
    gameRef.current?.set_paused(false);
    setMode("play");
    modeRef.current = "play";
    requestLock();
  }

  function toMenu() {
    document.exitPointerLock?.();
    setMode("menu");
    modeRef.current = "menu";
  }

  function onPointerDown(e: React.PointerEvent) {
    if (mode !== "play") return;
    const rect = wrapRef.current?.getBoundingClientRect();
    if (!rect) return;
    const x = e.clientX - rect.left;
    const left = x < rect.width * 0.42;
    if (left && !moveTouchRef.current) {
      moveTouchRef.current = { id: e.pointerId, x: e.clientX, y: e.clientY };
      (e.target as HTMLElement).setPointerCapture?.(e.pointerId);
    } else if (!left && !touchLookRef.current) {
      touchLookRef.current = { id: e.pointerId, x: e.clientX, y: e.clientY };
      (e.target as HTMLElement).setPointerCapture?.(e.pointerId);
    }
  }
  function onPointerMove(e: React.PointerEvent) {
    if (moveTouchRef.current?.id === e.pointerId) {
      const dx = (e.clientX - moveTouchRef.current.x) / 56;
      const dy = (e.clientY - moveTouchRef.current.y) / 56;
      stickRef.current = {
        x: Math.max(-1, Math.min(1, dx)),
        z: Math.max(-1, Math.min(1, -dy)),
      };
    }
    if (touchLookRef.current?.id === e.pointerId) {
      const dx = e.clientX - touchLookRef.current.x;
      const dy = e.clientY - touchLookRef.current.y;
      touchLookRef.current = { id: e.pointerId, x: e.clientX, y: e.clientY };
      gameRef.current?.add_look(dx * 1.6, dy * 1.6);
    }
  }
  function onPointerUp(e: React.PointerEvent) {
    if (moveTouchRef.current?.id === e.pointerId) {
      moveTouchRef.current = null;
      stickRef.current = { x: 0, z: 0 };
    }
    if (touchLookRef.current?.id === e.pointerId) {
      touchLookRef.current = null;
    }
  }

  const kph = Math.round(hud.speed * 3.6);
  const mm = Math.floor(hud.time / 60);
  const ss = String(Math.floor(hud.time % 60)).padStart(2, "0");
  const teamEmber = hud.team === 0;

  return (
    <main
      ref={wrapRef}
      className="relative h-dvh w-full overflow-hidden bg-bg text-fg touch-none select-none"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
    >
      <canvas ref={canvasRef} className="absolute inset-0 h-full w-full" />

      {mode === "menu" && (
        <Menu
          ready={ready}
          error={loadError}
          ember={ember}
          onTeam={setEmber}
          onStart={startMatch}
        />
      )}

      {mode === "play" && (
        <HudOverlay
          hud={hud}
          kph={kph}
          clock={`${mm}:${ss}`}
          teamEmber={teamEmber}
          isTouch={isTouch}
        />
      )}

      {mode === "play" && isTouch && (
        <TouchControls
          onJump={(v) => {
            if (v) keysRef.current.add("JumpTouch");
            else keysRef.current.delete("JumpTouch");
          }}
          onFire={(v) => {
            fireRef.current = v;
          }}
          onSwap={() => {
            weaponRef.current = weaponRef.current === 0 ? 1 : 0;
          }}
        />
      )}

      {mode === "pause" && (
        <PauseMenu
          onResume={resume}
          onMenu={toMenu}
          muted={muted}
          onMute={() => {
            setMutedState((m) => {
              setMuted(!m);
              return !m;
            });
          }}
        />
      )}

      {mode === "end" && (
        <EndScreen hud={hud} teamEmber={teamEmber} onAgain={startMatch} onMenu={toMenu} />
      )}

      {mode === "play" && hud.flash > 0 && (
        <div
          className="pointer-events-none absolute inset-0 bg-ember/30"
          style={{ opacity: hud.flash * 0.55 }}
        />
      )}
    </main>
  );
}

function Menu({
  ready,
  error,
  ember,
  onTeam,
  onStart,
}: {
  ready: boolean;
  error: string | null;
  ember: boolean;
  onTeam: (v: boolean) => void;
  onStart: () => void;
}) {
  return (
      <div className="absolute inset-0 flex flex-col justify-end">
      <img
        src="/hero.jpg"
        alt=""
        className={`pointer-events-none absolute inset-0 h-full w-full object-cover transition-opacity duration-500 ${ready ? "opacity-25" : "opacity-80"}`}
        crossOrigin="anonymous"
      />
      <div className="pointer-events-none absolute inset-0 bg-gradient-to-t from-bg via-bg/70 to-bg/20" />
      <div className="relative z-10 mx-auto flex w-full max-w-3xl flex-col gap-6 px-6 pb-10 pt-16 sm:pb-14">
        <p className="font-display text-sm font-medium tracking-[0.28em] text-glacier uppercase">
          Ski the ridgeline
        </p>
        <h1 className="font-display text-6xl font-semibold leading-none tracking-tight text-fg sm:text-8xl">
          PEAKSTRIFE
        </h1>
        <p className="max-w-md text-base leading-relaxed text-muted">
          High-speed ski and disc combat on a frozen rift. Hold jump to ski,
          jet the gaps, steal their flag, never touch the brakes.
        </p>
        <div className="flex flex-wrap gap-3">
          <button
            type="button"
            onClick={() => onTeam(true)}
            className={`h-11 rounded-sm border px-4 text-sm font-medium ${
              ember
                ? "border-ember bg-ember text-fg"
                : "border-border bg-surface text-muted"
            }`}
          >
            Ember
          </button>
          <button
            type="button"
            onClick={() => onTeam(false)}
            className={`h-11 rounded-sm border px-4 text-sm font-medium ${
              !ember
                ? "border-glacier bg-glacier text-bg"
                : "border-border bg-surface text-muted"
            }`}
          >
            Glacier
          </button>
        </div>
        <div className="flex flex-wrap items-center gap-4">
          <button
            type="button"
            disabled={!ready || !!error}
            onClick={onStart}
            className="h-12 min-w-44 rounded-md bg-fg px-8 font-display text-lg font-semibold tracking-wide text-bg transition-transform duration-150 hover:opacity-90 active:scale-[0.98] disabled:opacity-40"
          >
            {ready ? "Start match" : "Loading engine"}
          </button>
          <p className="text-xs tracking-wide text-muted uppercase">
            Rust / WebAssembly sim
          </p>
        </div>
        {error && <p className="text-sm text-ember">{error}</p>}
        <dl className="grid grid-cols-2 gap-x-6 gap-y-2 text-sm text-muted sm:grid-cols-4">
          <div>
            <dt className="text-xs tracking-wider uppercase">Move</dt>
            <dd className="text-fg">WASD</dd>
          </div>
          <div>
            <dt className="text-xs tracking-wider uppercase">Look</dt>
            <dd className="text-fg">Mouse</dd>
          </div>
          <div>
            <dt className="text-xs tracking-wider uppercase">Ski / Jet</dt>
            <dd className="text-fg">Space</dd>
          </div>
          <div>
            <dt className="text-xs tracking-wider uppercase">Fire</dt>
            <dd className="text-fg">Click · 1/2 weapons</dd>
          </div>
        </dl>
      </div>
    </div>
  );
}

function HudOverlay({
  hud,
  kph,
  clock,
  teamEmber,
  isTouch,
}: {
  hud: Hud;
  kph: number;
  clock: string;
  teamEmber: boolean;
  isTouch: boolean;
}) {
  return (
    <div className="pointer-events-none absolute inset-0 p-4 sm:p-6">
      <div className="flex items-start justify-between gap-4">
        <div className="rounded-md border border-border bg-bg/70 px-3 py-2">
          <p className="font-display text-xs tracking-[0.2em] text-muted uppercase">Speed</p>
          <p className="font-display text-4xl font-semibold tabular-nums leading-none">
            {kph}
            <span className="ml-1 text-base text-muted">kph</span>
          </p>
          <p className="mt-1 text-xs uppercase tracking-wider text-muted">
            {hud.ski ? "Skiing" : hud.jet ? "Jet" : hud.alive ? "Planted" : "Down"}
          </p>
        </div>
        <div className="flex flex-col items-center gap-1">
          <div className="flex items-center gap-3 rounded-md border border-border bg-bg/70 px-4 py-2 font-display text-2xl font-semibold tabular-nums">
            <span className="text-ember">{hud.ember}</span>
            <span className="text-muted text-sm">{clock}</span>
            <span className="text-glacier">{hud.glacier}</span>
          </div>
          <p className="text-xs tracking-widest text-muted uppercase">
            First to 3 captures
          </p>
        </div>
        <Minimap blips={hud.blips} yaw={hud.yaw} team={hud.team} />
      </div>

      <div className="absolute left-1/2 top-24 -translate-x-1/2 text-center">
        {hud.msg && (
          <p className="font-display text-2xl font-semibold tracking-wide text-fg drop-shadow">
            {hud.msg}
          </p>
        )}
        {hud.flag >= 0 && (
          <p className="mt-1 flex items-center justify-center gap-2 text-sm text-ember">
            <Flag className="size-4" /> You have the flag
          </p>
        )}
        {hud.ownFlag === 2 && hud.flag < 0 && (
          <p className="mt-1 text-sm text-ember">Your flag is taken</p>
        )}
      </div>

      <div
        className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2"
        style={{ opacity: hud.alive ? 1 : 0.2 }}
      >
        <span className={`absolute -left-3 top-0 h-px w-2 ${hud.hit > 0 ? "bg-ember" : "bg-fg"}`} />
        <span className={`absolute -right-3 top-0 h-px w-2 ${hud.hit > 0 ? "bg-ember" : "bg-fg"}`} />
        <span className={`absolute left-0 -top-3 w-px h-2 ${hud.hit > 0 ? "bg-ember" : "bg-fg"}`} />
        <span className={`absolute left-0 -bottom-3 w-px h-2 ${hud.hit > 0 ? "bg-ember" : "bg-fg"}`} />
      </div>

      <div className="absolute bottom-6 left-4 right-4 flex items-end justify-between gap-4 sm:left-6 sm:right-6">
        <div className="w-44 space-y-2 sm:w-56">
          <Bar label="Armor" value={hud.health} color="bg-ember" />
          <Bar label="Energy" value={hud.energy} color="bg-glacier" />
        </div>
        <div className="text-center">
          <p className="font-display text-xl font-semibold tracking-wide">
            {hud.weapon === 0 ? "Disc" : "Repeater"}
          </p>
          <p className="text-xs text-muted tabular-nums">
            {hud.kills} frag · {hud.deaths} down
          </p>
        </div>
        {!isTouch && (
          <p className="hidden w-44 text-right text-xs text-muted sm:block">
            Space ski/jet · ESC pause
            <br />
            {teamEmber ? "Ember" : "Glacier"}
          </p>
        )}
      </div>
    </div>
  );
}

function Bar({ label, value, color }: { label: string; value: number; color: string }) {
  return (
    <div>
      <div className="mb-1 flex justify-between text-xs tracking-wider text-muted uppercase">
        <span>{label}</span>
        <span className="tabular-nums text-fg">{Math.round(value)}</span>
      </div>
      <div className="h-1.5 overflow-hidden rounded-full bg-elevated">
        <div className={`h-full ${color}`} style={{ width: `${Math.max(0, Math.min(100, value))}%` }} />
      </div>
    </div>
  );
}

function Minimap({ blips, yaw, team }: { blips: string; yaw: number; team: number }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = ref.current;
    if (!c) return;
    const ctx = c.getContext("2d");
    if (!ctx) return;
    const s = 112;
    c.width = s;
    c.height = s;
    ctx.fillStyle = "#080d14";
    ctx.fillRect(0, 0, s, s);
    ctx.strokeStyle = "rgba(231,238,246,0.16)";
    ctx.strokeRect(0.5, 0.5, s - 1, s - 1);
    ctx.save();
    ctx.translate(s / 2, s / 2);
    ctx.rotate(-yaw);
    ctx.translate(-s / 2, -s / 2);
    for (const part of blips.split(";")) {
      if (!part) continue;
      const [xs, zs, ts, kind] = part.split(",");
      const x = ((Number(xs) / 256) * (s - 12)) + 6;
      const z = ((Number(zs) / 256) * (s - 12)) + 6;
      const ember = Number(ts) === 0;
      ctx.fillStyle = ember ? "#e24a32" : "#3ec8e0";
      if (kind === "2") {
        ctx.fillRect(x - 3, z - 3, 6, 6);
      } else if (kind === "1") {
        ctx.beginPath();
        ctx.arc(x, z, 4, 0, Math.PI * 2);
        ctx.fill();
        ctx.strokeStyle = "#e7eef6";
        ctx.stroke();
      } else {
        ctx.beginPath();
        ctx.arc(x, z, 2.4, 0, Math.PI * 2);
        ctx.fill();
      }
    }
    ctx.restore();
    const _ = team;
  }, [blips, yaw, team]);
  return <canvas ref={ref} className="h-24 w-24 rounded-md border border-border bg-bg/70" />;
}

function TouchControls({
  onJump,
  onFire,
  onSwap,
}: {
  onJump: (v: boolean) => void;
  onFire: (v: boolean) => void;
  onSwap: () => void;
}) {
  return (
    <div className="pointer-events-none absolute inset-x-0 bottom-4 flex items-end justify-between px-4">
      <div className="pointer-events-none h-28 w-28 rounded-full border border-border bg-surface/40" />
      <div className="pointer-events-auto flex gap-3">
        <button
          type="button"
          className="flex h-14 w-14 items-center justify-center rounded-full border border-border bg-surface/80 text-xs font-medium"
          onPointerDown={onSwap}
        >
          Swap
        </button>
        <button
          type="button"
          className="flex h-16 w-16 items-center justify-center rounded-full border border-glacier bg-surface/80"
          onPointerDown={() => onJump(true)}
          onPointerUp={() => onJump(false)}
          onPointerCancel={() => onJump(false)}
        >
          <Zap className="size-5 text-glacier" />
        </button>
        <button
          type="button"
          className="flex h-16 w-16 items-center justify-center rounded-full border border-ember bg-ember/80 font-display text-sm font-semibold"
          onPointerDown={() => onFire(true)}
          onPointerUp={() => onFire(false)}
          onPointerCancel={() => onFire(false)}
        >
          Fire
        </button>
      </div>
    </div>
  );
}

function PauseMenu({
  onResume,
  onMenu,
  muted,
  onMute,
}: {
  onResume: () => void;
  onMenu: () => void;
  muted: boolean;
  onMute: () => void;
}) {
  return (
    <div className="absolute inset-0 flex items-center justify-center bg-bg/70 p-6">
      <div className="w-full max-w-sm rounded-xl border border-border bg-surface p-6 shadow-lg">
        <div className="mb-6 flex items-center gap-2 text-muted">
          <Pause className="size-4" />
          <h2 className="font-display text-2xl font-semibold tracking-wide text-fg">Paused</h2>
        </div>
        <div className="flex flex-col gap-3">
          <button
            type="button"
            onClick={onResume}
            className="h-11 rounded-md bg-fg font-medium text-bg"
          >
            Resume
          </button>
          <button
            type="button"
            onClick={onMute}
            className="flex h-11 items-center justify-center gap-2 rounded-md border border-border text-fg"
          >
            {muted ? <VolumeX className="size-4" /> : <Volume2 className="size-4" />}
            {muted ? "Unmute" : "Mute"}
          </button>
          <button
            type="button"
            onClick={onMenu}
            className="h-11 rounded-md border border-border text-muted"
          >
            Leave rift
          </button>
        </div>
      </div>
    </div>
  );
}

function EndScreen({
  hud,
  teamEmber,
  onAgain,
  onMenu,
}: {
  hud: Hud;
  teamEmber: boolean;
  onAgain: () => void;
  onMenu: () => void;
}) {
  const win =
    hud.winner === -1 ? "Draw" : hud.winner === hud.team ? "Victory" : "Defeat";
  return (
    <div className="absolute inset-0 flex items-center justify-center bg-bg/75 p-6">
      <div className="w-full max-w-md rounded-xl border border-border bg-surface p-8 text-center">
        <p className="font-display text-xs tracking-[0.28em] text-muted uppercase">Match over</p>
        <h2 className="mt-2 font-display text-5xl font-semibold tracking-tight">{win}</h2>
        <p className="mt-4 font-display text-3xl tabular-nums">
          <span className="text-ember">{hud.ember}</span>
          <span className="mx-3 text-muted">–</span>
          <span className="text-glacier">{hud.glacier}</span>
        </p>
        <p className="mt-2 text-sm text-muted">
          {hud.kills} frags · {hud.deaths} deaths · {teamEmber ? "Ember" : "Glacier"}
        </p>
        <div className="mt-8 flex flex-col gap-3">
          <button type="button" onClick={onAgain} className="h-11 rounded-md bg-fg font-medium text-bg">
            Start match
          </button>
          <button type="button" onClick={onMenu} className="h-11 rounded-md border border-border text-muted">
            Menu
          </button>
        </div>
      </div>
    </div>
  );
}
