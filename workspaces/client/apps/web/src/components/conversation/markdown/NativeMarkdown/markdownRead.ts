import {
  makeAutoObservable,
  observableRef,
  onBecomeObserved,
  onBecomeUnobserved,
} from "mobx";
import { hasNativeClient, nativeRequest } from "../../../../native/client.ts";
import { parseMarkdown } from "../../../../wasm/client.ts";
import { completedMarkdown } from "../../../../native/markdownCache.ts";
import type { MarkdownBlock } from "../../../../generated/rustOwned.ts";

const reads = new Map<string, MarkdownRead>();
class MarkdownRead {
  blocks: MarkdownBlock[] | undefined;
  error = "";
  constructor(text: string) {
    this.blocks = completedMarkdown.get(text) as MarkdownBlock[] | undefined;
    makeAutoObservable(this, { blocks: observableRef }, { autoBind: true });
    const controller = new AbortController();
    onBecomeObserved(
      this,
      "blocks",
      () => void this.load(text, controller.signal),
    );
    onBecomeUnobserved(this, "blocks", () => {
      controller.abort();
      reads.delete(text);
    });
  }
  *load(
    text: string,
    signal: AbortSignal,
  ): Generator<Promise<MarkdownBlock[]>, void, MarkdownBlock[]> {
    if (this.blocks) return;
    try {
      const blocks = yield hasNativeClient()
        ? nativeRequest<MarkdownBlock[]>(
            "markdown.parse",
            { text, streaming: false },
            undefined,
            signal,
          )
        : parseMarkdown(text);
      if (!signal.aborted) {
        completedMarkdown.set(text, blocks);
        this.blocks = blocks;
      }
    } catch (error) {
      if (!signal.aborted)
        this.error = error instanceof Error ? error.message : String(error);
    }
  }
}

/** Canonical content reads live only while their blocks are observed. */
export function markdownRead(text: string) {
  let read = reads.get(text);
  if (!read) reads.set(text, (read = new MarkdownRead(text)));
  return read;
}
