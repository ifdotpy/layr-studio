import "../../src/rendererStyles.ts";
import { fixtureElement } from "./browser-helpers.ts";
const mount1 = async () => {
  window.snapshotMountedAt = performance.now();
  const { useStartupFixture } = await import("./startup-fixture.ts");
  const React = await import("react");
  const { createRoot } = await import("react-dom/client");
  const root = createRoot(fixtureElement("root"));
  window.unmountSnapshot = () => root.unmount();
  root.render(
    React.createElement(function SnapshotHarness() {
      window.snapshot = useStartupFixture();
      return React.createElement(
        "p",
        {
          role: window.snapshot.startup.error ? "alert" : "status",
        },
        window.snapshot.startup.error || "Connecting to LAYR Studio…",
      );
    }),
  );
};
window.entityFirstLoadRetryBrowserMount1 = mount1;
declare global {
  interface Window {
    entityFirstLoadRetryBrowserMount1: typeof mount1;
  }
}

declare global {
  interface Window {
    snapshotMountedAt: number;
    unmountSnapshot: () => void;
  }
}
