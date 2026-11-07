// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { createServerServices } from "./serverServices.ts";
import {
  ServerServicesProvider,
  ServerStoreProvider,
  useServerModelReader,
  useServerSidebarServices,
} from "./ServerServicesContext.tsx";
import { observableFixture } from "../components/observableFixture.tsx";

it("retains factories per authenticated owner and never supplies a local model reader to a store-only tree", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const first = createServerServices({
    id: "remote",
    origin: "https://remote.test",
    credentialId: "first",
    label: "Remote",
  });
  const second = createServerServices({
    ...first.owner,
    credentialId: "second",
  });
  const fixture = observableFixture();
  const host = document.createElement("div");
  const root = createRoot(host);
  const seen: unknown[] = [];
  function Reader() {
    seen.push(useServerModelReader());
    return null;
  }
  function Sidebar() {
    seen.push(useServerSidebarServices());
    return null;
  }
  try {
    await act(async () =>
      root.render(
        createElement(ServerServicesProvider, {
          services: first,
          children: createElement(Reader),
        }),
      ),
    );
    expect(seen.at(-1)).toBe(first.modelReader);
    const sidebar = first.sidebarServices;
    expect(first.sidebarServices).toBe(sidebar);
    await act(async () =>
      root.render(
        createElement(ServerServicesProvider, {
          services: first,
          children: createElement(Sidebar),
        }),
      ),
    );
    expect(seen.at(-1)).toBe(sidebar);
    expect(second.sidebarServices).not.toBe(sidebar);
    expect(second.modelReader).not.toBe(first.modelReader);
    expect(second.modelReader.scope).not.toBe(first.modelReader.scope);
    await act(async () =>
      root.render(
        createElement(
          ServerStoreProvider,
          { value: fixture.store },
          createElement(Reader),
        ),
      ),
    );
    expect(seen.at(-1)).toBeUndefined();
  } finally {
    await act(async () => root.unmount());
    first.close();
    second.close();
    fixture.store.dispose();
    vi.unstubAllGlobals();
  }
});
