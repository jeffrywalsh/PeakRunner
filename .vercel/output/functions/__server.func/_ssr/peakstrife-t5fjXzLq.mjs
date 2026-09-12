//#region node_modules/.nitro/vite/services/ssr/assets/peakstrife-t5fjXzLq.js
var wasm;
function isLikeNone(x) {
	return x === void 0 || x === null;
}
function addToExternrefTable0(obj) {
	const idx = wasm.__externref_table_alloc();
	wasm.__wbindgen_export_1.set(idx, obj);
	return idx;
}
var cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
	if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
	return cachedUint8ArrayMemory0;
}
var cachedTextDecoder = new TextDecoder("utf-8", {
	ignoreBOM: true,
	fatal: true
});
cachedTextDecoder.decode();
var MAX_SAFARI_DECODE_BYTES = 2146435072;
var numBytesDecoded = 0;
function decodeText(ptr, len) {
	numBytesDecoded += len;
	if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
		cachedTextDecoder = new TextDecoder("utf-8", {
			ignoreBOM: true,
			fatal: true
		});
		cachedTextDecoder.decode();
		numBytesDecoded = len;
	}
	return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}
function getStringFromWasm0(ptr, len) {
	ptr = ptr >>> 0;
	return decodeText(ptr, len);
}
function handleError(f, args) {
	try {
		return f.apply(this, args);
	} catch (e) {
		const idx = addToExternrefTable0(e);
		wasm.__wbindgen_exn_store(idx);
	}
}
var WASM_VECTOR_LEN = 0;
var cachedTextEncoder = new TextEncoder();
if (!("encodeInto" in cachedTextEncoder)) cachedTextEncoder.encodeInto = function(arg, view) {
	const buf = cachedTextEncoder.encode(arg);
	view.set(buf);
	return {
		read: arg.length,
		written: buf.length
	};
};
function passStringToWasm0(arg, malloc, realloc) {
	if (realloc === void 0) {
		const buf = cachedTextEncoder.encode(arg);
		const ptr = malloc(buf.length, 1) >>> 0;
		getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
		WASM_VECTOR_LEN = buf.length;
		return ptr;
	}
	let len = arg.length;
	let ptr = malloc(len, 1) >>> 0;
	const mem = getUint8ArrayMemory0();
	let offset = 0;
	for (; offset < len; offset++) {
		const code = arg.charCodeAt(offset);
		if (code > 127) break;
		mem[ptr + offset] = code;
	}
	if (offset !== len) {
		if (offset !== 0) arg = arg.slice(offset);
		ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
		const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
		const ret = cachedTextEncoder.encodeInto(arg, view);
		offset += ret.written;
		ptr = realloc(ptr, len, offset, 1) >>> 0;
	}
	WASM_VECTOR_LEN = offset;
	return ptr;
}
var cachedDataViewMemory0 = null;
function getDataViewMemory0() {
	if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || cachedDataViewMemory0.buffer.detached === void 0 && cachedDataViewMemory0.buffer !== wasm.memory.buffer) cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
	return cachedDataViewMemory0;
}
var cachedFloat32ArrayMemory0 = null;
function getFloat32ArrayMemory0() {
	if (cachedFloat32ArrayMemory0 === null || cachedFloat32ArrayMemory0.byteLength === 0) cachedFloat32ArrayMemory0 = new Float32Array(wasm.memory.buffer);
	return cachedFloat32ArrayMemory0;
}
function getArrayF32FromWasm0(ptr, len) {
	ptr = ptr >>> 0;
	return getFloat32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}
