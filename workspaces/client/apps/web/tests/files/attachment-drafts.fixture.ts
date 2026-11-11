import type { AttachmentDrafts } from "../../src/components/conversation/support/useAttachmentDrafts.ts";
/** The renderer under test wrote this local document with AttachmentDrafts. */
export function readStoredAttachments(
  empty: "{}" | null = "{}",
): AttachmentDrafts["current"] {
  return JSON.parse(
    (localStorage.getItem("codex-agent-attachments:fixture") || empty)!,
  );
}
declare global {
  interface Window {
    uploadRecoveryBrowserReadStoredAttachments: typeof readStoredAttachments;
  }
}
window.uploadRecoveryBrowserReadStoredAttachments = readStoredAttachments;
