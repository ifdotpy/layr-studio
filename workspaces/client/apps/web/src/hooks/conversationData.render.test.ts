import { resourceData } from "../store/runtime.ts";
import { useServerStore } from "../components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { observable, runInAction } from "mobx";
import { observer } from "mobx-react-lite";
import { expect, it, vi } from "vitest";
import { observableFixture } from "../components/shell/integration-support/observableFixture.tsx";
import { ServerStoreProvider } from "../components/servers/ServerServicesProvider/ServerServicesProvider.tsx";

it("waits for the generated state slot before reading conversation models", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const fixture = observableFixture();
  fixture.apply([
    { collection: "workspace", id: "workspace", value: { stateDir: "owner" } },
  ]);
  type Current = ReturnType<typeof fixture.store.state>["current"];
  const current = observable.box<Current>(
    { status: "loading" },
    { deep: false },
  );
  const store = {
    ...fixture.store,
    state: () => ({
      get current() {
        return current.get();
      },
    }),
  };
  const Caller = observer(function Caller() {
    const store = useServerStore();
    const data = resourceData(store.state().current);
    return createElement(
      "output",
      null,
      data ? data.workspaces.get("workspace")?.stateDir : "waiting",
    );
  });
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  try {
    await act(async () =>
      root.render(
        createElement(
          ServerStoreProvider,
          { value: store },
          createElement(Caller),
        ),
      ),
    );
    expect(host.textContent).toBe("waiting");
    await act(async () =>
      runInAction(() =>
        current.set({ status: "live", data: fixture.mirror.current }),
      ),
    );
    expect(host.textContent).toBe("owner");
  } finally {
    await act(async () => root.unmount());
    fixture.store.dispose();
    host.remove();
    vi.unstubAllGlobals();
  }
});
