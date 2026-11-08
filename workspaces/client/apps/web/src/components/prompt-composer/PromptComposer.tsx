import { observer } from "mobx-react-lite";
import {
  useSyncExternalStore,
  type DragEventHandler,
  type FormEvent,
  type ReactNode,
} from "react";
import { reportPromptComposerRender } from "./renderProbe";

export type DraftSubscription = (
  session: string,
  listener: () => void,
) => () => void;
export type DraftReader = (session: string) => string;
export type DraftWriter = (
  text: string | ((current: string) => string),
  session?: string,
) => void;

export function usePromptDraft(
  session: string,
  getDraft: DraftReader,
  subscribeDraft: DraftSubscription,
) {
  return useSyncExternalStore(
    (notify) => subscribeDraft(session, notify),
    () => getDraft(session),
    () => getDraft(session),
  );
}

type DraftSource = {
  session: string;
  getDraft: DraftReader;
  subscribeDraft: DraftSubscription;
};

export const PromptDraft = observer(function PromptDraft(
  p: DraftSource & { children: (draft: string) => ReactNode },
) {
  const draft = usePromptDraft(p.session, p.getDraft, p.subscribeDraft);
  reportPromptComposerRender("prompt-composer", p.session);
  return p.children(draft);
});

/** Draft changes execute the content leaf; the form and prebuilt settings stay idle. */
export default function PromptComposer(
  p: DraftSource & {
    onSubmit: (event: FormEvent<HTMLFormElement>) => void;
    onDragOver?: DragEventHandler<HTMLFormElement>;
    onDragLeave?: DragEventHandler<HTMLFormElement>;
    onDrop?: DragEventHandler<HTMLFormElement>;
    className?: string;
    children: (draft: string) => ReactNode;
  },
) {
  return (
    <form
      id="composer"
      className={p.className}
      onSubmit={p.onSubmit}
      onDragOver={p.onDragOver}
      onDragLeave={p.onDragLeave}
      onDrop={p.onDrop}
    >
      <PromptDraft {...p} />
    </form>
  );
}
