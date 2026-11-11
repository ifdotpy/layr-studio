import { serverLocalStorage } from "./servers/storage.ts";
// @vitest-environment happy-dom
import { action, autorun, spy } from "mobx";
import { expect, it, vi } from "vitest";
import { WorkspaceResource } from "./app/shell/WorkspaceResource.ts";
import { OrchestrationSettingsStore } from "./app/settings/SettingsModels.ts";
import { FederationSettingsStore } from "./components/settings/FederationSettings/FederationSettings.tsx";
import { ClaudeSettingsStore } from "./components/settings/ClaudeSettingsView/ClaudeSettingsStore.ts";
import { ProjectServerRead } from "./components/projects/ProjectServerCards/ProjectServerRead.ts";
import { ClaudeRepairReportStore } from "./components/conversation/ClaudeRepairNotice/ClaudeRepairReportStore.ts";
import { createProjectPeersStore } from "./components/projects/ProjectPeers/ProjectPeersStore.ts";
import { createCostInterest } from "./components/conversation/Usage/Usage.tsx";
import { AttachmentDrafts } from "./components/conversation/support/useAttachmentDrafts.ts";
import { createRecordingPreview } from "./components/conversation/Dictation/RecordingPreview.ts";
import * as defaultApi from "./servers/net/api.ts";
import { UnsentText } from "./unsent/useUnsentText.ts";

vi.mock("./app/projects/newChatSettings.ts", () => ({
  readNewChatResource: () => new Promise<never>(() => {}),
}));

const pending = () => new Promise<never>(() => {});
const api = { get: vi.fn(pending), post: vi.fn(pending) };
const cases: Record<string, () => () => unknown> = {
  WorkspaceResource: () => {
    const model = new WorkspaceResource(pending);
    return () => model.data;
  },
  OrchestrationSettingsStore: () => {
    const model = new OrchestrationSettingsStore(api);
    return () => model.configuration;
  },
  FederationSettingsStore: () => {
    const model = new FederationSettingsStore(api, () => {});
    return () => model.state;
  },
  ClaudeSettingsStore: () => {
    const model = new ClaudeSettingsStore(
      { ...defaultApi, ...api },
      () => undefined,
    );
    return () => model.state;
  },
  ProjectServerRead: () => {
    const model = new ProjectServerRead();
    action(() => {
      model.error = "previous";
    })();
    return () => model.value;
  },
  ClaudeRepairReportStore: () => {
    const model = new ClaudeRepairReportStore(api, () => undefined);
    return () => model.result;
  },
  ProjectPeersStore: () => {
    const model = createProjectPeersStore("project", api, () => {});
    return () => model.grants;
  },
  CostInterest: () => {
    const model = createCostInterest(
      api,
      {
        costs: () => ({ current: { status: "live", data: [] } }),
      },
      "account",
    );
    action(() => {
      model.error = "previous";
    })();
    return () => model.error;
  },
  AttachmentDrafts: () => {
    const model = new AttachmentDrafts(
      () => false,
      () => {},
      {
        key: "drafts",
        storage: serverLocalStorage,
      },
    );
    return () => model.current;
  },
  RecordingPreview: () => {
    const model = createRecordingPreview(
      {
        id: "r",
        chatId: "chat",
        created: 1,
        samples: 16000,
        state: "ready",
        sampleRate: 16000,
      },
      pending,
    );
    action(() => {
      model.url = "blob:previous";
    })();
    return () => model.url;
  },
  UnsentText: () => {
    const model = new UnsentText("server");
    action(() => {
      model.values.set(model.key("reply", "chat"), "draft");
    })();
    return () => model.read("reply", "chat");
  },
};

for (const [name, create] of Object.entries(cases)) {
  it(name + " observation and release never write synchronously", async () => {
    const read = create();
    const writes: string[] = [];
    const errors = vi.spyOn(console, "error");
    const stopSpy = spy((event) => {
      if (
        ["update", "add", "delete", "splice"].includes(event.type) &&
        !(event.type === "update" && event.observableKind === "computed")
      )
        writes.push(event.type);
    });
    let stop = () => {};
    try {
      stop = autorun(read);
      expect(writes).toEqual([]);
      stopSpy();
      await Promise.resolve();
      const stopReleaseSpy = spy((event) => {
        if (
          ["update", "add", "delete", "splice"].includes(event.type) &&
          !(event.type === "update" && event.observableKind === "computed")
        )
          writes.push(event.type);
      });
      stop();
      stopReleaseSpy();
      expect(writes).toEqual([]);
      await Promise.resolve();
      expect(
        errors.mock.calls.filter((args) =>
          args.some((arg) => String(arg).includes("Cannot update a component")),
        ),
      ).toEqual([]);
    } finally {
      stopSpy();
      stop();
      errors.mockRestore();
    }
  });
}
