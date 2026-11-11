import { observer } from "mobx-react-lite";
import { resourceData } from "../../../store/runtime.ts";
import { MAX_TURN_ATTACHMENTS } from "../../../generated/sharedValues.ts";
import { useServerNet } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { useServerFileDialogs } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { Button, Loader } from "@mantine/core";
import { File, Paperclip, X } from "lucide-react";
import { useRef, useState } from "react";
import { errorText } from "../../../api.ts";
import { IconAction } from "../../ui/actions/IconAction/IconAction.tsx";
import FilePreview, { useFilePreview } from "../FilePreview/FilePreview.tsx";

export type { LocalMessageAttachment as Attachment } from "../../../types.ts";
import type { LocalMessageAttachment as Attachment } from "../../../types.ts";
export { attachmentLimit } from "../../../sync/uploads.ts";
export default function ComposerAttachments(p: {
  notify: (message: string) => void;
  assets: readonly Attachment[];
  uploading: boolean;
  disabled: boolean;
  add: (files: globalThis.File[]) => Promise<void>;
  remove: (id: string) => void;
}) {
  const { canPickFiles, pickFiles } = useServerFileDialogs();
  const input = useRef<HTMLInputElement>(null);
  const [preview, setPreview] = useFilePreview();
  const unsavedFiles = useRef<{
    files: globalThis.File[];
    add: typeof p.add;
  } | null>(null);
  const [saveError, setSaveError] = useState("");
  const saveFiles = async (files: globalThis.File[], add = p.add) => {
    if (!files.length) return;
    const pending = { files, add };
    unsavedFiles.current = pending;
    try {
      await add(files);
      if (unsavedFiles.current !== pending) return;
      unsavedFiles.current = null;
      setSaveError("");
      if (input.current) input.current.value = "";
    } catch (error) {
      if (unsavedFiles.current !== pending) return;
      setSaveError(errorText(error));
      p.notify(errorText(error));
    }
  };
  return (
    <div className="composer-attachments">
      <FilePreview target={preview} onClose={() => setPreview(null)} />
      <input
        ref={input}
        type="file"
        multiple
        hidden
        aria-label="Choose attachments"
        onChange={(event) => {
          void saveFiles(Array.from(event.currentTarget.files || []));
        }}
      />
      <IconAction
        type="button"
        size={28}
        variant="subtle"
        className="attach-button"
        label="Attach files"
        title="Attach files"
        disabled={
          p.disabled || p.uploading || p.assets.length >= MAX_TURN_ATTACHMENTS
        }
        onClick={() => {
          if (!canPickFiles()) {
            input.current?.click();
            return;
          }
          void pickFiles()
            .then((files) => saveFiles(files))
            .catch((error) => p.notify(errorText(error)));
        }}
      >
        {p.uploading ? <Loader size={13} /> : <Paperclip size={16} />}
      </IconAction>
      {saveError && (
        <p role="status">
          {saveError}{" "}
          <button
            type="button"
            onClick={() => {
              const pending = unsavedFiles.current;
              if (pending) void saveFiles(pending.files, pending.add);
            }}
          >
            Retry saving files
          </button>
        </p>
      )}
      {!!p.assets.length && (
        <div className="attachment-list">
          {p.assets.map((asset) => (
            <div
              className="attachment-chip"
              key={asset.id}
              title={`${asset.name} · ${Math.ceil(asset.size / 1024)} KiB`}
            >
              <button
                type="button"
                className="attachment-preview-button"
                aria-label={`Preview ${asset.name}`}
                onClick={() => setPreview({ asset: asset.id })}
              >
                {asset.preview ? (
                  <img src={asset.preview} alt="" />
                ) : (
                  <File size={15} />
                )}
                <span>{asset.name}</span>
              </button>
              <IconAction
                type="button"
                size="xs"
                label={`Remove ${asset.name}`}
                onClick={() => p.remove(asset.id)}
              >
                <X size={12} />
              </IconAction>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

const AttachmentImage = observer(function AttachmentImage(p: {
  asset: Attachment;
  onPreview: () => void;
}) {
  const net = useServerNet();

  const slot = net.reads.file({ asset: p.asset.id }).current;
  const file = resourceData(slot);
  const [failed, setFailed] = useState<string>();
  const supported =
    file &&
    ["image/png", "image/jpeg", "image/gif", "image/webp"].includes(file.mime);
  const src = supported ? `data:${file.mime};base64,${file.base64}` : undefined;
  const error =
    failed === p.asset.id
      ? "Cannot load this image."
      : slot.status === "error"
        ? errorText(slot.error)
        : file && !supported
          ? "This image type has no preview."
          : "";
  return error ? (
    <p role="alert">{error}</p>
  ) : src ? (
    <button
      type="button"
      className="image-preview-button"
      aria-label={`Preview ${p.asset.name}`}
      onClick={p.onPreview}
    >
      <img
        src={src}
        alt={p.asset.name}
        loading="lazy"
        onError={() => setFailed(p.asset.id)}
      />
    </button>
  ) : (
    <span role="status">Load image…</span>
  );
});

export function MessageAttachments(p: {
  assets: readonly Attachment[];
  notify: (message: string) => void;
}) {
  const [preview, setPreview] = useFilePreview();
  if (!p.assets.length) return null;
  return (
    <>
      <FilePreview target={preview} onClose={() => setPreview(null)} />
      <div className="message-attachments">
        {p.assets.map((asset) => (
          <div
            key={asset.id}
            className={asset.image ? "message-attachment-image" : ""}
          >
            {asset.image && (
              <AttachmentImage
                asset={asset}
                onPreview={() => setPreview({ asset: asset.id })}
              />
            )}
            <Button
              variant="default"
              size="compact-sm"
              leftSection={<File size={14} />}
              onClick={() => setPreview({ asset: asset.id })}
            >
              {asset.name}
            </Button>
          </div>
        ))}
      </div>
    </>
  );
}
