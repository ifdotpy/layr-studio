import type { PreviewSelection } from "../../support/PreviewSelection.ts";
import type { Net } from "../../../../servers/net/net.ts";
import { localFileLink } from "../../support/fileLinks.ts";
import { relativeToDocument } from "../../support/filePreviewFormats.ts";
/** Undefined leaves a normal link alone; an empty result means the preview opened. */
export function openTranscriptLink(
  preview: PreviewSelection,
  net: Net,
  href: string,
  agentId?: string,
  basePath?: string,
) {
  try {
    const target = localFileLink(href);
    if (!target) return;
    if (!agentId)
      throw new Error("This conversation has no workspace for file previews.");
    preview.set(net, {
      agent: agentId,
      ...relativeToDocument(target, basePath),
    });
    return "";
  } catch (error) {
    return error instanceof Error
      ? error.message
      : "Cannot open this file link.";
  }
}
