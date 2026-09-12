import { i as __toESM } from "../_runtime.mjs";
import { L as require_react, v as require_jsx_runtime } from "../_libs/@tanstack/react-router+[...].mjs";
import { a as Pause, n as VolumeX, o as Flag, r as Volume2, t as Zap } from "../_libs/lucide-react.mjs";
//#region node_modules/.nitro/vite/services/ssr/assets/routes-DT2wWwma.js
var import_react = /* @__PURE__ */ __toESM(require_react());
var import_jsx_runtime = require_jsx_runtime();
var ctx = null;
var master = null;
var sfx = null;
var jet = null;
function ac() {
	if (typeof window === "undefined") return null;
	if (!ctx) {
		ctx = new AudioContext({ latencyHint: "interactive" });
		master = ctx.createGain();
		sfx = ctx.createGain();
		sfx.gain.value = .7;
		master.gain.value = .85;
		sfx.connect(master);
		master.connect(ctx.destination);
	}
	return ctx;
}
function unlockAudio() {
	const c = ac();
	if (c && c.state === "suspended") c.resume();
}
function setMuted(muted) {
	if (!master || !ctx) return;
	master.gain.setTargetAtTime(muted ? 0 : .85, ctx.currentTime, .02);
}
function noise(c, dur) {
	const n = Math.floor(c.sampleRate * dur);
	const buf = c.createBuffer(1, n, c.sampleRate);
	const d = buf.getChannelData(0);
	for (let i = 0; i < n; i++) d[i] = Math.random() * 2 - 1;
	return buf;
}
function beep(freq, dur, type, gain = .12, slide = 0) {
	const c = ac();
	if (!c || !sfx) return;
	const o = c.createOscillator();
	const g = c.createGain();
	o.type = type;
	o.frequency.value = freq;
	if (slide) o.frequency.exponentialRampToValueAtTime(Math.max(40, freq + slide), c.currentTime + dur);
	g.gain.setValueAtTime(gain, c.currentTime);
	g.gain.exponentialRampToValueAtTime(.001, c.currentTime + dur);
	o.connect(g);
	g.connect(sfx);
	o.start();
	o.stop(c.currentTime + dur);
}
function burst(dur, gain, hp, lp) {
	const c = ac();
	if (!c || !sfx) return;
	const src = c.createBufferSource();
	src.buffer = noise(c, dur);
	const filter = c.createBiquadFilter();
	filter.type = "bandpass";
	filter.frequency.value = (hp + lp) * .5;
	filter.Q.value = .7;
	const g = c.createGain();
	g.gain.setValueAtTime(gain, c.currentTime);
	g.gain.exponentialRampToValueAtTime(.001, c.currentTime + dur);
	src.connect(filter);
	filter.connect(g);
	g.connect(sfx);
	src.start();
}
function playEvent(name) {
	unlockAudio();
	switch (name) {
		case "disc":
			burst(.12, .28, 200, 900);
			beep(180, .16, "sawtooth", .08, -80);
			break;
		case "bolt":
			beep(1400, .05, "square", .05, 400);
			burst(.04, .12, 1200, 3e3);
			break;
		case "boom":
			burst(.28, .45, 80, 400);
			beep(70, .22, "sine", .16, -30);
			break;
		case "hit":
			beep(980, .04, "square", .07);
			break;
		case "pain":
			burst(.12, .22, 120, 500);
			break;
		case "death":
			beep(220, .4, "sawtooth", .1, -160);
			break;
		case "flag":
			beep(520, .12, "triangle", .08);
			beep(780, .16, "triangle", .07);
			break;
		case "capture":
			beep(392, .18, "triangle", .1);
			beep(523, .22, "triangle", .1);
			beep(659, .28, "triangle", .1);
			break;
		case "drop":
		case "return":
			beep(330, .1, "sine", .07, -40);
			break;
		case "start":
			beep(196, .3, "sine", .08, 80);
			break;
		case "end": beep(262, .4, "triangle", .1, -60);
	}
}
function setJet(on) {
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
		g.gain.value = 1e-4;
		g.gain.setTargetAtTime(.06, c.currentTime, .05);
		src.connect(filter);
		filter.connect(g);
		g.connect(sfx);
		src.start();
		jet = {
			src,
			gain: g
		};
	} else if (!on && jet) {
		jet.gain.gain.setTargetAtTime(1e-4, c.currentTime, .04);
		const old = jet;
		jet = null;
		window.setTimeout(() => {
			try {
				old.src.stop();
			} catch {}
		}, 120);
	}
}
var EMPTY = {
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
	blips: ""
};
function PeakstrifeApp() {
	const canvasRef = (0, import_react.useRef)(null);
	const wrapRef = (0, import_react.useRef)(null);
	const gameRef = (0, import_react.useRef)(null);
	const keysRef = (0, import_react.useRef)(/* @__PURE__ */ new Set());
	const fireRef = (0, import_react.useRef)(false);
	const weaponRef = (0, import_react.useRef)(0);
	const stickRef = (0, import_react.useRef)({
		x: 0,
		z: 0
	});
	const lookStickRef = (0, import_react.useRef)({
		x: 0,
		y: 0
	});
	const touchLookRef = (0, import_react.useRef)(null);
	const moveTouchRef = (0, import_react.useRef)(null);
	const hadLockRef = (0, import_react.useRef)(false);
	const modeRef = (0, import_react.useRef)("menu");
	const hudRef = (0, import_react.useRef)(EMPTY);
	const [mode, setMode] = (0, import_react.useState)("menu");
	const [ready, setReady] = (0, import_react.useState)(false);
	const [loadError, setLoadError] = (0, import_react.useState)(null);
	const [ember, setEmber] = (0, import_react.useState)(true);
	const [muted, setMutedState] = (0, import_react.useState)(false);
	const [hud, setHud] = (0, import_react.useState)(EMPTY);
	const [isTouch, setIsTouch] = (0, import_react.useState)(false);
	(0, import_react.useEffect)(() => {
		modeRef.current = mode;
	}, [mode]);
	(0, import_react.useEffect)(() => {
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
				const wasmUrl = (await import("./peakstrife_bg-DgA9K7mn.mjs")).default;
				const mod = await import("./peakstrife-t5fjXzLq.mjs");
				await mod.default({ module_or_path: wasmUrl });
				if (cancelled) return;
				const game = new mod.Game(canvas);
				gameRef.current = game;
				sizeToCanvas();
				setReady(true);
				window.__controlsTest = {
					getYaw: () => game.get_yaw(),
					getSpeed: () => game.get_speed(),
					getPos: () => ({
						x: game.get_px(),
						y: game.get_py(),
						z: game.get_pz()
					}),
					setKeys: (codes) => {
						game.set_keys(codes);
					}
				};
				if (new URLSearchParams(window.location.search).has("qa")) {
					game.start_match(true);
					setMode("play");
					modeRef.current = "play";
				}
				const loop = (t) => {
					raf = requestAnimationFrame(loop);
					const dt = Math.min((t - last) / 1e3, .1);
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
					lookStickRef.current = {
						x: gpMove.lx,
						y: gpMove.ly
					};
					g.set_input(Math.max(-1, Math.min(1, mx)), Math.max(-1, Math.min(1, mz)), jump, fireRef.current || gpMove.fire, weaponRef.current, lookStickRef.current.x, lookStickRef.current.y);
					const raw = g.frame(dt);
					const next = JSON.parse(raw);
					hudRef.current = next;
					if (next.events) for (const e of next.events.split(",")) playEvent(e);
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
		boot();
		const onResize = () => {
			sizeToCanvas();
			syncTouch();
		};
		window.addEventListener("resize", onResize);
		return () => {
			cancelled = true;
			cancelAnimationFrame(raf);
			window.removeEventListener("resize", onResize);
			window.__controlsTest = void 0;
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
	const gpMove = {
		x: 0,
		z: 0,
		lx: 0,
		ly: 0,
		jump: false,
		fire: false
	};
	function pollGamepad() {
		gpMove.x = 0;
		gpMove.z = 0;
		gpMove.lx = 0;
		gpMove.ly = 0;
		gpMove.jump = false;
		gpMove.fire = false;
		const pad = (navigator.getGamepads?.() ?? []).find((p) => p && p.mapping === "standard");
		if (!pad) return;
		const dz = (x, y) => {
			const m = Math.hypot(x, y);
			if (m < .15) return {
				x: 0,
				y: 0
			};
			const s = (m - .15) / .85 / m;
			return {
				x: x * s,
				y: y * s
			};
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
	(0, import_react.useEffect)(() => {
		const down = (e) => {
			if (e.code === "Escape") {
				if (modeRef.current === "play") pause();
				else if (modeRef.current === "pause") resume();
				return;
			}
			keysRef.current.add(e.code);
			if ([
				"Space",
				"ArrowUp",
				"ArrowDown",
				"ArrowLeft",
				"ArrowRight"
			].includes(e.code)) e.preventDefault();
		};
		const up = (e) => {
			keysRef.current.delete(e.code);
		};
		const blur = () => {
			keysRef.current.clear();
			fireRef.current = false;
		};
		const move = (e) => {
			if (document.pointerLockElement && gameRef.current && modeRef.current === "play") gameRef.current.add_look(e.movementX, e.movementY);
		};
		const mouseDown = (e) => {
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
		const req = canvas.requestPointerLock;
		try {
			const p = req.call(canvas, { unadjustedMovement: true });
			if (p && typeof p.catch === "function") p.catch(() => canvas.requestPointerLock());
		} catch {
			try {
				canvas.requestPointerLock();
			} catch {}
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
	function onPointerDown(e) {
		if (mode !== "play") return;
		const rect = wrapRef.current?.getBoundingClientRect();
		if (!rect) return;
		const left = e.clientX - rect.left < rect.width * .42;
		if (left && !moveTouchRef.current) {
			moveTouchRef.current = {
				id: e.pointerId,
				x: e.clientX,
				y: e.clientY
			};
			e.target.setPointerCapture?.(e.pointerId);
		} else if (!left && !touchLookRef.current) {
			touchLookRef.current = {
				id: e.pointerId,
				x: e.clientX,
				y: e.clientY
			};
			e.target.setPointerCapture?.(e.pointerId);
		}
	}
	function onPointerMove(e) {
		if (moveTouchRef.current?.id === e.pointerId) {
			const dx = (e.clientX - moveTouchRef.current.x) / 56;
			const dy = (e.clientY - moveTouchRef.current.y) / 56;
			stickRef.current = {
				x: Math.max(-1, Math.min(1, dx)),
				z: Math.max(-1, Math.min(1, -dy))
			};
		}
		if (touchLookRef.current?.id === e.pointerId) {
			const dx = e.clientX - touchLookRef.current.x;
			const dy = e.clientY - touchLookRef.current.y;
			touchLookRef.current = {
				id: e.pointerId,
				x: e.clientX,
				y: e.clientY
			};
			gameRef.current?.add_look(dx * 1.6, dy * 1.6);
		}
	}
	function onPointerUp(e) {
		if (moveTouchRef.current?.id === e.pointerId) {
			moveTouchRef.current = null;
			stickRef.current = {
				x: 0,
				z: 0
			};
		}
		if (touchLookRef.current?.id === e.pointerId) touchLookRef.current = null;
	}
	const kph = Math.round(hud.speed * 3.6);
	const mm = Math.floor(hud.time / 60);
	const ss = String(Math.floor(hud.time % 60)).padStart(2, "0");
	const teamEmber = hud.team === 0;
	return /* @__PURE__ */ (0, import_jsx_runtime.jsxs)("main", {
		ref: wrapRef,
		className: "relative h-dvh w-full overflow-hidden bg-bg text-fg touch-none select-none",
		onPointerDown,
		onPointerMove,
		onPointerUp,
		onPointerCancel: onPointerUp,
		children: [
			/* @__PURE__ */ (0, import_jsx_runtime.jsx)("canvas", {
				ref: canvasRef,
				className: "absolute inset-0 h-full w-full"
			}),
			mode === "menu" && /* @__PURE__ */ (0, import_jsx_runtime.jsx)(Menu, {
				ready,
				error: loadError,
				ember,
				onTeam: setEmber,
				onStart: startMatch
			}),
			mode === "play" && /* @__PURE__ */ (0, import_jsx_runtime.jsx)(HudOverlay, {
				hud,
				kph,
				clock: `${mm}:${ss}`,
				teamEmber,
				isTouch
			}),
			mode === "play" && isTouch && /* @__PURE__ */ (0, import_jsx_runtime.jsx)(TouchControls, {
				onJump: (v) => {
					if (v) keysRef.current.add("JumpTouch");
					else keysRef.current.delete("JumpTouch");
				},
				onFire: (v) => {
					fireRef.current = v;
				},
				onSwap: () => {
					weaponRef.current = weaponRef.current === 0 ? 1 : 0;
				}
			}),
			mode === "pause" && /* @__PURE__ */ (0, import_jsx_runtime.jsx)(PauseMenu, {
				onResume: resume,
				onMenu: toMenu,
				muted,
				onMute: () => {
					setMutedState((m) => {
						setMuted(!m);
						return !m;
					});
				}
			}),
			mode === "end" && /* @__PURE__ */ (0, import_jsx_runtime.jsx)(EndScreen, {
				hud,
				teamEmber,
				onAgain: startMatch,
				onMenu: toMenu
			}),
			mode === "play" && hud.flash > 0 && /* @__PURE__ */ (0, import_jsx_runtime.jsx)("div", {
				className: "pointer-events-none absolute inset-0 bg-ember/30",
				style: { opacity: hud.flash * .55 }
			})
		]
	});
}
function Menu({ ready, error, ember, onTeam, onStart }) {
	return /* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
		className: "absolute inset-0 flex flex-col justify-end",
		children: [
			/* @__PURE__ */ (0, import_jsx_runtime.jsx)("img", {
				src: "/hero.jpg",
				alt: "",
				className: `pointer-events-none absolute inset-0 h-full w-full object-cover transition-opacity duration-500 ${ready ? "opacity-25" : "opacity-80"}`,
				crossOrigin: "anonymous"
			}),
			/* @__PURE__ */ (0, import_jsx_runtime.jsx)("div", { className: "pointer-events-none absolute inset-0 bg-gradient-to-t from-bg via-bg/70 to-bg/20" }),
			/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
				className: "relative z-10 mx-auto flex w-full max-w-3xl flex-col gap-6 px-6 pb-10 pt-16 sm:pb-14",
				children: [
					/* @__PURE__ */ (0, import_jsx_runtime.jsx)("p", {
						className: "font-display text-sm font-medium tracking-[0.28em] text-glacier uppercase",
						children: "Ski the ridgeline"
					}),
					/* @__PURE__ */ (0, import_jsx_runtime.jsx)("h1", {
						className: "font-display text-6xl font-semibold leading-none tracking-tight text-fg sm:text-8xl",
						children: "PEAKSTRIFE"
					}),
					/* @__PURE__ */ (0, import_jsx_runtime.jsx)("p", {
						className: "max-w-md text-base leading-relaxed text-muted",
						children: "High-speed ski and disc combat on a frozen rift. Hold jump to ski, jet the gaps, steal their flag, never touch the brakes."
					}),
					/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
						className: "flex flex-wrap gap-3",
						children: [/* @__PURE__ */ (0, import_jsx_runtime.jsx)("button", {
							type: "button",
							onClick: () => onTeam(true),
							className: `h-11 rounded-sm border px-4 text-sm font-medium ${ember ? "border-ember bg-ember text-fg" : "border-border bg-surface text-muted"}`,
							children: "Ember"
						}), /* @__PURE__ */ (0, import_jsx_runtime.jsx)("button", {
							type: "button",
							onClick: () => onTeam(false),
							className: `h-11 rounded-sm border px-4 text-sm font-medium ${!ember ? "border-glacier bg-glacier text-bg" : "border-border bg-surface text-muted"}`,
							children: "Glacier"
						})]
					}),
					/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
						className: "flex flex-wrap items-center gap-4",
						children: [/* @__PURE__ */ (0, import_jsx_runtime.jsx)("button", {
							type: "button",
							disabled: !ready || !!error,
							onClick: onStart,
							className: "h-12 min-w-44 rounded-md bg-fg px-8 font-display text-lg font-semibold tracking-wide text-bg transition-transform duration-150 hover:opacity-90 active:scale-[0.98] disabled:opacity-40",
							children: ready ? "Start match" : "Loading engine"
						}), /* @__PURE__ */ (0, import_jsx_runtime.jsx)("p", {
							className: "text-xs tracking-wide text-muted uppercase",
							children: "Rust / WebAssembly sim"
						})]
					}),
					error && /* @__PURE__ */ (0, import_jsx_runtime.jsx)("p", {
						className: "text-sm text-ember",
						children: error
					}),
					/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("dl", {
						className: "grid grid-cols-2 gap-x-6 gap-y-2 text-sm text-muted sm:grid-cols-4",
						children: [
							/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", { children: [/* @__PURE__ */ (0, import_jsx_runtime.jsx)("dt", {
								className: "text-xs tracking-wider uppercase",
								children: "Move"
							}), /* @__PURE__ */ (0, import_jsx_runtime.jsx)("dd", {
								className: "text-fg",
								children: "WASD"
							})] }),
							/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", { children: [/* @__PURE__ */ (0, import_jsx_runtime.jsx)("dt", {
								className: "text-xs tracking-wider uppercase",
								children: "Look"
							}), /* @__PURE__ */ (0, import_jsx_runtime.jsx)("dd", {
								className: "text-fg",
								children: "Mouse"
							})] }),
							/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", { children: [/* @__PURE__ */ (0, import_jsx_runtime.jsx)("dt", {
								className: "text-xs tracking-wider uppercase",
								children: "Ski / Jet"
							}), /* @__PURE__ */ (0, import_jsx_runtime.jsx)("dd", {
								className: "text-fg",
								children: "Space"
							})] }),
							/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", { children: [/* @__PURE__ */ (0, import_jsx_runtime.jsx)("dt", {
								className: "text-xs tracking-wider uppercase",
								children: "Fire"
							}), /* @__PURE__ */ (0, import_jsx_runtime.jsx)("dd", {
								className: "text-fg",
								children: "Click · 1/2 weapons"
							})] })
						]
					})
				]
			})
		]
	});
}
function HudOverlay({ hud, kph, clock, teamEmber, isTouch }) {
	return /* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
		className: "pointer-events-none absolute inset-0 p-4 sm:p-6",
		children: [
			/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
				className: "flex items-start justify-between gap-4",
				children: [
					/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
						className: "rounded-md border border-border bg-bg/70 px-3 py-2",
						children: [
							/* @__PURE__ */ (0, import_jsx_runtime.jsx)("p", {
								className: "font-display text-xs tracking-[0.2em] text-muted uppercase",
								children: "Speed"
							}),
							/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("p", {
								className: "font-display text-4xl font-semibold tabular-nums leading-none",
								children: [kph, /* @__PURE__ */ (0, import_jsx_runtime.jsx)("span", {
									className: "ml-1 text-base text-muted",
									children: "kph"
								})]
							}),
							/* @__PURE__ */ (0, import_jsx_runtime.jsx)("p", {
								className: "mt-1 text-xs uppercase tracking-wider text-muted",
								children: hud.ski ? "Skiing" : hud.jet ? "Jet" : hud.alive ? "Planted" : "Down"
							})
						]
					}),
					/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
						className: "flex flex-col items-center gap-1",
						children: [/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
							className: "flex items-center gap-3 rounded-md border border-border bg-bg/70 px-4 py-2 font-display text-2xl font-semibold tabular-nums",
							children: [
								/* @__PURE__ */ (0, import_jsx_runtime.jsx)("span", {
									className: "text-ember",
									children: hud.ember
								}),
								/* @__PURE__ */ (0, import_jsx_runtime.jsx)("span", {
									className: "text-muted text-sm",
									children: clock
								}),
								/* @__PURE__ */ (0, import_jsx_runtime.jsx)("span", {
									className: "text-glacier",
									children: hud.glacier
								})
							]
						}), /* @__PURE__ */ (0, import_jsx_runtime.jsx)("p", {
							className: "text-xs tracking-widest text-muted uppercase",
							children: "First to 3 captures"
						})]
					}),
					/* @__PURE__ */ (0, import_jsx_runtime.jsx)(Minimap, {
						blips: hud.blips,
						yaw: hud.yaw,
						team: hud.team
					})
				]
			}),
			/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
				className: "absolute left-1/2 top-24 -translate-x-1/2 text-center",
				children: [
					hud.msg && /* @__PURE__ */ (0, import_jsx_runtime.jsx)("p", {
						className: "font-display text-2xl font-semibold tracking-wide text-fg drop-shadow",
						children: hud.msg
					}),
					hud.flag >= 0 && /* @__PURE__ */ (0, import_jsx_runtime.jsxs)("p", {
						className: "mt-1 flex items-center justify-center gap-2 text-sm text-ember",
						children: [/* @__PURE__ */ (0, import_jsx_runtime.jsx)(Flag, { className: "size-4" }), " You have the flag"]
					}),
					hud.ownFlag === 2 && hud.flag < 0 && /* @__PURE__ */ (0, import_jsx_runtime.jsx)("p", {
						className: "mt-1 text-sm text-ember",
						children: "Your flag is taken"
					})
				]
			}),
			/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
				className: "absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2",
				style: { opacity: hud.alive ? 1 : .2 },
				children: [
					/* @__PURE__ */ (0, import_jsx_runtime.jsx)("span", { className: `absolute -left-3 top-0 h-px w-2 ${hud.hit > 0 ? "bg-ember" : "bg-fg"}` }),
					/* @__PURE__ */ (0, import_jsx_runtime.jsx)("span", { className: `absolute -right-3 top-0 h-px w-2 ${hud.hit > 0 ? "bg-ember" : "bg-fg"}` }),
					/* @__PURE__ */ (0, import_jsx_runtime.jsx)("span", { className: `absolute left-0 -top-3 w-px h-2 ${hud.hit > 0 ? "bg-ember" : "bg-fg"}` }),
					/* @__PURE__ */ (0, import_jsx_runtime.jsx)("span", { className: `absolute left-0 -bottom-3 w-px h-2 ${hud.hit > 0 ? "bg-ember" : "bg-fg"}` })
				]
			}),
			/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
				className: "absolute bottom-6 left-4 right-4 flex items-end justify-between gap-4 sm:left-6 sm:right-6",
				children: [
					/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
						className: "w-44 space-y-2 sm:w-56",
						children: [/* @__PURE__ */ (0, import_jsx_runtime.jsx)(Bar, {
							label: "Armor",
							value: hud.health,
							color: "bg-ember"
						}), /* @__PURE__ */ (0, import_jsx_runtime.jsx)(Bar, {
							label: "Energy",
							value: hud.energy,
							color: "bg-glacier"
						})]
					}),
					/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
						className: "text-center",
						children: [/* @__PURE__ */ (0, import_jsx_runtime.jsx)("p", {
							className: "font-display text-xl font-semibold tracking-wide",
							children: hud.weapon === 0 ? "Disc" : "Repeater"
						}), /* @__PURE__ */ (0, import_jsx_runtime.jsxs)("p", {
							className: "text-xs text-muted tabular-nums",
							children: [
								hud.kills,
								" frag · ",
								hud.deaths,
								" down"
							]
						})]
					}),
					!isTouch && /* @__PURE__ */ (0, import_jsx_runtime.jsxs)("p", {
						className: "hidden w-44 text-right text-xs text-muted sm:block",
						children: [
							"Space ski/jet · ESC pause",
							/* @__PURE__ */ (0, import_jsx_runtime.jsx)("br", {}),
							teamEmber ? "Ember" : "Glacier"
						]
					})
				]
			})
		]
	});
}
function Bar({ label, value, color }) {
	return /* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", { children: [/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
		className: "mb-1 flex justify-between text-xs tracking-wider text-muted uppercase",
		children: [/* @__PURE__ */ (0, import_jsx_runtime.jsx)("span", { children: label }), /* @__PURE__ */ (0, import_jsx_runtime.jsx)("span", {
			className: "tabular-nums text-fg",
			children: Math.round(value)
		})]
	}), /* @__PURE__ */ (0, import_jsx_runtime.jsx)("div", {
		className: "h-1.5 overflow-hidden rounded-full bg-elevated",
		children: /* @__PURE__ */ (0, import_jsx_runtime.jsx)("div", {
			className: `h-full ${color}`,
			style: { width: `${Math.max(0, Math.min(100, value))}%` }
		})
	})] });
}
function Minimap({ blips, yaw, team }) {
	const ref = (0, import_react.useRef)(null);
	(0, import_react.useEffect)(() => {
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
		ctx.strokeRect(.5, .5, 111, 111);
		ctx.save();
		ctx.translate(s / 2, s / 2);
		ctx.rotate(-yaw);
		ctx.translate(-56, -56);
		for (const part of blips.split(";")) {
			if (!part) continue;
			const [xs, zs, ts, kind] = part.split(",");
			const x = Number(xs) / 256 * 100 + 6;
			const z = Number(zs) / 256 * 100 + 6;
			ctx.fillStyle = Number(ts) === 0 ? "#e24a32" : "#3ec8e0";
			if (kind === "2") ctx.fillRect(x - 3, z - 3, 6, 6);
			else if (kind === "1") {
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
	}, [
		blips,
		yaw,
		team
	]);
	return /* @__PURE__ */ (0, import_jsx_runtime.jsx)("canvas", {
		ref,
		className: "h-24 w-24 rounded-md border border-border bg-bg/70"
	});
}
function TouchControls({ onJump, onFire, onSwap }) {
	return /* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
		className: "pointer-events-none absolute inset-x-0 bottom-4 flex items-end justify-between px-4",
		children: [/* @__PURE__ */ (0, import_jsx_runtime.jsx)("div", { className: "pointer-events-none h-28 w-28 rounded-full border border-border bg-surface/40" }), /* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
			className: "pointer-events-auto flex gap-3",
			children: [
				/* @__PURE__ */ (0, import_jsx_runtime.jsx)("button", {
					type: "button",
					className: "flex h-14 w-14 items-center justify-center rounded-full border border-border bg-surface/80 text-xs font-medium",
					onPointerDown: onSwap,
					children: "Swap"
				}),
				/* @__PURE__ */ (0, import_jsx_runtime.jsx)("button", {
					type: "button",
					className: "flex h-16 w-16 items-center justify-center rounded-full border border-glacier bg-surface/80",
					onPointerDown: () => onJump(true),
					onPointerUp: () => onJump(false),
					onPointerCancel: () => onJump(false),
					children: /* @__PURE__ */ (0, import_jsx_runtime.jsx)(Zap, { className: "size-5 text-glacier" })
				}),
				/* @__PURE__ */ (0, import_jsx_runtime.jsx)("button", {
					type: "button",
					className: "flex h-16 w-16 items-center justify-center rounded-full border border-ember bg-ember/80 font-display text-sm font-semibold",
					onPointerDown: () => onFire(true),
					onPointerUp: () => onFire(false),
					onPointerCancel: () => onFire(false),
					children: "Fire"
				})
			]
		})]
	});
}
function PauseMenu({ onResume, onMenu, muted, onMute }) {
	return /* @__PURE__ */ (0, import_jsx_runtime.jsx)("div", {
		className: "absolute inset-0 flex items-center justify-center bg-bg/70 p-6",
		children: /* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
			className: "w-full max-w-sm rounded-xl border border-border bg-surface p-6 shadow-lg",
			children: [/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
				className: "mb-6 flex items-center gap-2 text-muted",
				children: [/* @__PURE__ */ (0, import_jsx_runtime.jsx)(Pause, { className: "size-4" }), /* @__PURE__ */ (0, import_jsx_runtime.jsx)("h2", {
					className: "font-display text-2xl font-semibold tracking-wide text-fg",
					children: "Paused"
				})]
			}), /* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
				className: "flex flex-col gap-3",
				children: [
					/* @__PURE__ */ (0, import_jsx_runtime.jsx)("button", {
						type: "button",
						onClick: onResume,
						className: "h-11 rounded-md bg-fg font-medium text-bg",
						children: "Resume"
					}),
					/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("button", {
						type: "button",
						onClick: onMute,
						className: "flex h-11 items-center justify-center gap-2 rounded-md border border-border text-fg",
						children: [muted ? /* @__PURE__ */ (0, import_jsx_runtime.jsx)(VolumeX, { className: "size-4" }) : /* @__PURE__ */ (0, import_jsx_runtime.jsx)(Volume2, { className: "size-4" }), muted ? "Unmute" : "Mute"]
					}),
					/* @__PURE__ */ (0, import_jsx_runtime.jsx)("button", {
						type: "button",
						onClick: onMenu,
						className: "h-11 rounded-md border border-border text-muted",
						children: "Leave rift"
					})
				]
			})]
		})
	});
}
function EndScreen({ hud, teamEmber, onAgain, onMenu }) {
	const win = hud.winner === -1 ? "Draw" : hud.winner === hud.team ? "Victory" : "Defeat";
	return /* @__PURE__ */ (0, import_jsx_runtime.jsx)("div", {
		className: "absolute inset-0 flex items-center justify-center bg-bg/75 p-6",
		children: /* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
			className: "w-full max-w-md rounded-xl border border-border bg-surface p-8 text-center",
			children: [
				/* @__PURE__ */ (0, import_jsx_runtime.jsx)("p", {
					className: "font-display text-xs tracking-[0.28em] text-muted uppercase",
					children: "Match over"
				}),
				/* @__PURE__ */ (0, import_jsx_runtime.jsx)("h2", {
					className: "mt-2 font-display text-5xl font-semibold tracking-tight",
					children: win
				}),
				/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("p", {
					className: "mt-4 font-display text-3xl tabular-nums",
					children: [
						/* @__PURE__ */ (0, import_jsx_runtime.jsx)("span", {
							className: "text-ember",
							children: hud.ember
						}),
						/* @__PURE__ */ (0, import_jsx_runtime.jsx)("span", {
							className: "mx-3 text-muted",
							children: "–"
						}),
						/* @__PURE__ */ (0, import_jsx_runtime.jsx)("span", {
							className: "text-glacier",
							children: hud.glacier
						})
					]
				}),
				/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("p", {
					className: "mt-2 text-sm text-muted",
					children: [
						hud.kills,
						" frags · ",
						hud.deaths,
						" deaths · ",
						teamEmber ? "Ember" : "Glacier"
					]
				}),
				/* @__PURE__ */ (0, import_jsx_runtime.jsxs)("div", {
					className: "mt-8 flex flex-col gap-3",
					children: [/* @__PURE__ */ (0, import_jsx_runtime.jsx)("button", {
						type: "button",
						onClick: onAgain,
						className: "h-11 rounded-md bg-fg font-medium text-bg",
						children: "Start match"
					}), /* @__PURE__ */ (0, import_jsx_runtime.jsx)("button", {
						type: "button",
						onClick: onMenu,
						className: "h-11 rounded-md border border-border text-muted",
						children: "Menu"
					})]
				})
			]
		})
	});
}
function Home() {
	return /* @__PURE__ */ (0, import_jsx_runtime.jsx)(PeakstrifeApp, {});
}
//#endregion
export { Home as component };
