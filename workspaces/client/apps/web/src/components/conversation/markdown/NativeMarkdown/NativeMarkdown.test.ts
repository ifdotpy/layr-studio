import { createElement } from "react";
import { autorun } from "mobx";
import { markdownRead } from "./markdownRead.ts";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, expect, it, vi } from "vitest";
import NativeMarkdown from "./NativeMarkdown.tsx";
import { NativeMarkdownBlock } from "./NativeMarkdown.tsx";
import { completedMarkdown } from "../../../../native/markdownCache.ts";

const { request } = vi.hoisted(() => ({
  request: vi.fn(
    async (
      _method: string,
      _params: unknown,
      _options: unknown,
      _signal: AbortSignal,
    ) => [],
  ),
}));
vi.mock("../../../../native/client.ts", async (original) => ({
  ...(await original<typeof import("../../../../native/client.ts")>()),
  hasNativeClient: () => true,
  nativeRequest: request,
}));
afterEach(() => {
  vi.useRealTimers();
  request.mockClear();
  vi.restoreAllMocks();
});

it("shows streaming markdown literally without invoking the parser", async () => {
  vi.useFakeTimers();
  const text = "**Still streaming**\n\n```ts\nconst x = 1;";
  const html = renderToStaticMarkup(
    createElement(NativeMarkdown, { text, streaming: true }),
  );
  await vi.runAllTimersAsync();
  expect(html).toContain("**Still streaming**");
  expect(html).toContain("white-space:pre-wrap");
  expect(request).not.toHaveBeenCalled();
});

it("reuses the cached completed tree without parsing or repeated decoding", () => {
  const text = "Cached completed paragraph.";
  completedMarkdown.set(text, [
    { length: text.length, nodes: [{ tag: "p", children: [{ text }] }] },
  ]);
  const read = vi.spyOn(completedMarkdown, "get");
  const html = renderToStaticMarkup(
    createElement(NativeMarkdown, { text, streaming: false }),
  );
  expect(html).toContain(text);
  expect(read).toHaveBeenCalledTimes(1);
});

it("renders the native tree as text and preserves file links", () => {
  const html = renderToStaticMarkup(
    createElement(NativeMarkdownBlock, {
      block: {
        length: 5,
        nodes: [
          {
            tag: "p",
            children: [
              {
                text: "<script>",
                sentences: [
                  { text: "<script>", index: 0 },
                  { text: "🦀 New", index: 8 },
                ],
              },
              {
                tag: "a",
                props: { href: "src/file.ts:12" },
                children: [{ text: "file" }],
              },
            ],
          },
        ],
      },
    }),
  );
  expect(html).toContain("&lt;script&gt;");
  expect(html).not.toContain("<script>");
  expect(html).toContain("🦀 New");
  expect(html).toContain('href="src/file.ts:12"');
});

it("renders live text literally even when a completed tree has the same source", async () => {
  vi.useFakeTimers();
  const text = "Streaming source.";
  completedMarkdown.set(text, []);
  const html = renderToStaticMarkup(
    createElement(NativeMarkdown, { text, streaming: true }),
  );
  await vi.runAllTimersAsync();
  expect(html).toContain(text);
  expect(request).not.toHaveBeenCalled();
});

it("starts parsing on observation, cancels and evicts when unobserved", () => {
  vi.useFakeTimers();
  const text = "Uncached finished history.";
  const read = markdownRead(text);
  const stop = autorun(() => read.blocks);
  expect(request).toHaveBeenCalledTimes(1);
  expect(markdownRead(text)).toBe(read);
  expect(vi.getTimerCount()).toBe(0);
  const signal = request.mock.calls[0]?.[3];
  stop();
  expect(signal?.aborted).toBe(true);
  expect(markdownRead(text)).not.toBe(read);
});

it("shares an observed parse and releases it only after the last observer", () => {
  const text = "Shared observed paragraph.";
  const read = markdownRead(text);
  const first = autorun(() => read.blocks);
  const second = autorun(() => markdownRead(text).blocks);
  expect(request).toHaveBeenCalledTimes(1);
  first();
  expect(request.mock.calls[0]?.[3].aborted).toBe(false);
  expect(markdownRead(text)).toBe(read);
  second();
  expect(request.mock.calls[0]?.[3].aborted).toBe(true);
  expect(markdownRead(text)).not.toBe(read);
});

it("publishes the parsed blocks to their observer", async () => {
  const text = "Observed finished paragraph.";
  const read = markdownRead(text);
  const seen: unknown[] = [];
  const stop = autorun(() => seen.push(read.blocks));
  await Promise.resolve();
  expect(seen).toEqual([undefined, []]);
  expect(completedMarkdown.get(text)).toEqual([]);
  stop();
});
