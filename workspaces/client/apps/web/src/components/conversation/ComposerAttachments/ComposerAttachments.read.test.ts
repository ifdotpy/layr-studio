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
import { MessageAttachments, type Attachment } from "./ComposerAttachments.tsx";
import { MantineProvider } from "@mantine/core";

const hooks = vi.hoisted(() => ({ net: vi.fn(), api: vi.fn() }));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerNet: hooks.net,
    useServerApi: hooks.api,
  }),
);
vi.mock("../FilePreview/FilePreview.tsx", () => ({
  default: () => null,
  useFilePreview: () => [null, vi.fn()],
}));
vi.mock("../PreviewModal/PreviewModal.tsx", () => ({ default: () => null }));

it("loads observed attachment files without an explicit read", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const subscriptions = createSubscriptions({
    watchResourceChanges: () => () => {},
    watchResourceConnection: () => () => {},
  });
  const store = createServerStore(subscriptions);
  const file = {
    name: "image.png",
    mime: "image/png",
    base64: "aGVsbG8=",
  } satisfies GetResult<"/api/file">;
  const fetch = vi.fn(
    async () =>
      new Response(JSON.stringify(file), {
        headers: {
          "Content-Type": "application/json",
          [API_SCHEMA_HASH_HEADER]: API_SCHEMA_HASH,
        },
      }),
  );
  const boundApi = createApiScope({
    origin: "https://server.test",
    fetch,
    localStorage: serverLocalStorage,
    sessionStorage,
    beginMutation: createMutationJournal("fixture", serverLocalStorage),
  });
  const net = createNet(boundApi, () => store);
  const explicit = vi.spyOn(net.reads.file({ asset: "image" }), "read");
  hooks.net.mockReturnValue(net);
  hooks.api.mockReturnValue(boundApi);
  const host = document.createElement("div"),
    root = createRoot(host);
  const asset: Attachment = {
    id: "image",
    name: "image.png",
    image: true,
    mime: "image/png",
    size: 5,
  };
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          { env: "test" },
          createElement(MessageAttachments, {
            assets: [asset],
            notify: vi.fn(),
          }),
        ),
      ),
    );
    expect(explicit).not.toHaveBeenCalled();
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(host.querySelector("img")?.getAttribute("src")).toBe(
      "data:image/png;base64,aGVsbG8=",
    );
  } finally {
    await act(async () => root.unmount());
    store.dispose();
    subscriptions.dispose();
    vi.unstubAllGlobals();
  }
});
