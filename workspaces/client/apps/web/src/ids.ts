// Branded ids of the Rust-owned contract. A plain string becomes an id only through
// these functions, where the caller states which kind of id it holds.
import type {
  AgentId,
  ApprovalId,
  CommandId,
  TurnId,
} from "./generated/rustOwned";

export const agentId = (value: string) => value as AgentId;
export const approvalId = (value: string) => value as ApprovalId;
export const commandId = (value: string) => value as CommandId;
export const turnId = (value: string) => value as TurnId;
