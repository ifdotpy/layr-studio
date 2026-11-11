// @vitest-environment happy-dom
import { autorun } from "mobx";
import { expect, it, vi } from "vitest";
import { richPreviewRead } from "./RichPreview.tsx";

const render = vi.hoisted(() => vi.fn());
vi.mock("mermaid", () => ({ default: { initialize: vi.fn(), render } }));

it("shares one observed render, lets it settle across unobserve, then evicts", async () => {
  let resolve!: (value: { svg: string }) => void;
  render.mockReturnValue(
    new Promise<{ svg: string }>((done) => {
      resolve = done;
    }),
  );
  const read = richPreviewRead("mermaid", "graph TD; A-->B");
  expect(render).not.toHaveBeenCalled();
  const stop = autorun(() => {
    void read.svg;
  });
  await vi.waitFor(() => expect(render).toHaveBeenCalledTimes(1));
  const same = richPreviewRead("mermaid", "graph TD; A-->B");
  const stopSecond = autorun(() => {
    void same.svg;
  });
  expect(same).toBe(read);
  stop();
  stopSecond();
  expect(richPreviewRead("mermaid", "graph TD; A-->B")).toBe(read);
  resolve({ svg: "<svg/>" });
  await vi.waitFor(() => expect(read.svg).toBe("<svg/>"));
  expect(richPreviewRead("mermaid", "graph TD; A-->B")).not.toBe(read);
  expect(render).toHaveBeenCalledTimes(1);
});

it("separates preview kinds and evicts completed unobserved reads", () => {
  const read = richPreviewRead("html", "same");
  const stop = autorun(() => {
    void read.svg;
  });
  expect(richPreviewRead("html", "same")).toBe(read);
  expect(richPreviewRead("mermaid", "same")).not.toBe(read);
  stop();
  expect(richPreviewRead("html", "same")).not.toBe(read);
});
