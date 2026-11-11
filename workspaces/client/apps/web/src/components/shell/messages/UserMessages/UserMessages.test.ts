// @vitest-environment happy-dom
import { ComplaintStatus } from "../../../../generated/api.ts";
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import { conversationActionsFixture } from "../../integration-support/observableFixture.tsx";
import { ServerAreasContextProvider as ConversationProvider } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { createSubscriptions } from "../../../../sync/subscriptions/module.ts";
import { createServerStore } from "../../../../store/server.ts";
import { ServerStoreProvider } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import type {
  ResourceRef,
  ResourceVersion,
} from "../../../../sync/resourceEvents.ts";
import type { components } from "../../../../generated/api.ts";

const api = vi.hoisted(() => ({ get: vi.fn(), post: vi.fn() }));
vi.mock(
  "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  async (importOriginal) => ({
    ...(await importOriginal<
      typeof import("../../../servers/ServerServicesProvider/ServerServicesProvider.tsx")
    >()),
    useServerApi: () => api,
  }),
);
const durableRows = vi.hoisted(() => new Map<string, unknown>());
vi.mock("../../../../indexedDb.ts", () => ({
  indexedDb: () => ({
    get: async (key: string) => durableRows.get(key),
    put: async (key: string, value: unknown) => {
      durableRows.set(key, value);
    },
    delete: async (key: string) => {
      durableRows.delete(key);
    },
    update: async (key: string, change: (value: unknown) => unknown) => {
      const next = change(durableRows.get(key));
      if (next === undefined) durableRows.delete(key);
      else durableRows.set(key, next);
    },
  }),
}));
import { unsentStore } from "../../../../unsent/store.ts";
import UserMessages from "./UserMessages.tsx";

