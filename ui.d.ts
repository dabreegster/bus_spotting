/* tslint:disable */
/* eslint-disable */
/**
*/
export function run_wasm(): void;
/**
*/
export class IntoUnderlyingByteSource {
  free(): void;
/**
* @param {any} controller
*/
  start(controller: any): void;
/**
* @param {any} controller
* @returns {Promise<any>}
*/
  pull(controller: any): Promise<any>;
/**
*/
  cancel(): void;
/**
*/
  readonly autoAllocateChunkSize: number;
/**
*/
  readonly type: string;
}
/**
*/
export class IntoUnderlyingSink {
  free(): void;
/**
* @param {any} chunk
* @returns {Promise<any>}
*/
  write(chunk: any): Promise<any>;
/**
* @returns {Promise<any>}
*/
  close(): Promise<any>;
/**
* @param {any} reason
* @returns {Promise<any>}
*/
  abort(reason: any): Promise<any>;
}
/**
*/
export class IntoUnderlyingSource {
  free(): void;
/**
* @param {any} controller
* @returns {Promise<any>}
*/
  pull(controller: any): Promise<any>;
/**
*/
  cancel(): void;
}
/**
* Raw options for [`pipeTo()`](https://developer.mozilla.org/en-US/docs/Web/API/ReadableStream/pipeTo).
*/
export class PipeOptions {
  free(): void;
/**
*/
  readonly preventAbort: boolean;
/**
*/
  readonly preventCancel: boolean;
/**
*/
  readonly preventClose: boolean;
/**
*/
  readonly signal: AbortSignal | undefined;
}
/**
*/
export class QueuingStrategy {
  free(): void;
/**
*/
  readonly highWaterMark: number;
}
/**
* Raw options for [`getReader()`](https://developer.mozilla.org/en-US/docs/Web/API/ReadableStream/getReader).
*/
export class ReadableStreamGetReaderOptions {
  free(): void;
/**
*/
  readonly mode: any;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly run_wasm: () => void;
  readonly __wbg_readablestreamgetreaderoptions_free: (a: number) => void;
  readonly readablestreamgetreaderoptions_mode: (a: number) => number;
  readonly __wbg_pipeoptions_free: (a: number) => void;
  readonly pipeoptions_preventClose: (a: number) => number;
  readonly pipeoptions_preventCancel: (a: number) => number;
  readonly pipeoptions_preventAbort: (a: number) => number;
  readonly pipeoptions_signal: (a: number) => number;
  readonly __wbg_intounderlyingbytesource_free: (a: number) => void;
  readonly intounderlyingbytesource_type: (a: number, b: number) => void;
  readonly intounderlyingbytesource_autoAllocateChunkSize: (a: number) => number;
  readonly intounderlyingbytesource_start: (a: number, b: number) => void;
  readonly intounderlyingbytesource_pull: (a: number, b: number) => number;
  readonly intounderlyingbytesource_cancel: (a: number) => void;
  readonly __wbg_queuingstrategy_free: (a: number) => void;
  readonly queuingstrategy_highWaterMark: (a: number) => number;
  readonly __wbg_intounderlyingsink_free: (a: number) => void;
  readonly intounderlyingsink_write: (a: number, b: number) => number;
  readonly intounderlyingsink_close: (a: number) => number;
  readonly intounderlyingsink_abort: (a: number, b: number) => number;
  readonly __wbg_intounderlyingsource_free: (a: number) => void;
  readonly intounderlyingsource_pull: (a: number, b: number) => number;
  readonly intounderlyingsource_cancel: (a: number) => void;
  readonly __wbindgen_malloc: (a: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number) => number;
  readonly __wbindgen_export_2: WebAssembly.Table;
  readonly _dyn_core__ops__function__FnMut_____Output___R_as_wasm_bindgen__closure__WasmClosure___describe__invoke__h5f90d6857b91a609: (a: number, b: number) => void;
  readonly _dyn_core__ops__function__FnMut__A____Output___R_as_wasm_bindgen__closure__WasmClosure___describe__invoke__h46c3bc3cc99a6e2b: (a: number, b: number, c: number) => void;
  readonly _dyn_core__ops__function__FnMut_____Output___R_as_wasm_bindgen__closure__WasmClosure___describe__invoke__hdd6d6dd12a17d45a: (a: number, b: number) => void;
  readonly _dyn_core__ops__function__FnMut__A____Output___R_as_wasm_bindgen__closure__WasmClosure___describe__invoke__h63c19e4a4f9f6d31: (a: number, b: number, c: number) => void;
  readonly _dyn_core__ops__function__FnMut_____Output___R_as_wasm_bindgen__closure__WasmClosure___describe__invoke__hc82db546f9cdcc4a: (a: number, b: number) => void;
  readonly _dyn_core__ops__function__FnMut__A____Output___R_as_wasm_bindgen__closure__WasmClosure___describe__invoke__h5f33362706220879: (a: number, b: number, c: number) => void;
  readonly __wbindgen_exn_store: (a: number) => void;
  readonly wasm_bindgen__convert__closures__invoke2_mut__hc5dbc8e58bdb34f7: (a: number, b: number, c: number, d: number) => void;
  readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
  readonly __wbindgen_free: (a: number, b: number) => void;
  readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;
/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {SyncInitInput} module
*
* @returns {InitOutput}
*/
export function initSync(module: SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {InitInput | Promise<InitInput>} module_or_path
*
* @returns {Promise<InitOutput>}
*/
export default function init (module_or_path?: InitInput | Promise<InitInput>): Promise<InitOutput>;
