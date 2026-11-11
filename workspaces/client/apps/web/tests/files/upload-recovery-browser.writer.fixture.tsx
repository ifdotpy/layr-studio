import "../../src/rendererStyles.ts";
import "./attachment-drafts.fixture.ts";
import type { RecoveryHarness } from "./upload-recovery-browser.fixture.tsx";
declare global {
  interface Window {
    uploadRecoveryBrowserRaceMount: typeof mount;
  }
}
window.uploadRecoveryBrowserRaceMount = mount;

import React from "react";
import { observer } from "mobx-react-lite";
import { createRoot } from "react-dom/client";
import { AttachmentDrafts } from "../../src/components/conversation/support/useAttachmentDrafts.ts";
export function mount() {
  const drafts = new AttachmentDrafts(
    () => false,
    () => {},
    { key: "codex-agent-attachments:fixture", storage: localStorage },
  );
  createRoot(document.body).render(
    React.createElement(
      observer(function Writer() {
        const attachments = drafts.current,
          setAttachments = drafts.set;
        // Base writer exposes only attachment state, omitting recovery/form members.
        window.uploadRecoveryBrowserFixture = {
          attachments,
          setAttachments,
        } as RecoveryHarness;
        return null;
      }),
    ),
  );
}
