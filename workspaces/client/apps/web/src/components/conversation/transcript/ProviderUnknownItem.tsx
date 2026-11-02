import type { Message } from "../../../types";

/** A provider item whose kind Studio does not know, kept as the raw provider item. */
export type ProviderUnknown = { kind: string; raw: unknown };

/** Reads `providerUnknown` from a stored item. Owner: notification_dispatch/items.rs. */
export function providerUnknown(item: Message): ProviderUnknown | undefined {
  const value: unknown = (item as { providerUnknown?: unknown })
    .providerUnknown;
  if (typeof value !== "object" || value === null) return undefined;
  const { kind, raw } = value as { kind?: unknown; raw?: unknown };
  return typeof kind === "string" ? { kind, raw } : undefined;
}

export default function ProviderUnknownItem({ kind, raw }: ProviderUnknown) {
  return (
    <details className="tool-card provider-unknown">
      <summary>{kind}</summary>
      <pre>{JSON.stringify(raw, null, 2)}</pre>
    </details>
  );
}
