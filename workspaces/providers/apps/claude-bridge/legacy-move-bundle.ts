import { z } from "zod";
import type { BridgeMoveProof, BridgeMoveSession } from "./bridge-protocol.ts";

// Temporary opaque boundary, removed with server-owned moves in wave C. Only
// the headers are validated here. The caller must retain the existing canonical,
// optionsHash and snapshotHash checks before changing files or session ownership.
const proofHeader = z
  .object({
    snapshotHash: z.string(),
    optionsHash: z.string(),
    proofMethod: z.string(),
    tools: z.array(z.object({ name: z.string() }).passthrough()),
    options: z.record(z.string(), z.unknown()),
    session: z.record(z.string(), z.unknown()),
  })
  .passthrough();
const sessionHeader = z
  .object({
    id: z.string(),
    cwd: z.string(),
    turns: z.array(z.unknown()),
    dynamicToolsJson: z.string(),
    moveProof: z
      .object({ snapshotHash: z.string(), optionsHash: z.string() })
      .passthrough(),
  })
  .passthrough();
export function decodeLegacyMoveProof(value: unknown): BridgeMoveProof {
  return proofHeader.parse(value) as BridgeMoveProof;
}
export function decodeLegacyMoveSession(value: unknown): BridgeMoveSession {
  return sessionHeader.parse(value) as BridgeMoveSession;
}

export function decodeLegacyProofTools(
  value: unknown,
): import("./move.ts").ProofTool[] {
  const tools = z.array(z.object({ name: z.string() }).passthrough());
  tools.parse(value);
  // Native compiled catalogs retain exact property order after validation.
  return value as import("./move.ts").ProofTool[];
}
