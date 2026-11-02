import type { Message } from "../../../types";

/** A provider item whose kind Studio does not know, kept as the raw provider item. */
export type ProviderUnknown = NonNullable<Message["providerUnknown"]>;

/** Reads the typed `providerUnknown` field of a stored item. Owner: openapi.json. */
export function providerUnknown(item: Message): ProviderUnknown | undefined {
  return item.providerUnknown;
}

export default function ProviderUnknownItem({ kind, raw }: ProviderUnknown) {
  return (
    <details className="tool-card provider-unknown">
      <summary>{kind}</summary>
      <pre>{JSON.stringify(raw, null, 2)}</pre>
    </details>
  );
}
