import "../../src/rendererStyles.ts";
import React from "react";
import { createRoot } from "react-dom/client";
import { useStartupFixture } from "../sync/startup-fixture.ts";

declare global {
  interface Window {
    snapshot: ReturnType<typeof useStartupFixture>;
  }
}

const node = document.createElement("div");
document.body.appendChild(node);
function Harness() {
  window.snapshot = useStartupFixture();
  return null;
}
createRoot(node).render(React.createElement(Harness));
