import {
  makeAutoObservable,
  action,
  observableRef,
  onBecomeObserved,
  onBecomeUnobserved,
  autorun,
} from "mobx";
import { resourceData } from "../../../store/runtime.ts";
import { fileFormat, parseDelimited, safeSvg } from "./filePreviewFormats.ts";
import type { PreviewSource } from "../FilePreview/FilePreview.tsx";

export class FilePreviewState {
  readonly selection: PreviewSource;
  failure: unknown = null;
  url: string | undefined = undefined;

  constructor(selection: PreviewSource) {
    this.selection = selection;
    makeAutoObservable(
      this,
      { selection: false, failure: observableRef, mount: action },
      { autoBind: true },
    );
    let stop: (() => void) | undefined;
    onBecomeObserved(this, "url", () => {
      stop = autorun(() => this.mount(this.file), {
        scheduler: queueMicrotask,
      });
    });
    onBecomeUnobserved(this, "url", () => {
      stop?.();
      this.mount(null);
    });
  }

  get file() {
    const result = resourceData(this.selection.slot.current);
    if (!result) return null;
    const bytes = Uint8Array.from(atob(result.base64), (c) => c.charCodeAt(0));
    const format = fileFormat(result.name, result.mime);
    const text = [
      "markdown",
      "csv",
      "tsv",
      "json",
      "text",
      "html",
      "svg",
    ].includes(format)
      ? new TextDecoder().decode(bytes)
      : "";
    return { name: result.name, mime: result.mime, bytes, format, text };
  }

  mount(file: FilePreviewState["file"]) {
    if (this.url) URL.revokeObjectURL(this.url);
    this.url = undefined;
    if (
      !file ||
      !["image", "svg", "audio", "video", "pdf"].includes(file.format)
    )
      return;
    try {
      this.url = URL.createObjectURL(
        new Blob([file.format === "svg" ? safeSvg(file.text) : file.bytes], {
          type: file.format === "svg" ? "image/svg+xml" : file.mime,
        }),
      );
    } catch (error) {
      this.fail(error);
    }
  }

  get error() {
    const slot = this.selection.slot.current;
    return this.failure ?? (slot.status === "error" ? slot.error : null);
  }

  get excerpt() {
    const file = this.file,
      target = this.selection.target;
    if (!file || !target.line) return null;
    let start = 0;
    for (let line = 1; line < target.line; line++) {
      const end = file.text.indexOf("\n", start);
      if (end < 0) return null;
      start = end + 1;
    }
    const end = file.text.indexOf("\n", start),
      stop = end < 0 ? file.text.length : end;
    return {
      before: file.text.slice(0, start),
      line: file.text.slice(start, stop),
      after: file.text.slice(stop),
    };
  }

  get table() {
    const file = this.file;
    return file && ["csv", "tsv"].includes(file.format)
      ? parseDelimited(file.text, file.format === "tsv" ? "\t" : ",")
      : null;
  }

  get json() {
    const file = this.file;
    if (file?.format !== "json") return null;
    try {
      return {
        text: JSON.stringify(JSON.parse(file.text), null, 2),
        error: "",
      };
    } catch {
      return {
        text: file.text,
        error: "Invalid JSON. The original source is shown.",
      };
    }
  }

  fail(error: unknown) {
    this.failure = error;
  }

  *act(operation: () => Promise<unknown>) {
    try {
      yield operation();
    } catch (error) {
      this.fail(error);
    }
  }
}
