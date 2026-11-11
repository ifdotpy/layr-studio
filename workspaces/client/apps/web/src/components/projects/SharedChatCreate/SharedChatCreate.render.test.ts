import { ProviderKind, AccountStatus } from "../../../generated/api.ts";
// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { makeAutoObservable } from "mobx";
import { expect, it, vi } from "vitest";
import type { useWorkerModels } from "../../agents/support/WorkerModelPicker.tsx";
import type { SharedCreation } from "../../../app/projects/CreationOwner.ts";
import type { SharedCreationSource } from "./SharedChatCreate.tsx";

const h = vi.hoisted(() => ({
  post: vi.fn(),
  catalogs: {
    workerCatalogs: new Map<string, ReturnType<typeof useWorkerModels>>(),
  },
}));
vi.mock("../../../app/ServerAreasProvider.tsx", () => ({
  useServerAreas: () => ({ agentsUi: h.catalogs }),
}));
vi.mock(
  "../../servers/ServerServicesProvider/ServerServicesProvider.tsx",
  () => ({
    useServerApi: () => ({ post: h.post }),
    useServerStorage: () => undefined,
  }),
);
vi.mock("../../agents/AgentSetupPicker/AgentSetupPicker.tsx", () => ({
  AgentSetupPicker: () => null,
}));
import SharedChatCreate from "./SharedChatCreate.tsx";

it("reuses the frozen shared-chat request after a lost response and remount", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const body: SharedCreation["body"] = {
    action: "radio",
    radio_action: "create",
    request_id: "saved-request",
    path: "/project",
    name: "Shared",
    participants: [
      { account_key: "first", model: "model" },
      { account_key: "second", model: "model" },
    ],
  };
  const creation = makeAutoObservable<SharedCreationSource["creation"]>(
    {
      sharedAttempt: { body },
      rememberShared(value: SharedCreation | null) {
        this.sharedAttempt = value;
      },
    },
    {},
    { autoBind: true },
  );
  const source: SharedCreationSource = {
    creation,
    startup: { creationScope: "fixture" },
    state: undefined,
  };
  const accounts = {
    accounts: [],
    archivedAccounts: [],
    defaultAccountKey: "default",
    defaultAccountGroups: [],
    supportsDelete: true,
    supportsDisconnect: true,
  };
  h.post.mockRejectedValue(new Error("Response lost"));
  const host = document.body.appendChild(document.createElement("div"));
  let root = createRoot(host);
  const mount = async () =>
    act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(SharedChatCreate, { source, accounts }),
        ),
      ),
    );
  const submit = async () => {
    const form = host.querySelector("form");
    if (!form) throw Error("Missing shared-chat form");
    await act(async () =>
      form.dispatchEvent(
        new Event("submit", { bubbles: true, cancelable: true }),
      ),
    );
  };
  try {
    await mount();
    await submit();
    expect(h.post).toHaveBeenCalledTimes(1);
    await act(async () => root.unmount());
    root = createRoot(host);
    await mount();
    await submit();
    expect(h.post).toHaveBeenCalledTimes(2);
    expect(h.post.mock.calls.map((call) => call[1])).toEqual([body, body]);
    expect(creation.sharedAttempt?.body.request_id).toBe("saved-request");
    expect(host.textContent).toContain("Response lost");
  } finally {
    await act(async () => root.unmount());
    host.remove();
    vi.unstubAllGlobals();
  }
});

