import { MAX_TURN_ATTACHMENTS } from "../../../generated/sharedValues.ts";
import { useServerNet } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { useServerFileDialogs } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { Button, Loader } from "@mantine/core";
import { File, Paperclip, X } from "lucide-react";
import { useRef } from "react";
import { IconAction } from "../../ui/actions/IconAction/IconAction.tsx";
import FilePreview from "../FilePreview/FilePreview.tsx";
import { PreviewSelection } from "../support/PreviewSelection.ts";
import MarkdownImage from "../MarkdownImage/MarkdownImage.tsx";
import { Owned } from "../../ui/Owned/Owned.tsx";
import { AttachmentSave } from "./AttachmentSave.ts";

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
  const net = useServerNet();
  const input = useRef<HTMLInputElement>(null);
  const reset = () => {
    if (input.current) input.current.value = "";
  };
  return (
    <Owned
      create={() => ({
        preview: new PreviewSelection(),
        save: new AttachmentSave(),
      })}
    >
      {({ preview, save }) => (
        <div className="composer-attachments">
          <FilePreview
            target={preview.current(net)}
            onClose={() => preview.set(net, null)}
          />
          <input
            ref={input}
            type="file"
            multiple
            hidden
            aria-label="Choose attachments"
            onChange={(event) => {
              void save.save(
                Array.from(event.currentTarget.files || []),
                p.add,
                p.notify,
                reset,
              );
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
              p.disabled ||
              p.uploading ||
              p.assets.length >= MAX_TURN_ATTACHMENTS
            }
            onClick={() => {
              if (!canPickFiles()) {
                input.current?.click();
                return;
              }
              void save.save(pickFiles, p.add, p.notify, reset);
            }}
          >
            {p.uploading ? <Loader size={13} /> : <Paperclip size={16} />}
          </IconAction>
          {save.error && (
            <p role="status">
              {save.error}{" "}
              <button type="button" onClick={() => save.retry(p.notify, reset)}>
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
                    onClick={() => preview.set(net, { asset: asset.id })}
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
      )}
    </Owned>
  );
}

export function MessageAttachments(p: {
  assets: readonly Attachment[];
  notify: (message: string) => void;
}) {
  const net = useServerNet();
  if (!p.assets.length) return null;
  return (
    <Owned create={() => new PreviewSelection()}>
      {(preview) => (
        <>
          <FilePreview
            target={preview.current(net)}
            onClose={() => preview.set(net, null)}
          />
          <div className="message-attachments">
            {p.assets.map((asset) => (
              <div
                key={asset.id}
                className={asset.image ? "message-attachment-image" : ""}
              >
                {asset.image && (
                  <MarkdownImage src="" asset={asset.id} alt={asset.name} />
                )}
                <Button
                  variant="default"
                  size="compact-sm"
                  leftSection={<File size={14} />}
                  onClick={() => preview.set(net, { asset: asset.id })}
                >
                  {asset.name}
                </Button>
              </div>
            ))}
          </div>
        </>
      )}
    </Owned>
  );
}
