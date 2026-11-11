// @vitest-environment happy-dom
import { act, createElement as h, createRef } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { expect, it, vi } from "vitest";
import PromptInput from "./PromptInput.tsx";
import { SkillAutocomplete } from "../../support/SkillAutocomplete.ts";
import { ServerStoreProvider } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { observableFixture } from "../../../shell/integration-support/observableFixture.tsx";
import { requestTestEnvironment } from "../../questions/Requests/requestTestFixture.ts";

requestTestEnvironment();
it("uses the live injected autocomplete when a mounted session owner is replaced", async () => {
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      disconnect() {}
    },
  );
  const models = observableFixture();
  const first = new SkillAutocomplete(),
    second = new SkillAutocomplete();
  const host = document.createElement("div"),
    root = createRoot(host);
  document.body.append(host);
  const props = {
    value: "/r",
    session: "same",
    getDraft: () => "/r",
    setDraft: vi.fn(),
    input: createRef<HTMLTextAreaElement>(),
    mobile: false,
    managed: true,
    canSend: true,
    blocked: false,
    sending: false,
    uploading: false,
    draftTooLong: false,
    modelCommand: false,
    hasAttachments: false,
    onChange: vi.fn(),
    onPasteFiles: vi.fn(),
    onSend: vi.fn(),
    onQueue: vi.fn(),
    onRecallKeyDown: () => false,
    skillCatalog: {
      enabled: true,
      agentId: "agent",
      workspace: "workspace",
      account: "default",
      cwd: "/project",
      provider: "codex",
    },
  };
  const render = (autocomplete: SkillAutocomplete) =>
    root.render(
      h(
        MantineProvider,
        { env: "test" },
        h(
          ServerStoreProvider,
          { value: models.store },
          h(PromptInput, { ...props, autocomplete }),
        ),
      ),
    );
  try {
    await act(async () => render(first));
    await act(async () => render(second));
    await act(async () =>
      props.input.current!.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
      ),
    );
    expect(first.dismissedCommand).toBeNull();
    expect(second.dismissedCommand).toBe("same:/r");
  } finally {
    await act(async () => root.unmount());
    models.store.dispose();
    host.remove();
  }
});
