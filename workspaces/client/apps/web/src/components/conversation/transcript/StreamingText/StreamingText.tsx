import { serviceTimeText } from "../../../../local-time.ts";
import { ownedView } from "../support/ownedView.tsx";
import { PreviewSelection } from "../../support/PreviewSelection.ts";
import { openTranscriptLink } from "../support/transcriptLinks.ts";
import { useState } from "react";
import { useServerNet } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { useItemText } from "../../../../conversation/liveText.ts";
import type { Message } from "../../../../types.ts";
import { reportPromptComposerRender } from "../../../../perf/promptComposerRenderProbe.ts";
import NativeMarkdown from "../../markdown/NativeMarkdown/NativeMarkdown.tsx";
import PreviewModal from "../../PreviewModal/PreviewModal.tsx";
import FilePreview from "../../FilePreview/FilePreview.tsx";

export default ownedView(
  (_props: {
    text?: string;
    message?: Message | undefined;
    streaming?: boolean;
    agentId?: string | undefined;
    basePath?: string | undefined;
  }) => new PreviewSelection(),
  function StreamingText({
    text: fallback = "",
    message,
    streaming = false,
    agentId,
    basePath,
    model,
  }) {
    const net = useServerNet();
    const rawText = useItemText(message, fallback);
    const text = message
      ? serviceTimeText(
          rawText,
          message.at
            ? new Date(
                typeof message.at === "number" ? message.at * 1000 : message.at,
              )
            : undefined,
        )
      : rawText;
    reportPromptComposerRender("message-text", message?.id, text);
    const preview = model.current(net);
    const [linkError, setLinkError] = useState("");
    return (
      <div
        className="prose"
        data-streaming={streaming || undefined}
        onClick={(event) => {
          const link = (event.target as Element).closest("a[href]");
          if (
            !(link instanceof HTMLAnchorElement) ||
            link.hasAttribute("download")
          )
            return;
          const error = openTranscriptLink(
            model,
            net,
            link.getAttribute("href") || "",
            agentId,
            basePath,
          );
          if (error !== undefined) {
            if (error) setLinkError(error);
            event.preventDefault();
            event.stopPropagation();
          }
        }}
      >
        <NativeMarkdown
          text={text}
          streaming={streaming}
          agentId={agentId}
          basePath={basePath}
        />
        {preview && (
          <FilePreview target={preview} onClose={() => model.set(net, null)} />
        )}
        <PreviewModal
          opened={!!linkError}
          onClose={() => setLinkError("")}
          title="Cannot open file"
        >
          <p role="alert">{linkError}</p>
        </PreviewModal>
      </div>
    );
  },
);
