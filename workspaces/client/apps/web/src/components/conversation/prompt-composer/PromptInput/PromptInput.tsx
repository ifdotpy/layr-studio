import { observer } from "mobx-react-lite";
import { Textarea, type TextareaProps } from "@mantine/core";
import {
  memo,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  type KeyboardEvent,
  type RefObject,
} from "react";
import ComposerAutocomplete from "../../ComposerAutocomplete/ComposerAutocomplete.tsx";
import { SkillAutocomplete } from "../../support/SkillAutocomplete.ts";
import { Owned } from "../../../ui/Owned/Owned.tsx";
import { useServerNet } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import type { DraftReader } from "../PromptComposer/PromptComposer.tsx";
import type { DraftWriter } from "../PromptComposer/PromptComposer.tsx";

const INPUT_ROWS = 1;

type InputHandlers = Pick<
  TextareaProps,
  | "onPaste"
  | "onChange"
  | "onClick"
  | "onBlur"
  | "onKeyUp"
  | "onSelect"
  | "onKeyDown"
  | "onCompositionStart"
  | "onCompositionEnd"
>;

// Autosize measures layout on every render of the textarea, which is expensive
// beside hundreds of team rows. Rendering it only for its own visible inputs
// keeps unrelated snapshot renders out of the layout engine; handlers always
// run the latest closure through one stable proxy.
const AutosizeInput = memo(function AutosizeInput({
  handlers,
  ...props
}: Omit<TextareaProps, Exclude<keyof InputHandlers, "onKeyDown">> & {
  handlers: { current: InputHandlers };
}) {
  return (
    <Textarea
      {...props}
      variant="unstyled"
      rows={INPUT_ROWS}
      onPaste={(event) => handlers.current.onPaste?.(event)}
      onChange={(event) => handlers.current.onChange?.(event)}
      onClick={(event) => handlers.current.onClick?.(event)}
      onBlur={(event) => handlers.current.onBlur?.(event)}
      onKeyUp={(event) => handlers.current.onKeyUp?.(event)}
      onSelect={(event) => handlers.current.onSelect?.(event)}
      onCompositionStart={(event) =>
        handlers.current.onCompositionStart?.(event)
      }
      onCompositionEnd={(event) => handlers.current.onCompositionEnd?.(event)}
    />
  );
});

