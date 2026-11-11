import { useState } from "react";
import { ownedView } from "../support/ownedView.tsx";
import { ConversationResultsStore } from "./ConversationResultsStore.ts";
import { useServerNet } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { PanelHeader } from "../../../ui/PanelHeader/PanelHeader.tsx";
import { Button, Modal } from "@mantine/core";
import {
  ChevronRight,
  FileDiff as FileDiffIcon,
  FileText,
  Image,
  Shapes,
} from "lucide-react";
import type { Message } from "../../../../types.ts";
import FilePreview from "../../FilePreview/FilePreview.tsx";
import RichPreview from "../../RichPreview/RichPreview.tsx";
import FileDiff from "../../FileDiff/FileDiff.tsx";
import { fileChanges, unifiedDiff } from "../../support/fileChangeModel.ts";
import { type ConversationResult } from "./conversationResultModel.ts";

export default ownedView(
  (_props: {
    messages: Message[];
    agentId?: string | undefined;
    onJump?: (messageId: string) => void;
  }) => new ConversationResultsStore(),
  function ConversationResults({ messages, agentId, onJump, model }) {
    const net = useServerNet();
    const { results, selected, file } = model.view(messages, agentId);
    const [open, setOpen] = useState(true);
    const [all, setAll] = useState(false);
    const visible = all ? results : results.slice(0, 3);
    const preview = model.preview.current(net);
    if (!results.length) return null;
    const icon = (result: ConversationResult) =>
      result.kind === "patch" ? (
        <FileDiffIcon size={14} />
      ) : result.kind === "html" || result.kind === "mermaid" ? (
        <Shapes size={14} />
      ) : (result.kind === "file" || result.kind === "asset") &&
        result.image ? (
        <Image size={14} />
      ) : (
        <FileText size={14} />
      );
    return (
      <section
        className="conversation-results"
        aria-label="Results in these messages"
      >
        <PanelHeader
          title={`Results (${results.length})`}
          actions={
            <Button
              variant="subtle"
              className="conversation-results-toggle"
              aria-label="Results"
              aria-expanded={open}
              onClick={() => setOpen(!open)}
              leftSection={
                <ChevronRight size={14} className={open ? "expanded" : ""} />
              }
            >
              {open ? "Hide" : "Show"}
            </Button>
          }
        />
        {open && (
          <div className="conversation-results-list">
            {visible.map((result) => (
              <button
                type="button"
                className="conversation-result"
                key={result.id}
                title={
                  result.kind === "file"
                    ? result.path
                    : result.kind === "patch"
                      ? result.paths.join("\n")
                      : result.label
                }
                disabled={result.kind === "file" && !agentId}
                onClick={() => model.select(result, agentId, net)}
              >
                {icon(result)}
                <span>{result.label}</span>
                {result.kind === "patch" && <small>Patch</small>}
              </button>
            ))}
            {results.length > 3 && (
              <Button
                variant="subtle"
                className="conversation-results-more"
                onClick={() => setAll(!all)}
              >
                {all ? "Show fewer" : `Show ${results.length - 3} more`}
              </Button>
            )}
          </div>
        )}
        {file && (
          <FilePreview target={preview} onClose={() => model.close(net)} />
        )}
        <Modal
          opened={!!selected && !file}
          onClose={() => model.close(net)}
          title={selected?.label}
          size="xl"
          className="conversation-result-modal"
        >
          {selected && !file && (
            <>
              {selected.kind === "patch" ? (
                <>
                  <p className="conversation-results-note">
                    Recorded patch from this message.
                    {selected.truncated ? " The saved content is clipped." : ""}
                  </p>
                  <FileDiff
                    files={
                      selected.change
                        ? fileChanges([selected.change])
                        : unifiedDiff(selected.source, selected.paths[0])
                    }
                    showCounts={!selected.truncated}
                  />
                  <details className="file-change-source">
                    <summary>Original patch</summary>
                    <pre className="file-diff-raw">{selected.source}</pre>
                  </details>
                </>
              ) : (
                (selected.kind === "html" || selected.kind === "mermaid") && (
                  <RichPreview kind={selected.kind} source={selected.source} />
                )
              )}
              {onJump && (
                <Button
                  size="compact-sm"
                  variant="subtle"
                  onClick={() => {
                    onJump(selected.messageId);
                    model.close(net);
                  }}
                >
                  Show message
                </Button>
              )}
            </>
          )}
        </Modal>
      </section>
    );
  },
);
