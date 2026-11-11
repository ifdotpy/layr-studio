import { observer } from "mobx-react-lite";
import { resourceData } from "../../../store/runtime.ts";
import { useServerNet } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { useMemo, useState } from "react";
import PreviewModal from "../PreviewModal/PreviewModal.tsx";
import { errorText } from "../../../api.ts";
import FilePreview, {
  useFilePreview,
  type PreviewTarget,
} from "../FilePreview/FilePreview.tsx";
import ImageViewer from "../../shell/ImageViewer/ImageViewer.tsx";
import { relativeToDocument, safeSvg } from "../support/filePreviewFormats.ts";
import { localFileLink } from "../support/fileLinks.ts";

const raster = /^data:image\/(png|jpeg|gif|webp);base64,/i;
const MarkdownImageContent = observer(function MarkdownImage({
  src,
  alt,
  agentId,
  basePath,
  localPath,
}: {
  src: string;
  alt: string;
  agentId?: string | undefined;
  basePath?: string | undefined;
  localPath?: string | undefined;
}) {
  const net = useServerNet();

  const [error, setError] = useState("");
  const [remoteAllowed, setRemoteAllowed] = useState(false);
  const [preview, setPreview] = useState(false);
  const [filePreview, openFilePreview] = useFilePreview();
  const target = useMemo((): PreviewTarget | null => {
    try {
      const local =
        localPath !== undefined ? { path: localPath } : localFileLink(src);
      return local && agentId
        ? {
            agent: agentId,
            ...(localPath !== undefined
              ? local
              : relativeToDocument(local, basePath)),
          }
        : null;
    } catch {
      return null;
    }
  }, [src, agentId, basePath, localPath]);
  const remoteHost = useMemo(() => {
    try {
      const url = new URL(src);
      return /^https?:$/.test(url.protocol) && !url.username && !url.password
        ? url.hostname
        : "";
    } catch {
      return "";
    }
  }, [src]);
  const remote = !!remoteHost;
  const read =
    !raster.test(src) && target
      ? net.reads.file({ agent: target.agent!, path: target.path! })
      : undefined;
  const current = read?.current;
  const file = current && resourceData(current);
  let loaded = raster.test(src) ? src : "";
  let loadError = current?.status === "error" ? current.error.message : "";
  try {
    if (file) {
      if (file.mime === "image/svg+xml") {
        const svg = safeSvg(
          new TextDecoder().decode(
            Uint8Array.from(atob(file.base64), (c) => c.charCodeAt(0)),
          ),
        );
        loaded = "data:image/svg+xml;charset=utf-8," + encodeURIComponent(svg);
      } else {
        if (!/^image\/(png|jpeg|gif|webp|avif|bmp|x-icon)$/.test(file.mime))
          throw new Error("This image type is not supported.");
        loaded = `data:${file.mime};base64,${file.base64}`;
      }
    } else if (!target && !remote && !loaded) {
      loadError = "This image cannot be opened here.";
    }
  } catch (cause) {
    loadError = errorText(cause);
  }
  const imageError = error || loadError;
  const image = loaded || (remoteAllowed ? src : "");
  return (
    <span className="markdown-image">
      {image && !imageError ? (
        <button
          type="button"
          className="image-preview-button"
          aria-label={`Preview ${alt || "image"}`}
          onClick={(event) => {
            event.preventDefault();
            event.stopPropagation();
            setPreview(true);
            if (target) openFilePreview(target);
          }}
        >
          <img
            src={image}
            alt={alt}
            loading="lazy"
            referrerPolicy="no-referrer"
            onError={() => setError("Cannot load this image.")}
          />
        </button>
      ) : (
        <span className="markdown-image-placeholder">
          <span>{alt || "Image"}</span>
          {imageError ? (
            <>
              <span role="alert">{imageError}</span>
              <button
                type="button"
                onClick={() => {
                  setError("");
                  if (remote) setRemoteAllowed(true);
                  else void read?.read().catch(() => {});
                }}
              >
                Retry image
              </button>
            </>
          ) : remote ? (
            <>
              <span>External image. Load it to contact {remoteHost}.</span>
              <button type="button" onClick={() => setRemoteAllowed(true)}>
                Load image
              </button>
            </>
          ) : (
            <span role="status">Load image…</span>
          )}
        </span>
      )}
      {target ? (
        <FilePreview
          target={preview ? filePreview : null}
          onClose={() => setPreview(false)}
        />
      ) : (
        <PreviewModal
          opened={preview}
          onClose={() => setPreview(false)}
          title={alt || "Image preview"}
          size="xl"
        >
          {preview && (
            <ImageViewer
              src={image}
              alt={alt}
              onError={() => setError("Cannot load this image.")}
            />
          )}
        </PreviewModal>
      )}
    </span>
  );
});

export default function MarkdownImage(
  props: Parameters<typeof MarkdownImageContent>[0],
) {
  return (
    <MarkdownImageContent
      key={JSON.stringify([
        props.src,
        props.agentId,
        props.basePath,
        props.localPath,
      ])}
      {...props}
    />
  );
}