type PromptInputProps = {
  autocomplete?: SkillAutocomplete;
  value: string;
  draftResetVersion?: number;
  session: string;
  getDraft: DraftReader;
  setDraft: DraftWriter;
  input: RefObject<HTMLTextAreaElement | null>;
  mobile: boolean;
  managed: boolean;
  canSend: boolean;
  blocked: boolean;
  sending: boolean;
  uploading: boolean;
  draftTooLong: boolean;
  modelCommand: boolean;
  hasAttachments: boolean;
  onChange: (value: string) => void;
  onPasteFiles: (files: File[]) => void;
  onSend: () => void;
  onQueue: () => void;
  onRecallKeyDown: (event: KeyboardEvent<HTMLTextAreaElement>) => boolean;
  skillCatalog: {
    enabled: boolean;
    agentId: string;
    workspace: string;
    account: string;
    cwd: string;
    provider: string;
  };
};
export default function PromptInput(p: PromptInputProps) {
  if (p.autocomplete)
    return <PromptInputView {...p} autocomplete={p.autocomplete} />;
  return (
    <Owned key={p.session} create={() => new SkillAutocomplete()}>
      {(state) => <PromptInputView {...p} autocomplete={state} />}
    </Owned>
  );
}
const PromptInputView = observer(function PromptInputView(
  p: PromptInputProps & { autocomplete: SkillAutocomplete },
) {
  const composing = useRef(false);
  const nativeEditVersion = useRef(0);
  const observedResetVersion = useRef(p.draftResetVersion);
  const pendingExternal = useRef<
    { value: string; nativeVersion: number; reset: boolean } | undefined
  >(undefined);
  const resizeFrame = useRef(0);
  const resize = useCallback(() => {
    if (resizeFrame.current) return;
    resizeFrame.current = requestAnimationFrame(() => {
      resizeFrame.current = 0;
      const element = p.input.current;
      if (!element) return;
      element.style.height = "auto";
      element.style.height = `${element.scrollHeight}px`;
    });
  }, [p.input]);
  useLayoutEffect(() => {
    const element = p.input.current;
    // React's controlled textarea also rewrites defaultValue (its child text),
    // disturbing the native IME. Only external draft changes need a value write.
    const reset = p.draftResetVersion !== observedResetVersion.current;
    observedResetVersion.current = p.draftResetVersion;
    if (element) {
      if (composing.current)
        pendingExternal.current =
          element.value === p.value && !reset
            ? undefined
            : {
                value: p.value,
                nativeVersion: nativeEditVersion.current,
                reset,
              };
      else if (element.value !== p.value) element.value = p.value;
    }
    resize();
  }, [p.value, p.draftResetVersion, p.input, resize]);
  useEffect(() => {
    const element = p.input.current;
    if (!element) return;
    let width: number | undefined;
    const observer = new ResizeObserver(([entry]) => {
      if (entry && entry.contentRect.width !== width) {
        width = entry.contentRect.width;
        resize();
      }
    });
    observer.observe(element);
    return () => {
      observer.disconnect();
      cancelAnimationFrame(resizeFrame.current);
      resizeFrame.current = 0;
    };
  }, [p.input, resize]);
  const insertSkill = useCallback(
    (text: string, start: number, end: number) => {
      const current = p.input.current?.value ?? p.getDraft(p.session);
      p.setDraft(
        current.slice(0, start) + text + current.slice(end),
        p.session,
      );
      requestAnimationFrame(() => {
        const element = p.input.current;
        if (!element || composing.current) return;
        const caret = start + text.length;
        element.focus();
        element.setSelectionRange(caret, caret);
      });
    },
    [p.getDraft, p.input, p.session, p.setDraft],
  );
  const net = useServerNet();
  const { enabled, agentId, ...scope } = p.skillCatalog;
  const read = net.reads.skills({ agent: agentId }, scope);
  const skills = p.autocomplete;
  const range = skills.rangeFor(read);
  const updateRange = () => skills.updateRange(enabled, read, p.input.current);
  const matches = range ? skills.matches : [];
  const commandQuery =
    p.managed && /^\/[a-z-]*$/i.test(p.value)
      ? p.value.slice(1).toLowerCase()
      : null;
  const handlers = useRef<InputHandlers>({});
  const keyDown = useCallback<NonNullable<InputHandlers["onKeyDown"]>>(
    (event) => handlers.current.onKeyDown?.(event),
    [],
  );
  handlers.current = {
    onPaste: (event) => {
      const files = Array.from(event.clipboardData.files);
      if (p.managed && files.length) {
        event.preventDefault();
        p.onPasteFiles(files);
      }
    },
    onChange: (event) => {
      const version = ++nativeEditVersion.current;
      const pending = pendingExternal.current;
      if (composing.current && pending?.reset) return;
      if (pending && version > pending.nativeVersion)
        pendingExternal.current = undefined;
      p.onChange(event.currentTarget.value);
    },
    onCompositionStart: () => {
      composing.current = true;
    },
    onCompositionEnd: (event) => {
      composing.current = false;
      const pending = pendingExternal.current;
      pendingExternal.current = undefined;
      // Only a value received after the latest native edit is still pending.
      // A send reset is explicit and must not resurrect the captured message.
      if (pending !== undefined) {
        if (event.currentTarget.value !== pending.value)
          event.currentTarget.value = pending.value;
        resize();
      } else p.onChange(event.currentTarget.value);
    },
    onClick: updateRange,
    onBlur: skills.blur,
    onKeyUp: updateRange,
    onSelect: updateRange,
    onKeyDown: (event) => {
      if (
        event.key === "Tab" &&
        p.managed &&
        p.canSend &&
        !p.modelCommand &&
        !event.shiftKey &&
        !event.altKey &&
        !event.ctrlKey &&
        !event.metaKey &&
        !event.repeat &&
        !event.nativeEvent.isComposing &&
        !p.sending &&
        !p.uploading &&
        !p.draftTooLong &&
        (p.value.trim() || p.hasAttachments)
      ) {
        event.preventDefault();
        p.onQueue();
        return;
      }
      if (p.onRecallKeyDown(event)) return;
      if (
        !p.mobile &&
        event.key === "Enter" &&
        !event.shiftKey &&
        !event.nativeEvent.isComposing
      ) {
        event.preventDefault();
        p.onSend();
      }
    },
  };
  return (
    <ComposerAutocomplete
      id={commandQuery !== null ? "command-suggestions" : "skill-suggestions"}
      loadingMessage="Loading skills…"
      emptyMessage="No matching skills"
      label={commandQuery !== null ? "Commands" : "Skills"}
      opened={
        !p.modelCommand &&
        ((commandQuery !== null &&
          commandQuery !== "rename" &&
          skills.dismissedCommand !== `${p.session}:${p.value}`) ||
          !!range)
      }
      resetKey={commandQuery ?? range?.signature}
      options={
        commandQuery !== null
          ? "rename".includes(commandQuery)
            ? [
                {
                  value: "rename",
                  label: "/rename",
                  description:
                    "Name this chat from its conversation, or add a name.",
                },
              ]
            : []
          : matches.map((skill) => ({
              value: skill.name,
              label: skill.name,
              description: skill.description,
            }))
      }
      loading={
        commandQuery === null && !!range && read.current.status === "loading"
      }
      error={
        commandQuery === null && range && read.current.status === "error"
          ? "Could not load skills"
          : undefined
      }
      warning={
        commandQuery === null && !!skills.catalog?.errors.length
          ? "Some skills could not be loaded"
          : undefined
      }
      onDismiss={() => {
        if (commandQuery !== null)
          skills.dismissCommand(`${p.session}:${p.value}`);
        else skills.dismiss();
      }}
      onSelect={(name) => {
        if (commandQuery !== null) {
          if (name === "rename") insertSkill("/rename ", 0, p.value.length);
          return;
        }
        const skill = matches.find((item) => item.name === name);
        if (skill) {
          const insertion = skills.choose(skill, p.input.current?.value || "");
          if (insertion)
            insertSkill(insertion.text, insertion.start, insertion.end);
        }
      }}
    >
      <AutosizeInput
        handlers={handlers}
        // The autocomplete wraps this handler in its own stable one.
        onKeyDown={keyDown}
        id="message"
        ref={p.input}
        aria-label="Message"
        aria-description={
          p.mobile
            ? "Use the send button to send."
            : p.managed
              ? "Enter sends to the current turn. Tab queues the message for the next turn. Shift + Enter adds a new line."
              : "Enter to send. Shift + Enter for a new line."
        }
        placeholder={
          p.blocked
            ? "Start a new chat or open another chat."
            : p.canSend
              ? "What should we work on?"
              : "This session has no live mailbox"
        }
        disabled={!p.canSend}
        error={p.draftTooLong}
        aria-describedby={p.draftTooLong ? "draft-length-error" : undefined}
      />
    </ComposerAutocomplete>
  );
});