var cachedUint16ArrayMemory0 = null;
function getUint16ArrayMemory0() {
	if (cachedUint16ArrayMemory0 === null || cachedUint16ArrayMemory0.byteLength === 0) cachedUint16ArrayMemory0 = new Uint16Array(wasm.memory.buffer);
	return cachedUint16ArrayMemory0;
}
function getArrayU16FromWasm0(ptr, len) {
	ptr = ptr >>> 0;
	return getUint16ArrayMemory0().subarray(ptr / 2, ptr / 2 + len);
}
function takeFromExternrefTable0(idx) {
	const value = wasm.__wbindgen_export_1.get(idx);
	wasm.__externref_table_dealloc(idx);
	return value;
}
var GameFinalization = typeof FinalizationRegistry === "undefined" ? {
	register: () => {},
	unregister: () => {}
} : new FinalizationRegistry((ptr) => wasm.__wbg_game_free(ptr >>> 0, 1));
var Game = class {
	__destroy_into_raw() {
		const ptr = this.__wbg_ptr;
		this.__wbg_ptr = 0;
		GameFinalization.unregister(this);
		return ptr;
	}
	free() {
		const ptr = this.__destroy_into_raw();
		wasm.__wbg_game_free(ptr, 0);
	}
	/**
	* @param {boolean} paused
	*/
	set_paused(paused) {
		wasm.game_set_paused(this.__wbg_ptr, paused);
	}
	/**
	* @param {boolean} ember
	*/
	start_match(ember) {
		wasm.game_start_match(this.__wbg_ptr, ember);
	}
	/**
	* @param {HTMLCanvasElement} canvas
	*/
	constructor(canvas) {
		const ret = wasm.game_new(canvas);
		if (ret[2]) throw takeFromExternrefTable0(ret[1]);
		this.__wbg_ptr = ret[0] >>> 0;
		GameFinalization.register(this, this.__wbg_ptr, this);
		return this;
	}
	/**
	* @param {number} dt
	* @returns {string}
	*/
	frame(dt) {
		let deferred1_0;
		let deferred1_1;
		try {
			const ret = wasm.game_frame(this.__wbg_ptr, dt);
			deferred1_0 = ret[0];
			deferred1_1 = ret[1];
			return getStringFromWasm0(ret[0], ret[1]);
		} finally {
			wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
		}
	}
	/**
	* @returns {number}
	*/
	get_px() {
		return wasm.game_get_px(this.__wbg_ptr);
	}
	/**
	* @returns {number}
	*/
	get_py() {
		return wasm.game_get_py(this.__wbg_ptr);
	}
	/**
	* @returns {number}
	*/
	get_pz() {
		return wasm.game_get_pz(this.__wbg_ptr);
	}
	/**
	* @param {number} w
	* @param {number} h
	*/
	resize(w, h) {
		wasm.game_resize(this.__wbg_ptr, w, h);
	}
	/**
	* @returns {number}
	*/
	get_yaw() {
		return wasm.game_get_yaw(this.__wbg_ptr);
	}
	/**
	* @param {number} dx
	* @param {number} dy
	*/
	add_look(dx, dy) {
		wasm.game_add_look(this.__wbg_ptr, dx, dy);
	}
	/**
	* @param {Array<any>} codes
	*/
	set_keys(codes) {
		wasm.game_set_keys(this.__wbg_ptr, codes);
	}
	/**
	* @returns {number}
	*/
	get_speed() {
		return wasm.game_get_speed(this.__wbg_ptr);
	}
	/**
	* @param {number} move_x
	* @param {number} move_z
	* @param {boolean} jump
	* @param {boolean} fire
	* @param {number} weapon
	* @param {number} look_x
	* @param {number} look_y
	*/
	set_input(move_x, move_z, jump, fire, weapon, look_x, look_y) {
		wasm.game_set_input(this.__wbg_ptr, move_x, move_z, jump, fire, weapon, look_x, look_y);
	}
};
if (Symbol.dispose) Game.prototype[Symbol.dispose] = Game.prototype.free;
var EXPECTED_RESPONSE_TYPES = /* @__PURE__ */ new Set([
	"basic",
	"cors",
	"default"
]);
async function __wbg_load(module, imports) {
	if (typeof Response === "function" && module instanceof Response) {
		if (typeof WebAssembly.instantiateStreaming === "function") try {
			return await WebAssembly.instantiateStreaming(module, imports);
		} catch (e) {
			if (module.ok && EXPECTED_RESPONSE_TYPES.has(module.type) && module.headers.get("Content-Type") !== "application/wasm") console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);
			else throw e;
		}
		const bytes = await module.arrayBuffer();
		return await WebAssembly.instantiate(bytes, imports);
	} else {
		const instance = await WebAssembly.instantiate(module, imports);
		if (instance instanceof WebAssembly.Instance) return {
			instance,
			module
		};
		else return instance;
	}
}
function __wbg_get_imports() {
	const imports = {};
	imports.wbg = {};
	imports.wbg.__wbg_attachShader_8bc6f118fa003360 = function(arg0, arg1, arg2) {
		arg0.attachShader(arg1, arg2);
	};
	imports.wbg.__wbg_bindBuffer_ca632d407a6cd394 = function(arg0, arg1, arg2) {
		arg0.bindBuffer(arg1 >>> 0, arg2);
	};
	imports.wbg.__wbg_bindVertexArray_38371b6174c99865 = function(arg0, arg1) {
		arg0.bindVertexArray(arg1);
	};
	imports.wbg.__wbg_blendFunc_53c2ed15a60e24a4 = function(arg0, arg1, arg2) {
		arg0.blendFunc(arg1 >>> 0, arg2 >>> 0);
	};
	imports.wbg.__wbg_bufferData_a964c14d0eebdeb8 = function(arg0, arg1, arg2, arg3) {
		arg0.bufferData(arg1 >>> 0, arg2, arg3 >>> 0);
	};
	imports.wbg.__wbg_clearColor_6e4857102d3b1d7f = function(arg0, arg1, arg2, arg3, arg4) {
		arg0.clearColor(arg1, arg2, arg3, arg4);
	};
	imports.wbg.__wbg_clear_7b717c6b7a62cb56 = function(arg0, arg1) {
		arg0.clear(arg1 >>> 0);
	};
	imports.wbg.__wbg_compileShader_3ed42f9f82c060ea = function(arg0, arg1) {
		arg0.compileShader(arg1);
	};
	imports.wbg.__wbg_createBuffer_6a92125855922b2e = function(arg0) {
		const ret = arg0.createBuffer();
		return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
	};
	imports.wbg.__wbg_createProgram_905f3efd8354e76c = function(arg0) {
		const ret = arg0.createProgram();
		return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
	};
	imports.wbg.__wbg_createShader_8548d722c1327303 = function(arg0, arg1) {
		const ret = arg0.createShader(arg1 >>> 0);
		return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
	};
	imports.wbg.__wbg_createVertexArray_54f6bb34c6bf6a01 = function(arg0) {
		const ret = arg0.createVertexArray();
		return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
	};
	imports.wbg.__wbg_depthFunc_bc0220c6a44cc71b = function(arg0, arg1) {
		arg0.depthFunc(arg1 >>> 0);
	};
	imports.wbg.__wbg_depthMask_8fdcb3b4d4e6e61e = function(arg0, arg1) {
		arg0.depthMask(arg1 !== 0);
	};
	imports.wbg.__wbg_disable_8a09d5dbbf79acd8 = function(arg0, arg1) {
		arg0.disable(arg1 >>> 0);
	};
	imports.wbg.__wbg_drawElements_3acf8f6523f00d29 = function(arg0, arg1, arg2, arg3, arg4) {
		arg0.drawElements(arg1 >>> 0, arg2, arg3 >>> 0, arg4);
	};
	imports.wbg.__wbg_enableVertexAttribArray_17e09202dc56b410 = function(arg0, arg1) {
		arg0.enableVertexAttribArray(arg1 >>> 0);
	};
	imports.wbg.__wbg_enable_d2b20d4e604e4ada = function(arg0, arg1) {
		arg0.enable(arg1 >>> 0);
	};
	imports.wbg.__wbg_getContext_15e158d04230a6f6 = function() {
		return handleError(function(arg0, arg1, arg2) {
			const ret = arg0.getContext(getStringFromWasm0(arg1, arg2));
			return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
		}, arguments);
	};
	imports.wbg.__wbg_getProgramInfoLog_0f2cbb1decc2bdb4 = function(arg0, arg1, arg2) {
		const ret = arg1.getProgramInfoLog(arg2);
		var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
		var len1 = WASM_VECTOR_LEN;
		getDataViewMemory0().setInt32(arg0 + 4, len1, true);
		getDataViewMemory0().setInt32(arg0 + 0, ptr1, true);
	};
	imports.wbg.__wbg_getProgramParameter_fbfb133d8f8e5a0e = function(arg0, arg1, arg2) {
		return arg0.getProgramParameter(arg1, arg2 >>> 0);
	};
	imports.wbg.__wbg_getShaderInfoLog_42f0460a19309f2b = function(arg0, arg1, arg2) {
		const ret = arg1.getShaderInfoLog(arg2);
		var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
		var len1 = WASM_VECTOR_LEN;
		getDataViewMemory0().setInt32(arg0 + 4, len1, true);
		getDataViewMemory0().setInt32(arg0 + 0, ptr1, true);
	};
	imports.wbg.__wbg_getShaderParameter_17cf070915068143 = function(arg0, arg1, arg2) {
		return arg0.getShaderParameter(arg1, arg2 >>> 0);
	};
	imports.wbg.__wbg_getUniformLocation_3c1cc7519f10e1e9 = function(arg0, arg1, arg2, arg3) {
		const ret = arg0.getUniformLocation(arg1, getStringFromWasm0(arg2, arg3));
		return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
	};
	imports.wbg.__wbg_get_0da715ceaecea5c8 = function(arg0, arg1) {
		return arg0[arg1 >>> 0];
	};
	imports.wbg.__wbg_instanceof_WebGl2RenderingContext_0437ff340aef5ac7 = function(arg0) {
		let result;
		try {
			result = arg0 instanceof WebGL2RenderingContext;
		} catch (_) {
			result = false;
		}
		return result;
	};
	imports.wbg.__wbg_length_186546c51cd61acd = function(arg0) {
		return arg0.length;
	};
	imports.wbg.__wbg_length_a8cca01d07ea9653 = function(arg0) {
		return arg0.length;
	};
	imports.wbg.__wbg_length_e0fc3528a2d029bf = function(arg0) {
		return arg0.length;
	};
	imports.wbg.__wbg_linkProgram_4bf446d2d081aa07 = function(arg0, arg1) {
		arg0.linkProgram(arg1);
	};
	imports.wbg.__wbg_newwithlength_0a042e763e18d539 = function(arg0) {
		return new Uint16Array(arg0 >>> 0);
	};
	imports.wbg.__wbg_newwithlength_dff392ea2a428c80 = function(arg0) {
		return new Float32Array(arg0 >>> 0);
	};
	imports.wbg.__wbg_set_5c1dc658cb210721 = function(arg0, arg1, arg2) {
		arg0.set(getArrayF32FromWasm0(arg1, arg2));
	};
	imports.wbg.__wbg_set_a5d33398eef3cdef = function(arg0, arg1, arg2) {
		arg0.set(getArrayU16FromWasm0(arg1, arg2));
	};
	imports.wbg.__wbg_shaderSource_2ed8147ed144f6d6 = function(arg0, arg1, arg2, arg3) {
		arg0.shaderSource(arg1, getStringFromWasm0(arg2, arg3));
	};
	imports.wbg.__wbg_uniform1f_0a141bee125b351e = function(arg0, arg1, arg2) {
		arg0.uniform1f(arg1, arg2);
	};
	imports.wbg.__wbg_uniform3f_16e5596ebd3b41a3 = function(arg0, arg1, arg2, arg3, arg4) {
		arg0.uniform3f(arg1, arg2, arg3, arg4);
	};
	imports.wbg.__wbg_uniform4f_bdbc23e5bfc9627b = function(arg0, arg1, arg2, arg3, arg4, arg5) {
		arg0.uniform4f(arg1, arg2, arg3, arg4, arg5);
	};
	imports.wbg.__wbg_uniformMatrix4fv_cefcf2bb4c08d391 = function(arg0, arg1, arg2, arg3, arg4) {
		arg0.uniformMatrix4fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
	};
	imports.wbg.__wbg_useProgram_3e5c220728446c29 = function(arg0, arg1) {
		arg0.useProgram(arg1);
	};
	imports.wbg.__wbg_vertexAttribPointer_3549d2703f29bf38 = function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
		arg0.vertexAttribPointer(arg1 >>> 0, arg2, arg3 >>> 0, arg4 !== 0, arg5, arg6);
	};
	imports.wbg.__wbg_viewport_08854654c5c2bba6 = function(arg0, arg1, arg2, arg3, arg4) {
		arg0.viewport(arg1, arg2, arg3, arg4);
	};
	imports.wbg.__wbg_wbindgenbooleanget_3fe6f642c7d97746 = function(arg0) {
		const v = arg0;
		const ret = typeof v === "boolean" ? v : void 0;
		return isLikeNone(ret) ? 16777215 : ret ? 1 : 0;
	};
	imports.wbg.__wbg_wbindgenstringget_0f16a6ddddef376f = function(arg0, arg1) {
		const obj = arg1;
		const ret = typeof obj === "string" ? obj : void 0;
		var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
		var len1 = WASM_VECTOR_LEN;
		getDataViewMemory0().setInt32(arg0 + 4, len1, true);
		getDataViewMemory0().setInt32(arg0 + 0, ptr1, true);
	};
	imports.wbg.__wbg_wbindgenthrow_451ec1a8469d7eb6 = function(arg0, arg1) {
		throw new Error(getStringFromWasm0(arg0, arg1));
	};
	imports.wbg.__wbindgen_cast_2241b6af4c4b2941 = function(arg0, arg1) {
		return getStringFromWasm0(arg0, arg1);
	};
	imports.wbg.__wbindgen_init_externref_table = function() {
		const table = wasm.__wbindgen_export_1;
		const offset = table.grow(4);
		table.set(0, void 0);
		table.set(offset + 0, void 0);
		table.set(offset + 1, null);
		table.set(offset + 2, true);
		table.set(offset + 3, false);
	};
	return imports;
}
function __wbg_finalize_init(instance, module) {
	wasm = instance.exports;
	__wbg_init.__wbindgen_wasm_module = module;
	cachedDataViewMemory0 = null;
	cachedFloat32ArrayMemory0 = null;
	cachedUint16ArrayMemory0 = null;
	cachedUint8ArrayMemory0 = null;
	wasm.__wbindgen_start();
	return wasm;
}
async function __wbg_init(module_or_path) {
	if (wasm !== void 0) return wasm;
	if (typeof module_or_path !== "undefined") {
		if (Object.getPrototypeOf(module_or_path) === Object.prototype) ({module_or_path} = module_or_path);
		else console.warn("using deprecated parameters for the initialization function; pass a single object instead");
	}
	if (typeof module_or_path === "undefined") module_or_path = new URL("peakstrife_bg.wasm", import.meta.url);
	const imports = __wbg_get_imports();
	if (typeof module_or_path === "string" || typeof Request === "function" && module_or_path instanceof Request || typeof URL === "function" && module_or_path instanceof URL) module_or_path = fetch(module_or_path);
	const { instance, module } = await __wbg_load(await module_or_path, imports);
	return __wbg_finalize_init(instance, module);
}
//#endregion
export { Game, __wbg_init as default };
