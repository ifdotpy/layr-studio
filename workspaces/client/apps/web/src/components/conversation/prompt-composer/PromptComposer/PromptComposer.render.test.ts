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
  const listeners = new Set<() => void>();
  const model = observable({ label: "Draft" }, {}, { name: "DraftDisplay" });
  configure({ observableRequiresReaction: true });
  const warnings = vi.spyOn(console, "warn").mockImplementation(() => {});
  let draft = "",
    inputRenders = 0,
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
          getDraft: () => draft,
          subscribeDraft: (_session, notify) => {
            listeners.add(notify);
            return () => {
              listeners.delete(notify);
            };
          },
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
        draft += "x";
        listeners.forEach((notify) => notify());
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
          subscribeDraft: () => () => {},
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
