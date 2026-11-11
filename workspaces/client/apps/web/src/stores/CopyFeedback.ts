import { makeAutoObservable } from "mobx";
import { copyText } from "../clipboard/clipboard";
import { reached } from "../clock";

/** How long a "copied" confirmation stays visible, in milliseconds. */
const COPIED_VISIBLE_MS = 1500;

/** Copies text to the clipboard and shows a short confirmation for the copied source. */
export class CopyFeedback {
  /** Source key of the last successful copy and its expiry (epoch ms). */
  private last: { source: string; until: number } | undefined = undefined;
  failed = false;

  constructor() {
    makeAutoObservable(this, {}, { autoBind: true });
  }

  /** Whether `source` (default: the only source) was copied recently. */
  copied(source = ""): boolean {
    return this.last?.source === source && !reached(this.last.until);
  }

  *copy(text: string, source = ""): Generator<Promise<void>, void, void> {
    this.failed = false;
    try {
      yield copyText(text);
      this.last = { source, until: Date.now() + COPIED_VISIBLE_MS };
    } catch {
      this.failed = true;
    }
  }
}
