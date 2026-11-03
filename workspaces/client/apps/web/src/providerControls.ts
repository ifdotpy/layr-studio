import type { ProviderCapability } from "./generated/providerKind.ts";

export type CapabilityControls = { steer: boolean; fork: boolean };

// A control shows only when the provider's capability list has its capability.
export function capabilityControls(
  capabilities: readonly ProviderCapability[],
): CapabilityControls {
  return {
    steer: capabilities.includes("steer"),
    fork: capabilities.includes("fork"),
  };
}
