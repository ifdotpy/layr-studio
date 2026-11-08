import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, test, vi } from "vitest";

test("formats repeated message renders without constructing a formatter per call", async () => {
  const expected = new Date(1700000000 * 1000).toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  });
  const localeString = vi.spyOn(Date.prototype, "toLocaleString");
  const formatter = vi.spyOn(Intl, "DateTimeFormat");
  try {
    const { default: MessageDate } = await import("./MessageDate.tsx");
    const first = renderToStaticMarkup(
      createElement(MessageDate, { at: 1700000000 }),
    );
    const second = renderToStaticMarkup(
      createElement(MessageDate, { at: 1700000000 }),
    );
    expect(first).toBe(second);
    expect(first).toContain(expected);
    expect(localeString).not.toHaveBeenCalled();
    expect(formatter).toHaveBeenCalledTimes(1);
  } finally {
    localeString.mockRestore();
    formatter.mockRestore();
  }
});
