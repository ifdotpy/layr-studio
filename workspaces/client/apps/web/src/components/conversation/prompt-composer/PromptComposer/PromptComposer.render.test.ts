// @vitest-environment happy-dom
import { configure, observable, runInAction } from "mobx";
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import PromptComposer from "./PromptComposer.tsx";

const counts = vi.hoisted(() => ({ form: 0 }));
vi.mock("./PromptComposer.tsx", async (original) => {
  const actual = await original<typeof import("./PromptComposer.tsx")>();
  return {
    ...actual,
    default: (props: Parameters<typeof actual.default>[0]) => {
      counts.form++;
      return actual.default(props);
    },
  };
});

it("50 draft changes render the input leaf without the form or static settings", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const drafts = observable.map<string, string>();
  const model = observable({ label: "Draft" }, {}, { name: "DraftDisplay" });
  configure({ observableRequiresReaction: true });
  const warnings = vi.spyOn(console, "warn").mockImplementation(() => {});
  let inputRenders = 0,
    settingsRenders = 0;
  function Settings() {
    settingsRenders++;
    return createElement("button", { type: "button" }, "Role settings");
  }
  const settings = createElement(Settings);
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(PromptComposer, {
          session: "chat",
          getDraft: () => drafts.get("chat") ?? "",
          onSubmit: (event) => event.preventDefault(),
          children: (value) => {
            inputRenders++;
            return createElement(
              "div",
              null,
              createElement("input", {
                value,
                readOnly: true,
                "aria-label": model.label,
              }),
              settings,
            );
          },
        }),
      ),
    );
    counts.form = inputRenders = settingsRenders = 0;
    warnings.mockClear();
    for (let index = 0; index < 50; index++)
      await act(async () => {
        runInAction(() => drafts.set("chat", "x".repeat(index + 1)));
      });
    expect(host.querySelector("input")?.value).toBe("x".repeat(50));
    console.log("50 drafts", {
      form: counts.form,
      input: inputRenders,
      settings: settingsRenders,
    });
    const draftWarnings = warnings.mock.calls.filter(([text]) =>
      String(text).includes("DraftDisplay.label"),
    );
    console.log("50 draft observable read warnings", draftWarnings.length);
    expect(draftWarnings).toHaveLength(0);
    expect(inputRenders).toBe(50);
    expect(counts.form).toBe(0);
    expect(settingsRenders).toBe(0);
  } finally {
    await act(async () => root.unmount());
    host.remove();
    warnings.mockRestore();
    configure({ observableRequiresReaction: false });
    vi.unstubAllGlobals();
  }
});

it("tracks observable render-prop reads in the draft leaf without waking the form", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const model = observable({ label: "Before" });
  let renders = 0;
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(PromptComposer, {
          session: "chat",
          getDraft: () => "draft",
          onSubmit: (event) => event.preventDefault(),
          children: (draft) => {
            renders++;
            return createElement("span", null, model.label + draft);
          },
        }),
      ),
    );
    counts.form = renders = 0;
    await act(async () =>
      runInAction(() => {
        model.label = "After";
      }),
    );
    console.log("observable draft metadata", {
      leaf: renders,
      form: counts.form,
    });
    expect(host.textContent).toBe("Afterdraft");
    expect(renders).toBe(1);
    expect(counts.form).toBe(0);
  } finally {
    await act(async () => root.unmount());
    host.remove();
    vi.unstubAllGlobals();
  }
});

it("renders restored and programmatic drafts from the real draft owner", async () => {
  const { createDrafts } = await import("../../../../sync/drafts.ts");
  type Version =
    import("../../../../sync/subscriptions/contracts.ts").ResourceVersion;
  let receive: ((version?: Version) => void) | undefined;
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.useFakeTimers();
  const drafts = createDrafts({
    serverId: "composer-real-owner",
    api: { syncPost: async () => [] },
    resources: {
      watchResourceChanges: (_ref, listener) => {
        receive = listener;
        return () => {};
      },
      replaceStream: vi.fn(),
    },
  });
  const stop = drafts.start();
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(PromptComposer, {
          session: "chat",
          getDraft: drafts.getDraft,
          onSubmit: (event) => event.preventDefault(),
          children: (value) =>
            createElement("textarea", { value, readOnly: true }),
        }),
      ),
    );
    await act(async () =>
      receive?.({
        epoch: "composer",
        revision: 1,
        reason: "initial",
        draftSnapshot: {
          complete: true,
          documents: [
            {
              id: "remote:chat",
              seq: 1,
              _deleted: false,
              payload: JSON.stringify({
                id: "remote:chat",
                device: "remote",
                session: "chat",
                text: "Restored draft",
                updated: 1,
              }),
            },
          ],
        },
      }),
    );
    expect(drafts.getDraft("chat")).toBe("Restored draft");
    expect(host.querySelector("textarea")?.value).toBe("Restored draft");
    await act(async () => drafts.setDrafts({ chat: "Programmatic draft" }));
    expect(host.querySelector("textarea")?.value).toBe("Programmatic draft");
  } finally {
    await act(async () => root.unmount());
    stop();
    host.remove();
    vi.useRealTimers();
    vi.unstubAllGlobals();
  }
});
