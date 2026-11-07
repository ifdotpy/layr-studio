import { ByteBudgetLru } from "../byteBudgetLru";

export type CompiledMarkdownBlock =
  | { readonly html: string; readonly kind?: never; readonly source?: never }
  | {
      readonly kind: "html" | "mermaid";
      readonly source: string;
      readonly html?: never;
    };

// Retain only completed source and compiled strings, never tokens or React nodes.
// Charge UTF-16 source/output plus bookkeeping per entry and block.
// These are cache accounting limits, not a measurement of the JavaScript heap.
export class CompletedMarkdownCache {
  private readonly entries: ByteBudgetLru<readonly CompiledMarkdownBlock[]>;

  constructor(maxBytes?: number, maxEntries?: number) {
    this.entries = new ByteBudgetLru(maxBytes, maxEntries);
  }

  get(text: string) {
    return this.entries.get(text);
  }

  set(text: string, blocks: readonly CompiledMarkdownBlock[]) {
    const bytes =
      128 +
      blocks.length * 64 +
      2 *
        (text.length +
          blocks.reduce(
            (total, block) => total + (block.html ?? block.source).length,
            0,
          ));
    this.entries.set(text, blocks, bytes);
  }
}
