import type { Message } from "../../../../types.ts";

/** A provider item whose kind Studio does not know; raw is its JSON text. */
export type ProviderUnknown = NonNullable<Message["providerUnknown"]>;

/** Reads the typed `providerUnknown` field of a stored item. Owner: openapi.json. */
export function providerUnknown(item: Message): ProviderUnknown | undefined {
  return item.providerUnknown;
}

export default function ProviderUnknownItem({ kind, raw }: ProviderUnknown) {
  return (
    <details className="tool-card provider-unknown">
      <summary>{kind}</summary>
      <pre>{raw}</pre>
    </details>
  );
}
