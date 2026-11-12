import "../../src/rendererStyles.ts";
import { AgentModelsFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import type { ComponentProps } from "react";
import type { Attachment } from "../../src/components/conversation/ComposerAttachments/ComposerAttachments.tsx";
import type { Message } from "../../src/types.ts";
declare global {
  interface Window {
    setText: (value: string) => void;
    setCount: (value: number) => void;
    setTheme: (value: "light" | "dark") => void;
    retained: Element;
  }
}

import { useState } from "react";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import StreamingText from "../../src/components/conversation/transcript/StreamingText/StreamingText.tsx";
import Activity, {
  ToolCard,
} from "../../src/components/conversation/transcript/Activity/Activity.tsx";
import codexProgressFixtures from "../../../../../integrations/providers/codex/tests/fixtures/native-progress.json";
import claudeProgressFixtures from "../../../../../integrations/providers/claude/tests/fixtures/native-progress.json";
const progressFixtures = [...codexProgressFixtures, ...claudeProgressFixtures];
import History from "../../src/components/conversation/transcript/TurnHistory/TurnHistory.fixture.tsx";
function TurnHistory(props: ComponentProps<typeof History>) {
  return (
    <AgentModelsFixture agents={props.agent ? [props.agent] : []}>
      <History {...props} />
    </AgentModelsFixture>
  );
}
import ComposerAttachments, {
  MessageAttachments,
} from "../../src/components/conversation/ComposerAttachments/ComposerAttachments.tsx";
const asset = {
  id: "fixture-image",
  name: "pixel.png",
  mime: "image/png",
  image: true,
  size: 68,
  preview:
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aN1sAAAAASUVORK5CYII=",
} satisfies Attachment;
function Harness() {
  const [text, setText] = useState("First sentence. Partial");
  const [count, setCount] = useState(2);
  const [streaming, setStreaming] = useState(true);
  const [theme, setTheme] = useState<"light" | "dark">("light");
  Object.assign(window, { setText, setCount, setStreaming, setTheme });
  const items: Message[] = Array.from({ length: count }, (_, i) => ({
    id: "tool" + i,
    turnId: "turn",
    role: "tool",
    text: JSON.stringify({
      type: "commandExecution",
      command: "pwd",
      aggregatedOutput: "result",
    }),
    createdAt: "2026-09-10",
  }));
  // The preview fixture's no-op add callback omits its Promise result.
  type AttachmentProps = ComponentProps<typeof ComposerAttachments>;
  type PreviewAttachmentProps = Omit<AttachmentProps, "add"> & {
    add: (
      ...files: Parameters<AttachmentProps["add"]>
    ) => ReturnType<AttachmentProps["add"]> | void;
  };
  const attachmentProps: PreviewAttachmentProps = {
    notify: () => {},
    assets: [asset],
    uploading: false,
    disabled: false,
    add: () => {},
    remove: () => {},
  };
  return (
    <MantineProvider forceColorScheme={theme}>
      <main>
        <section id="native-progress">
          {progressFixtures.map((fixture) => (
            <section key={fixture.provider}>
              <h2>
                {fixture.provider === "codex" ? "Codex" : "Claude"} progress
              </h2>
              {fixture.rows.map((row) =>
                row.payload ? (
                  <ToolCard
                    key={row.id}
                    item={{
                      id: row.id,
                      role: "output",
                      text: JSON.stringify(row.payload),
                      toolStatus: "running",
                    }}
                  />
                ) : (
                  <StreamingText key={row.id} text={row.text} streaming />
                ),
              )}
            </section>
          ))}
        </section>
        <div id="text">
          <StreamingText text={text} streaming={streaming} agentId="fixture" />
        </div>
        <div id="activity">
          <Activity items={items} />
        </div>
        <div id="turn">
          <TurnHistory
            items={items}
            currentTurn="turn"
            enabled
            storageKey="test-rendering"
            renderMessage={() => null}
            onJump={() => {}}
          />
        </div>
        <ComposerAttachments {...(attachmentProps as AttachmentProps)} />
        <MessageAttachments assets={[asset]} notify={() => {}} />
      </main>
    </MantineProvider>
  );
}
const root = document.getElementById("root")!;
createRoot(root).render(
  <AgentModelsFixture agents={[]}>
    <Harness />
  </AgentModelsFixture>,
);
