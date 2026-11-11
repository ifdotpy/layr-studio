import { serviceTimeText } from "../../../../local-time.ts";
import { observer } from "mobx-react-lite";
import { useItemText } from "../../../../conversation/liveText.ts";
import type { Message } from "../../../../types.ts";
import { reportPromptComposerRender } from "../../../../perf/promptComposerRenderProbe.ts";
import NativeMarkdown from "../../markdown/NativeMarkdown/NativeMarkdown.tsx";
import { useState } from "react";
import PreviewModal from "../../PreviewModal/PreviewModal.tsx";
import FilePreview, { useFilePreview } from "../../FilePreview/FilePreview.tsx";
import { localFileLink } from "../../support/fileLinks.ts";
import { relativeToDocument } from "../../support/filePreviewFormats.ts";

export default observer(function StreamingText({
  text: fallback = "",
  message,
  streaming = false,
  agentId,
  basePath,
}: {
  text?: string;
  message?: Message | undefined;
  streaming?: boolean;
  agentId?: string | undefined;
  basePath?: string | undefined;
}) {
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
  const [preview, setPreview] = useFilePreview();
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
        try {
          const target = localFileLink(link.getAttribute("href") || "");
          if (!target) return;
          event.preventDefault();
          // React portal events still reach the outer transcript Markdown.
          event.stopPropagation();
          if (!agentId)
            throw new Error(
              "This conversation has no workspace for file previews.",
            );
          setPreview({
            agent: agentId,
            ...relativeToDocument(target, basePath),
          });
        } catch (error) {
          event.preventDefault();
          event.stopPropagation();
          setLinkError(
            error instanceof Error
              ? error.message
              : "Cannot open this file link.",
          );
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
        <FilePreview target={preview} onClose={() => setPreview(null)} />
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
});
