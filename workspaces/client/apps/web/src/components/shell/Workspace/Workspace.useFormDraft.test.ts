import { autorun, flowResult } from "mobx";
import { afterEach, expect, it, vi } from "vitest";
import { ApiError } from "../../../api.ts";
import * as serverApi from "../../../servers/net/api.ts";
import { UnsentText } from "../../../unsent/useUnsentText.ts";
import { FormDraft } from "../../../app/shell/FormDraft.ts";
afterEach(() => vi.restoreAllMocks());
const saved = vi.hoisted(() => new Map<string, string>());
vi.mock("../../../unsent/store.ts", async (original) => ({
  ...(await original<typeof import("../../../unsent/store.ts")>()),
  reportUnsentFailure() {},
  unsentStore: () => ({
    get: async (key: string) => saved.get(key),
    put: async (key: string, value: string) => {
      saved.set(key, value);
    },
    deleteIfEqual: async (key: string, expected: string) => {
      if (saved.get(key) === expected) saved.delete(key);
    },
  }),
}));

it("restores and writes form input through the shared buffer", () => {
  const text = new UnsentText("form-test");
  text.update("form", "form", '{"old":true}');
  const form = new FormDraft(text, "form");
  const stop = autorun(() => form.value);
  expect(form.value).toEqual({ old: true });
  form.update({ text: "edited" });
  expect(text.cached("form", "form")).toBe('{"text":"edited"}');
  stop();
});

it("keeps a newer edit when an older form submission completes", async () => {
  const text = new UnsentText("form-edit-test");
  const form = new FormDraft(text, "form");
  form.update({ text: "submitted" });
  const stop = autorun(() => form.value);
  const submitted = form.serialized;
  form.update({ text: "newer" });
  expect(form.value).toEqual({ text: "newer" });
  await flowResult(form.clear(submitted));
  expect(form.value).toEqual({ text: "newer" });
  stop();
});

import { WorkspacePanel } from "../../../app/shell/WorkspaceStore.ts";

it("conditionally clears a successful durable save after the form unmounts", async () => {
  const text = new UnsentText("unmounted-form");
  const form = new FormDraft(text, "saved-after-unmount");
  form.update({ name: "submitted" });
  const stop = autorun(() => form.value);
  const pending = Promise.withResolvers<{}>();
  vi.spyOn(serverApi, "post").mockReturnValue(pending.promise);
  const panel = new WorkspacePanel();
  const completion = panel.submit(
    { run: serverApi.post },
    "/api/profiles",
    { action: "save" },
    () => {},
    form,
  );
  stop();
  await Promise.resolve();
  pending.resolve({});
  await completion;
  expect(saved.has("saved-after-unmount")).toBe(false);
});

it("preserves a newer durable edit when an unmounted save completes", async () => {
  const text = new UnsentText("edited-unmounted-form");
  const form = new FormDraft(text, "edited-after-submit");
  form.update({ name: "submitted" });
  const stop = autorun(() => form.value);
  const pending = Promise.withResolvers<{}>();
  vi.spyOn(serverApi, "post").mockReturnValue(pending.promise);
  const completion = new WorkspacePanel().submit(
    { run: serverApi.post },
    "/api/profiles",
    { action: "save" },
    () => {},
    form,
  );
  form.update({ name: "newer" });
  stop();
  await Promise.resolve();
  pending.resolve({});
  await completion;
  expect(saved.get("edited-after-submit")).toBe('{"name":"newer"}');
});

it.each([
  ["definite rejection", new ApiError("Refused", 400), false],
  ["uncertain failure", new ApiError("Unavailable", 503), true],
])(
  "preserves conditional durable clear semantics after %s",
  async (_name, error, retained) => {
    const text = new UnsentText("rejected-form");
    const form = new FormDraft(text, "rejected-draft");
    form.update({ name: "submitted" });
    const stop = autorun(() => form.value);
    vi.spyOn(serverApi, "post").mockRejectedValue(error);
    await new WorkspacePanel().submit(
      { run: serverApi.post },
      "/api/profiles",
      { action: "save" },
      undefined,
      form,
    );
    expect(saved.has("rejected-draft")).toBe(retained);
    stop();
  },
);
