import { serverLocalStorage } from "../../../servers/storage.ts";
// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { createApiScope } from "../../../servers/net/api.ts";
import { createMutationJournal } from "../../../servers/mutationIdentity.ts";
import {
  API_SCHEMA_HASH,
  API_SCHEMA_HASH_HEADER,
} from "../../../generated/apiSchema.ts";
import { createNet } from "../../../servers/net/net.ts";
import { createServerStore } from "../../../store/server.ts";
import { createSubscriptions } from "../../../sync/subscriptions/module.ts";
import type { GetResult } from "../../../api.ts";
import MarkdownImage from "./MarkdownImage.tsx";

const hooks = vi.hoisted(() => ({
  net: vi.fn(),
  api: vi.fn(),
  preview: vi.fn<(...args: unknown[]) => null>(() => null),
}));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerNet: hooks.net,
    useServerApi: hooks.api,
  }),
);
vi.mock("../FilePreview/FilePreview.tsx", () => ({
  default: hooks.preview,
  useFilePreview: () => [null, vi.fn()],
}));
vi.mock("../PreviewModal/PreviewModal.tsx", () => ({ default: () => null }));

it("observes one file read for duplicate images and settles through a source change", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const subscriptions = createSubscriptions({
    watchResourceChanges: () => () => {},
    watchResourceConnection: () => () => {},
  });
  const store = createServerStore(subscriptions);
  const pending = Promise.withResolvers<GetResult<"/api/file">>();
  const fetch = vi.fn(() =>
    pending.promise.then(
      (data) =>
        new Response(JSON.stringify(data), {
          headers: {
            "Content-Type": "application/json",
            [API_SCHEMA_HASH_HEADER]: API_SCHEMA_HASH,
          },
        }),
    ),
  );
  const boundApi = createApiScope({
    origin: "https://server.test",
    fetch,
    localStorage: serverLocalStorage,
    sessionStorage,
    beginMutation: createMutationJournal("fixture", serverLocalStorage),
  });
  const net = createNet(boundApi, () => store);
  const files = vi.spyOn(net.reads, "file");
  hooks.net.mockReturnValue(net);
  hooks.api.mockReturnValue(boundApi);
  const host = document.createElement("div"),
    root = createRoot(host);
  const local = () =>
    createElement(MarkdownImage, {
      src: "./image.png",
      alt: "local",
      agentId: "agent",
    });
  try {
    await act(async () =>
      root.render(createElement("div", null, local(), local())),
    );
    expect(files).toHaveBeenCalled();
    expect(fetch).toHaveBeenCalledTimes(1);
    await act(async () =>
      root.render(
        createElement(MarkdownImage, {
          src: "https://images.test/remote.png",
          alt: "remote",
        }),
      ),
    );
    expect(host.textContent).toContain("Load image");
    await act(async () =>
      pending.resolve({
        name: "image.png",
        mime: "image/png",
        base64: "aGVsbG8=",
      }),
    );
    expect(host.querySelector("img")).toBeNull();
    expect(fetch).toHaveBeenCalledTimes(1);
    await act(async () => root.render(local()));
    expect(host.querySelector("img")?.getAttribute("src")).toBe(
      "data:image/png;base64,aGVsbG8=",
    );
    expect(fetch).toHaveBeenCalledTimes(2);
  } finally {
    await act(async () => root.unmount());
    store.dispose();
    subscriptions.dispose();
    vi.unstubAllGlobals();
  }
});

it("uses the current server after an image provider rerender", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const sources = ["QQ==", "Qg=="].map((base64) => {
    const subscriptions = createSubscriptions({
      watchResourceChanges: () => () => {},
      watchResourceConnection: () => () => {},
    });
    const store = createServerStore(subscriptions);
    const api = createApiScope({
      origin: "https://server.test",
      fetch: async () =>
        new Response(
          JSON.stringify({ name: "image.png", mime: "image/png", base64 }),
          {
            headers: {
              "Content-Type": "application/json",
              [API_SCHEMA_HASH_HEADER]: API_SCHEMA_HASH,
            },
          },
        ),
      localStorage: serverLocalStorage,
      sessionStorage,
      beginMutation: createMutationJournal("fixture", serverLocalStorage),
    });
    return {
      net: createNet(api, () => store),
      dispose() {
        store.dispose();
        subscriptions.dispose();
      },
    };
  });
  const host = document.createElement("div"),
    root = createRoot(host);
  const render = (index: number) =>
    act(async () => {
      hooks.net.mockReturnValue(sources[index]?.net);
      root.render(
        createElement(MarkdownImage, {
          src: "image.png",
          alt: "image",
          agentId: "agent",
        }),
      );
    });
  try {
    await render(0);
    expect(host.querySelector("img")?.getAttribute("src")).toBe(
      "data:image/png;base64,QQ==",
    );
    await render(1);
    expect(host.querySelector("img")?.getAttribute("src")).toBe(
      "data:image/png;base64,Qg==",
    );
    const net = sources[1]!.net,
      slot = net.reads.file({ agent: "agent", path: "image.png" });
    const retry = vi.spyOn(slot, "read");
    await act(async () =>
      host
        .querySelector("img")
        ?.dispatchEvent(new Event("error", { bubbles: true })),
    );
    await act(async () =>
      host
        .querySelector<HTMLButtonElement>(".markdown-image-placeholder button")
        ?.click(),
    );
    expect(retry).toHaveBeenCalledOnce();
    await act(async () =>
      host.querySelector<HTMLButtonElement>(".image-preview-button")?.click(),
    );
    expect(retry).toHaveBeenCalledTimes(2);
    expect(hooks.preview.mock.lastCall?.[0]).toEqual(
      expect.objectContaining({ target: expect.objectContaining({ slot }) }),
    );
  } finally {
    await act(async () => root.unmount());
    sources.forEach((source) => source.dispose());
    vi.unstubAllGlobals();
  }
});
