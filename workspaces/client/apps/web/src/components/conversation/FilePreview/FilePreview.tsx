import { useServerFileActions } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { observer } from "mobx-react-lite";
import { type ReadSlot } from "../../../store/runtime.ts";
import type { GetResult } from "../../../api.ts";
import { Button, Loader } from "@mantine/core";
import { lazy, Suspense, useEffect, useRef, useState } from "react";

import ErrorDescription from "../../shell/ErrorDescription/ErrorDescription.tsx";
import { isolatedDocument } from "../RichPreview/RichPreview.tsx";
import ImageViewer from "../../shell/ImageViewer/ImageViewer.tsx";
import PreviewModal from "../PreviewModal/PreviewModal.tsx";
import type { FilePreviewState } from "../support/FilePreviewState.ts";
import { SourceToggle } from "../SourceToggle/SourceToggle.tsx";
export { openFilePreview } from "../support/PreviewSelection.ts";

// StreamingText opens FilePreview for links. Load it only for Markdown files.
const Markdown = lazy(
  () => import("../transcript/StreamingText/StreamingText.tsx"),
);
export interface PreviewTarget {
  agent?: string | undefined;
  path?: string | undefined;
  asset?: string;
  line?: number | undefined;
}
export type PreviewSource = {
  target: PreviewTarget;
  slot: ReadSlot<GetResult<"/api/file">>;
};
export type FilePreviewSelection = PreviewSource & { state: FilePreviewState };
export default observer(function FilePreview({
  target: selection,
  onClose,
  image,
}: {
  target: FilePreviewSelection | null;
  onClose: () => void;
  image?: { src: string; alt: string; onError: () => void } | undefined;
}) {
  const { canUseNativeFiles, openFile, previewFile, revealFile, saveFile } =
    useServerFileActions();
  const state = selection?.state;
  const target = selection?.target;
  const file = state?.file;
  const [view, setView] = useState<{
    selection: typeof selection;
    source: boolean;
  }>();
  const source = view?.selection === selection ? view.source : !!target?.line;
  const setSource = (source: boolean) => setView({ selection, source });
  const error = state?.error;
  const url = state?.url;
  const excerpt = state?.excerpt;
  const table = state?.table;
  const json = state?.json;
  const selectedLine = useRef<HTMLElement>(null);
  useEffect(() => {
    if (!file || !target?.line || !source) return;
    const frame = requestAnimationFrame(() =>
      selectedLine.current?.scrollIntoView({ block: "center" }),
    );
    return () => cancelAnimationFrame(frame);
  }, [file, target?.line, source]);
  const toggle =
    file && ["markdown", "csv", "tsv", "json"].includes(file.format);
  const Media = file?.format === "audio" ? "audio" : "video";
  return (
    <PreviewModal
      opened={!!target || !!image}
      onClose={onClose}
      title={
        image
          ? image.alt || "Image preview"
          : file?.name || target?.path || "File preview"
      }
      size={
        image
          ? "xl"
          : file &&
              file.text.length < 2000 &&
              ["text", "json"].includes(file.format)
            ? "min(720px, 100vw)"
            : "min(900px, 100vw)"
      }
      overlayProps={{ blur: 0 }}
      className={image ? undefined : "workspace-file-modal"}
    >
      <div
        className="file-preview-toolbar"
        style={image ? { display: "none" } : undefined}
      >
        {file && (
          <>
            <span title={file.mime}>
              {["text", "markdown", "csv", "tsv", "json"].includes(file.format)
                ? "Text file"
                : file.mime}
              {target?.line ? ` · Line ${target.line}` : ""}
            </span>
            <Button
              size="xs"
              variant="subtle"
              onClick={() =>
                state?.act(() =>
                  saveFile({
                    name: file.name,
                    mime: file.mime,
                    data: file.bytes,
                  }),
                )
              }
            >
              Save a copy
            </Button>
          </>
        )}
        {target &&
          canUseNativeFiles() &&
          [
            { label: "Open", operation: openFile },
            { label: "Show in folder", operation: revealFile },
            { label: "Preview", operation: previewFile },
          ].map(({ label, operation }) => (
            <Button
              key={label}
              size="xs"
              variant="subtle"
              onClick={() => state?.act(() => operation(target))}
            >
              {label}
            </Button>
          ))}
      </div>
      {error != null && (
        <p>
          <ErrorDescription value={error} />
        </p>
      )}
      {!file && error == null && target && (
        <Loader size="sm" aria-label="Load file" />
      )}
      {toggle && state && <SourceToggle source={source} onChange={setSource} />}
      {image && <ImageViewer {...image} />}
      {file &&
        (source || file.format === "text" ? (
          <pre className="file-preview-body file-preview-source">
            {excerpt ? (
              <>
                {excerpt.before}
                <mark ref={selectedLine} data-file-line={target?.line}>
                  {excerpt.line || " "}
                </mark>
                {excerpt.after}
              </>
            ) : (
              file.text
            )}
          </pre>
        ) : (file.format === "image" || file.format === "svg") && url ? (
          <ImageViewer
            src={url}
            alt={file.name}
            onError={() => state?.fail("Cannot load this image.")}
          />
        ) : file.format === "markdown" ? (
          <div className="file-preview-body">
            <Suspense fallback={<Loader size="sm" />}>
              <Markdown
                text={file.text}
                agentId={target?.agent}
                basePath={target?.asset ? undefined : target?.path}
              />
            </Suspense>
          </div>
        ) : table ? (
          <>
            {table.error && <p role="alert">{table.error}</p>}
            {table.truncated && (
              <p role="status">Shows up to 200 rows and 100 columns.</p>
            )}
            <div className="file-preview-table">
              <table aria-label="File table">
                <tbody>
                  {table.rows.map((row, i) => (
                    <tr key={i}>
                      <th scope="row">{i + 1}</th>
                      {row.map((cell, j) => (
                        <td key={j}>{cell}</td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </>
        ) : json ? (
          <>
            {json.error && <p role="alert">{json.error}</p>}
            <pre className="file-preview-body file-preview-source">
              {json.text}
            </pre>
          </>
        ) : file.format === "html" ? (
          <iframe
            title={file.name}
            className="workspace-preview-frame"
            sandbox=""
            referrerPolicy="no-referrer"
            srcDoc={isolatedDocument(file.text)}
          />
        ) : file.format === "pdf" && url ? (
          <iframe
            title={file.name}
            className="workspace-preview-frame"
            src={url}
          />
        ) : ["audio", "video"].includes(file.format) && url ? (
          <Media
            onError={() =>
              state?.fail(`Cannot play this ${file.format} format.`)
            }
            className="file-preview-media"
            controls
            playsInline={file.format === "video"}
            preload="metadata"
            src={url}
          />
        ) : (
          <p>No preview for this file type.</p>
        ))}
    </PreviewModal>
  );
});
