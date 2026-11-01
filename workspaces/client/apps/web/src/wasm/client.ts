import type { MarkdownBlock } from "../generated/rustOwned";
import init, {
  identity_proof_message,
  multi_server_signing_bytes,
  parse_markdown,
  sha256 as hashSha256,
} from "./generated/client";
import wasmURL from "./generated/client_bg.wasm?url";

let ready: Promise<unknown> | undefined;
async function initialize() {
  ready ??= init({ module_or_path: wasmURL }).catch((error: unknown) => {
    ready = undefined;
    throw error;
  });
  await ready;
}

export async function sha256(bytes: Uint8Array): Promise<Uint8Array> {
  await initialize();
  return hashSha256(bytes);
}

/** The message a server signs for `nonce`. Rejects a nonce of the wrong shape. */
export async function identityProofMessage(
  serverId: string,
  nonce: string,
): Promise<string> {
  await initialize();
  return identity_proof_message(serverId, nonce);
}

/** The canonical bytes of a signed multi-server request. The owner is `studio-request-signing`. */
export async function multiServerSigningBytes(
  method: string,
  target: string,
  serverId: string,
  clientId: string,
  timestamp: string,
  nonce: string,
  requestId: string,
  body: Uint8Array,
): Promise<Uint8Array<ArrayBuffer>> {
  await initialize();
  return multi_server_signing_bytes(
    method,
    target,
    serverId,
    clientId,
    timestamp,
    nonce,
    requestId,
    body,
  ) as Uint8Array<ArrayBuffer>;
}

export async function parseMarkdown(text: string): Promise<MarkdownBlock[]> {
  await initialize();
  return JSON.parse(parse_markdown(text, false));
}
