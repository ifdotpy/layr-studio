import { expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ save: vi.fn(), clear: vi.fn() }));
vi.mock("react", () => ({ useRef: (current: unknown) => ({ current }) }));
vi.mock("../unsent/useUnsentText", () => ({
  useUnsentText: () => ['{"old":true}', mocks.save, mocks.clear],
}));
import { useFormDraft } from "./useFormDraft";

it("restores and writes form input through the shared buffer", () => {
  const [initial, update] = useFormDraft("form");
  expect(initial).toEqual({ old: true });
  update({ text: "edited" });
  expect(mocks.save).toHaveBeenCalledWith('{"text":"edited"}');
});
