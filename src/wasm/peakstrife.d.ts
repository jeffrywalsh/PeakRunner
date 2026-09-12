/* tslint:disable */
/* eslint-disable */
export class Game {
  free(): void;
  [Symbol.dispose](): void;
  set_paused(paused: boolean): void;
  start_match(ember: boolean): void;
  constructor(canvas: HTMLCanvasElement);
  frame(dt: number): string;
  get_px(): number;
  get_py(): number;
  get_pz(): number;
  resize(w: number, h: number): void;
  get_yaw(): number;
  add_look(dx: number, dy: number): void;
  set_keys(codes: Array<any>): void;
  get_speed(): number;
  set_input(move_x: number, move_z: number, jump: boolean, fire: boolean, weapon: number, look_x: number, look_y: number): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_game_free: (a: number, b: number) => void;
  readonly game_add_look: (a: number, b: number, c: number) => void;
  readonly game_frame: (a: number, b: number) => [number, number];
  readonly game_get_px: (a: number) => number;
  readonly game_get_py: (a: number) => number;
  readonly game_get_pz: (a: number) => number;
  readonly game_get_speed: (a: number) => number;
  readonly game_get_yaw: (a: number) => number;
  readonly game_new: (a: any) => [number, number, number];
  readonly game_resize: (a: number, b: number, c: number) => void;
  readonly game_set_input: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
  readonly game_set_keys: (a: number, b: any) => void;
  readonly game_set_paused: (a: number, b: number) => void;
  readonly game_start_match: (a: number, b: number) => void;
  readonly __externref_table_alloc: () => number;
  readonly __wbindgen_export_1: WebAssembly.Table;
  readonly __wbindgen_exn_store: (a: number) => void;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __externref_table_dealloc: (a: number) => void;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;
/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
