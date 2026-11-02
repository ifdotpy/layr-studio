/* tslint:disable */
/* eslint-disable */

/**
 * The message a server signs to prove its identity for `nonce`. The owner is `studio-request-signing`.
 */
export function identity_proof_message(server_id: string, nonce: string): string;

/**
 * The canonical bytes of one signed multi-server request. The owner is `studio-request-signing`;
 * the browser signs these bytes with its non-extractable device key.
 */
export function multi_server_signing_bytes(method: string, target: string, server_id: string, client_id: string, timestamp: string, nonce: string, request_id: string, body: Uint8Array): Uint8Array;

export function parse_markdown(text: string, streaming: boolean): string;

/**
 * SHA-256 for browser content identities when Web Crypto is unavailable.
 */
export function sha256(bytes: Uint8Array): Uint8Array;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly identity_proof_message: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly multi_server_signing_bytes: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number, p: number) => [number, number];
    readonly parse_markdown: (a: number, b: number, c: number) => [number, number, number, number];
    readonly sha256: (a: number, b: number) => [number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
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
