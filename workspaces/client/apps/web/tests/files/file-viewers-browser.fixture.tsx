import "../../src/rendererStyles.ts";
import { desktopFixture } from "../files/desktop.fixture.ts";
import type { DesktopBridge } from "../../src/desktop.ts";
declare global {
  interface Window {
    parseDelimited: typeof parseDelimited;
    safeSvg: typeof safeSvg;
    actions: Parameters<NonNullable<DesktopBridge["fileAction"]>>[0][];
    saved: { name: string; bytes: number[] };
    show: (path: string, line?: number) => void;
    svgExecuted?: boolean;
  }
}
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import FilePreview, {
  useFilePreview,
} from "../../src/components/conversation/FilePreview/FilePreview.tsx";
import { ServerViewStoreProvider } from "../../src/components/servers/ServerServicesProvider/ServerServicesProvider.tsx";
import StreamingText from "../../src/components/conversation/transcript/StreamingText/StreamingText.tsx";
import {
  parseDelimited,
  safeSvg,
} from "../../src/components/conversation/support/filePreviewFormats.ts";
window.parseDelimited = parseDelimited;
window.safeSvg = safeSvg;
window.actions = [];
window.codexDesktop = {
  ...desktopFixture(),
  fileAction: async (value) => {
    window.actions.push(value);
    return true;
  },
  saveFile: async (value) => {
    window.saved = {
      name: value.name,
      bytes: Array.from(new Uint8Array(value.data)),
    };
    return true;
  },
};
function App() {
  const [target, setTarget] = useFilePreview();
  window.show = (path, line) => setTarget({ agent: "agent-a", path, line });
  return (
    <MantineProvider>
      <StreamingText
        agentId="agent-a"
        text="[Open document](/docs/readme.md)"
      />
      <FilePreview target={target} onClose={() => setTarget(null)} />
    </MantineProvider>
  );
}
createRoot(document.getElementById("root")!).render(
  <ServerViewStoreProvider>
    <App />
  </ServerViewStoreProvider>,
);