it("keeps a deferred shared receipt on its captured scope after unmount", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const body: SharedCreation["body"] = {
    action: "radio",
    radio_action: "create",
    request_id: "scope-a-request",
    path: "/a",
    name: "Shared",
    participants: [],
  };
  const startup = { creationScope: "A" };
  const remember = vi.fn();
  const creation = makeAutoObservable<SharedCreationSource["creation"]>(
    {
      sharedAttempt: { body },
      rememberShared(value: SharedCreation | null, scope?: string) {
        remember(value, scope ?? startup.creationScope);
        if (!scope || scope === startup.creationScope)
          this.sharedAttempt = value;
      },
    },
    {},
    { autoBind: true },
  );
  const pending = Promise.withResolvers<{ room: { id: string } }>();
  h.post.mockReset().mockReturnValue(pending.promise);
  const host = document.body.appendChild(document.createElement("div")),
    root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(SharedChatCreate, {
            source: { creation, startup, state: undefined },
            accounts: {
              accounts: [],
              archivedAccounts: [],
              defaultAccountGroups: [],
              defaultAccountKey: "",
              supportsDelete: true,
              supportsDisconnect: true,
            },
          }),
        ),
      ),
    );
    const form = host.querySelector("form");
    if (!form) throw Error("Missing form");
    await act(async () =>
      form.dispatchEvent(
        new Event("submit", { bubbles: true, cancelable: true }),
      ),
    );
    await act(async () => root.render(null));
    startup.creationScope = "B";
    const newer = { body: { ...body, request_id: "scope-b-request" } };
    creation.rememberShared(newer);
    await act(async () => pending.resolve({ room: { id: "room-a" } }));
    expect(remember).toHaveBeenLastCalledWith(
      expect.objectContaining({ roomId: "room-a" }),
      "A",
    );
    expect(creation.sharedAttempt).toEqual(newer);
  } finally {
    await act(async () => root.unmount());
    host.remove();
    vi.unstubAllGlobals();
  }
});

it("enables first creation from the scoped catalogs and captures its generated request", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const catalog: ReturnType<typeof useWorkerModels> = {
    models: [
      {
        model: "default-model",
        isDefault: true,
        serviceTiers: [],
        supportedReasoningEfforts: [],
      },
    ],
    loading: false,
    error: "",
    retry: () => {},
  };
  h.catalogs.workerCatalogs.clear();
  h.catalogs.workerCatalogs.set(JSON.stringify(["codex", false]), catalog);
  h.catalogs.workerCatalogs.set(JSON.stringify(["claude", false]), catalog);
  const creation = makeAutoObservable<SharedCreationSource["creation"]>(
    {
      sharedAttempt: null,
      rememberShared(value: SharedCreation | null) {
        this.sharedAttempt = value;
      },
    },
    {},
    { autoBind: true },
  );
  const accounts: React.ComponentProps<typeof SharedChatCreate>["accounts"] = {
    accounts: [
      {
        id: "codex",
        label: "Codex",
        provider: ProviderKind.codex,
        status: AccountStatus.ready,
        home: "/codex",
        source: "fixture",
        eligibility: { kind: "available" },
      },
      {
        id: "claude",
        label: "Claude",
        provider: ProviderKind.claude,
        status: AccountStatus.ready,
        home: "/claude",
        source: "fixture",
        eligibility: { kind: "available" },
      },
    ],
    archivedAccounts: [],
    defaultAccountGroups: [],
    defaultAccountKey: "codex",
    supportsDelete: true,
    supportsDisconnect: true,
  };
  h.post.mockReset().mockResolvedValue({ room: { id: "created" } });
  const host = document.body.appendChild(document.createElement("div")),
    root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(SharedChatCreate, {
            source: {
              creation,
              startup: { creationScope: "fixture" },
              state: undefined,
            },
            accounts,
            initialPath: "/project",
          }),
        ),
      ),
    );
    const button = host.querySelector<HTMLButtonElement>(
      'button[type="submit"]',
    );
    if (!button) throw Error("Missing submit");
    expect(button.disabled).toBe(false);
    await act(async () => button.click());
    expect(h.post).toHaveBeenCalledWith(
      "/api/peer-teams",
      expect.objectContaining({
        path: "/project",
        request_id: expect.any(String),
        participants: [
          { account_key: "codex", model: "default-model" },
          { account_key: "claude", model: "default-model" },
        ],
      }),
      { timeoutMs: 15000 },
    );
    expect(creation.sharedAttempt?.roomId).toBe("created");
  } finally {
    await act(async () => root.unmount());
    host.remove();
    h.catalogs.workerCatalogs.clear();
    vi.unstubAllGlobals();
  }
});
