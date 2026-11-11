import { serverLocalStorage } from "../../../servers/storage.ts";
import { observer } from "mobx-react-lite";
// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import type { Attachment } from "./ComposerAttachments.tsx";
import { AttachmentDrafts } from "../support/useAttachmentDrafts.ts";

const { cancelled } = vi.hoisted(() => ({ cancelled: new Set<string>() }));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerUploads: () => ({
      attachmentCancelled: (id: string) => cancelled.has(id),
    }),
  }),
);

it("retains attachment entries across renders and replaces only changed files", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const host = document.createElement("div");
  const root = createRoot(host);
  const store = new AttachmentDrafts((id) => cancelled.has(id), vi.fn());
  let draft!: readonly [typeof store.current, typeof store.set];
  const View = observer(function View({ version }: { version: number }) {
    void version;
    draft = [store.current, store.set];
    return null;
  });
  let version = 0;
  const asset = (id: string): Attachment => ({
    id,
    image: false,
    mime: "text/plain",
    name: id,
    size: 1,
  });
  try {
    await act(async () =>
      root.render(createElement(View, { version: version++ })),
    );
    const files = [asset("one"), asset("two")];
    const other = [asset("other")];
    await act(async () => {
      await draft[1]({ agent: files, other });
    });
    const initial = draft[0];
    expect(initial.agent).toBe(files);
    await act(async () =>
      root.render(createElement(View, { version: version++ })),
    );
    expect(draft[0]).toBe(initial);
    expect(draft[0].agent).toBe(files);
    cancelled.add("one");
    await act(async () => {
      await store.set((current) => current);
    });
    await act(async () =>
      root.render(createElement(View, { version: version++ })),
    );
    expect(draft[0].agent).not.toBe(files);
    expect(draft[0].agent?.map((file) => file.id)).toEqual(["two"]);
    expect(draft[0].other).toBe(other);
    const filtered = draft[0];
    await act(async () =>
      root.render(createElement(View, { version: version++ })),
    );
    expect(draft[0]).toBe(filtered);
    const replacement = [asset("three")];
    await act(async () => {
      await draft[1]({ agent: replacement, other });
    });
    expect(draft[0].agent).toBe(replacement);
    expect(draft[0].other).toBe(other);
  } finally {
    await act(async () => root.unmount());
    cancelled.clear();
    vi.unstubAllGlobals();
  }
});

import { autorun } from "mobx";
it("reloads attachments from the existing storage key", async () => {
  const key = "codex-agent-attachments:reload";
  serverLocalStorage.removeItem(key);
  const options = { key, storage: serverLocalStorage };
  const first = new AttachmentDrafts(() => false, vi.fn(), options);
  const asset: Attachment = {
    id: "saved",
    name: "saved",
    size: 1,
    mime: "text/plain",
    image: false,
  };
  expect(await first.set({ lead: [asset] })).toBe(true);
  const reopened = new AttachmentDrafts(() => false, vi.fn(), options);
  expect(reopened.current.lead?.map((item) => item.id)).toEqual(["saved"]);
  serverLocalStorage.removeItem(key);
});
it("merges another tab's write and reflects storage events into the observer", async () => {
  const key = "codex-agent-attachments:tabs";
  serverLocalStorage.removeItem(key);
  const options = { key, storage: serverLocalStorage };
  const first = new AttachmentDrafts(() => false, vi.fn(), options);
  const second = new AttachmentDrafts(() => false, vi.fn(), options);
  const observed: string[][] = [];
  const stop = autorun(() => {
    observed.push((first.current.lead || []).map((item) => item.id));
  });
  const asset = (id: string): Attachment => ({
    id,
    name: id,
    size: 1,
    mime: "text/plain",
    image: false,
  });
  await first.set({ lead: [asset("first")] });
  await second.set((current) => ({
    ...current,
    lead: [...(current.lead || []), asset("second")],
  }));
  window.dispatchEvent(
    new StorageEvent("storage", {
      key,
      newValue: serverLocalStorage.getItem(key),
    }),
  );
  expect(observed.at(-1)).toEqual(["first", "second"]);
  expect(
    new AttachmentDrafts(() => false, vi.fn(), options).current.lead?.map(
      (item) => item.id,
    ),
  ).toEqual(["first", "second"]);
  stop();
  serverLocalStorage.removeItem(key);
});
