import { observer } from "mobx-react-lite";
// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import type { Attachment } from "./ComposerAttachments.tsx";
import { useAttachmentDrafts } from "../support/useAttachmentDrafts.ts";

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
  let draft!: ReturnType<typeof useAttachmentDrafts>;
  const View = observer(function View({ version }: { version: number }) {
    void version;
    draft = useAttachmentDrafts("chat", vi.fn());
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
