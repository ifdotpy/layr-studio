import "../../src/rendererStyles.ts";
import "./attachment-drafts.fixture.ts";
type AttachmentState = readonly [
  AttachmentDrafts["current"],
  AttachmentDrafts["set"],
];
type FormState = ReturnType<typeof useFormDraft>;
export type RecoveryHarness = {
  attachments: AttachmentState[0];
  setAttachments: AttachmentState[1];
  scope: string;
  setScope: React.Dispatch<React.SetStateAction<string>>;
  recovery: UploadRecovery;
  profile: FormState[0];
  setProfile: FormState[1];
  rule: FormState[0];
  setRule: FormState[1];
  pending: () => ReturnType<typeof pendingUploads>;
};
declare global {
  interface Window {
    // fixture stores scroll handlers in tests/performance/scroll-restart.fixture.tsx.
    uploadRecoveryBrowserFixture: RecoveryHarness;
    scope: string;
    notices: string[];
    restoreTransactions: () => void;
    cancelError: string;
    releaseFileRead: () => void;
    uploadRecoveryBrowserMount: typeof mount;
    uploadRecoveryBrowserMountWriter: typeof mountWriter;
  }
}
window.uploadRecoveryBrowserMount = mount;
window.uploadRecoveryBrowserMountWriter = mountWriter;

import React from "react";
import { observableFixture } from "../../src/components/shell/integration-support/observableFixture.tsx";
import { makeAutoObservable } from "mobx";
import { observer } from "mobx-react-lite";
import * as uploads from "../../src/sync/uploads.ts";
import { createRoot } from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import "@mantine/core/styles.css";
import ComposerAttachments from "../../src/components/conversation/ComposerAttachments/ComposerAttachments.tsx";
import { AttachmentDrafts } from "../../src/components/conversation/support/useAttachmentDrafts.ts";
import { UploadRecovery } from "../../src/components/conversation/support/useUploadRecovery.ts";
import { useFormDraft } from "../../src/components/shell/Workspace/useFormDraft.ts";
import { queueUploads, pendingUploads } from "../../src/sync/uploads.ts";
export function mount() {
  const models = observableFixture();
  window.notices ||= [];
  const notify = (message: string) => window.notices.push(message);
  const drafts = new AttachmentDrafts(uploads.attachmentCancelled, notify, {
    key: "codex-agent-attachments:fixture",
    storage: localStorage,
  });
  const owner = makeAutoObservable({
    recovery: undefined as UploadRecovery | undefined,
  });
  let stop = () => {};
  const setScope = (scope: React.SetStateAction<string>) => {
    stop();
    window.scope = typeof scope === "function" ? scope(window.scope) : scope;
    owner.recovery = new UploadRecovery(
      uploads,
      "fixture",
      window.scope,
      drafts.set,
    );
    stop = owner.recovery.start();
  };
  setScope(window.scope);
  createRoot(document.body).render(
    React.createElement(
      observer(function Harness() {
        const recovery = owner.recovery!;
        const scope = recovery.workspaceId;
        const attachments = drafts.current,
          setAttachments = drafts.set;
        const [profile, setProfile] = useFormDraft("fixture-profile:" + scope);
        const [rule, setRule] = useFormDraft("fixture-rule:" + scope);
        window.uploadRecoveryBrowserFixture = {
          attachments,
          setAttachments,
          scope,
          setScope,
          recovery,
          profile,
          setProfile,
          rule,
          setRule,
          pending: () => pendingUploads("fixture"),
        };
        return React.createElement(
          models.Provider,
          null,
          React.createElement(
            MantineProvider,
            null,
            React.createElement(ComposerAttachments, {
              notify,
              assets: attachments.lead || [],
              disabled: false,
              uploading: false,
              add: (files) =>
                queueUploads(
                  "fixture",
                  scope === "b".repeat(32) ? "other" : "lead",
                  files,
                  scope,
                ).then(() => {}),
              remove: () => {},
            }),
          ),
        );
      }),
    ),
  );
}
export function mountWriter() {
  const drafts = new AttachmentDrafts(uploads.attachmentCancelled, () => {}, {
    key: "codex-agent-attachments:fixture",
    storage: localStorage,
  });
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