type Detail = components["schemas"]["ComplaintDetailResponse"];
it("real complaint subscription isolates detail, retains confirmed reply against stale detail, and separates reload failure", async () => {
  vi.useFakeTimers();
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  Object.defineProperty(document, "fonts", {
    configurable: true,
    value: new EventTarget(),
  });
  const changes = new Map<string, (version?: ResourceVersion) => void>();
  const released: ResourceRef[] = [];
  const subscriptions = createSubscriptions({
    watchResourceChanges: (ref, accept) => {
      changes.set(JSON.stringify(ref), accept);
      return () => {
        released.push(ref);
      };
    },
    watchResourceConnection: (accept) => {
      accept("live", "open", {});
      return () => {};
    },
  });
  const store = createServerStore(subscriptions);
  const actions = conversationActionsFixture();
  const notify = vi.fn();
  actions.notices.notify = notify;
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const counts = new Map<string, number>();
  window.__studioMessageRenderProbe = (leaf, id) => {
    const key = leaf + ":" + (id || "");
    counts.set(key, (counts.get(key) || 0) + 1);
  };
  let revision = 0;
  const publish = <R extends ResourceRef>(
    ref: R,
    value: import("../../../../sync/subscriptions/contracts.ts").ResourceData<R>,
  ) => {
    changes.get(JSON.stringify(ref))?.({
      epoch: "test",
      revision: ++revision,
      cursor: { workspaceId: "w", seq: revision },
      replyKind: "delta",
    });
    subscriptions.publish(ref, value);
  };
  const first: Detail = {
    id: "a",
    recipient: "user",
    version: 1,
    text: "Please confirm",
    responses: [],
  };
  const sibling: Detail = { ...first, id: "b", text: "Sibling message" };
  const confirmed: Detail = {
    ...first,
    version: 3,
    responses: [
      {
        id: "reply",
        author: "user",
        at: 1,
        text: "Original reply",
        status: ComplaintStatus.in_progress,
      },
    ],
  };
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          {},
          createElement(
            ServerStoreProvider,
            { value: store },
            createElement(
              ConversationProvider,
              { value: actions },
              createElement(UserMessages),
            ),
          ),
        ),
      ),
    );
    await act(async () => vi.advanceTimersByTimeAsync(100));
    await act(async () =>
      publish({ kind: "state" }, [
        {
          collection: "workspace",
          id: "w",
          value: { stateDir: "message-requests-test" },
        },
        ...[first, sibling].map((value) => ({
          collection: "complaint" as const,
          id: value.id,
          value,
        })),
      ]),
    );
    await act(async () => vi.advanceTimersByTimeAsync(100));
    await act(async () => {
      publish({ kind: "complaint", complaintId: "a" }, first);
      publish({ kind: "complaint", complaintId: "b" }, sibling);
    });
    expect(host.textContent).toContain("Please confirm");
    counts.clear();
    await act(async () =>
      publish(
        { kind: "complaint", complaintId: "a" },
        { ...first, text: "Updated detail" },
      ),
    );
    expect(host.textContent).toContain("Updated detail");
    expect(counts.get("user-message:a")).toBe(1);
    expect(counts.get("user-message:b") || 0).toBe(0);
    const card = host.querySelector('[data-complaint="a"]');
    const reply = Array.from(card?.querySelectorAll("button") || []).find(
      (button) => button.textContent === "Reply",
    );
    await act(async () => reply?.click());
    const input = card?.querySelector("textarea");
    if (!input) throw new Error("Reply missing");
    await act(async () => {
      Object.getOwnPropertyDescriptor(
        HTMLTextAreaElement.prototype,
        "value",
      )?.set?.call(input, "Original reply");
      input.dispatchEvent(new Event("input", { bubbles: true }));
    });
    api.post.mockRejectedValueOnce(new TypeError("connection lost"));
    const submit = async () =>
      act(async () => {
        card
          ?.querySelector("form")
          ?.dispatchEvent(
            new Event("submit", { bubbles: true, cancelable: true }),
          );
      });
    await submit();
    expect(card?.textContent).toContain("Retry response");
    const original = api.post.mock.calls[0]?.[1];
    await act(async () => {
      Object.getOwnPropertyDescriptor(
        HTMLTextAreaElement.prototype,
        "value",
      )?.set?.call(input, "Newer draft B");
      input.dispatchEvent(new Event("input", { bubbles: true }));
    });
    const draftId = JSON.stringify([
      "studio-message-responses:message-requests-test",
      "a",
    ]);
    expect(await unsentStore("local", "reply").get(draftId)).toBe(
      "Newer draft B",
    );
    await act(async () =>
      publish(
        { kind: "complaint", complaintId: "a" },
        { ...first, version: 2, text: "Newer facts" },
      ),
    );
    api.post.mockResolvedValueOnce({});
    api.get.mockResolvedValueOnce(confirmed);
    await submit();
    expect(api.post.mock.calls[1]?.[1]).toEqual(original);
    expect(card?.textContent).toContain("Original reply");
    await act(async () =>
      publish({ kind: "complaint", complaintId: "a" }, first),
    );
    expect(card?.querySelector(".complaint-response")?.textContent).toContain(
      "Original reply",
    );
    expect(await unsentStore("local", "reply").get(draftId)).toBe(
      "Newer draft B",
    );
    expect(input.value).toBe("Newer draft B");
    await act(async () => {
      Object.getOwnPropertyDescriptor(
        HTMLTextAreaElement.prototype,
        "value",
      )?.set?.call(input, "Next reply");
      input.dispatchEvent(new Event("input", { bubbles: true }));
    });
    api.post.mockResolvedValueOnce({});
    api.get.mockRejectedValueOnce(new Error("detail reload failed"));
    await submit();
    expect(card?.textContent).not.toContain("Retry response");
    expect(input.value).toBe("");
    expect(notify).toHaveBeenCalledWith("detail reload failed");
    await act(async () => root.unmount());
    await act(async () => vi.advanceTimersByTimeAsync(30001));
    expect(
      released.some(
        (ref) => ref.kind === "complaint" && ref.complaintId === "a",
      ),
    ).toBe(true);
  } finally {
    await act(async () => root.unmount());
    store.dispose();
    subscriptions.dispose();
    host.remove();
    delete window.__studioMessageRenderProbe;
    vi.useRealTimers();
    vi.unstubAllGlobals();
  }
});
