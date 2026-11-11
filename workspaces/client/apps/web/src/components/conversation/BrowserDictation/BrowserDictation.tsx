import { useConversationStore } from "../support/ConversationStore.ts";
import { observer } from "mobx-react-lite";
import { DictationPanel } from "../DictationPanel/DictationPanel.tsx";
import type { DictationProps } from "../Dictation/Dictation.tsx";

export { dictationLanguageOptions } from "../DictationPanel/DictationPanel.tsx";

// Web Speech is implemented by the browser. It is not the Electron speech bridge.
export type Recognition = {
  lang: string;
  continuous: boolean;
  interimResults: boolean;
  onresult:
    | ((event: {
        results: ArrayLike<{ isFinal: boolean; 0: { transcript: string } }>;
      }) => void)
    | null;
  onerror: ((event: { error: string }) => void) | null;
  onstart: (() => void) | null;
  onend: (() => void) | null;
  start(): void;
  stop(): void;
  abort(): void;
};
export type SpeechWindow = Window & {
  SpeechRecognition?: new () => Recognition;
  webkitSpeechRecognition?: new () => Recognition;
};
export const BrowserDictation = observer(function BrowserDictation({
  onInsert,
  disabled = false,
}: DictationProps) {
  const { state, supported, start, stop } =
    useConversationStore().context.browserDictation;

  return (
    <>
      <DictationPanel
        state={state}
        disabled={disabled}
        locked={state.active}
        supported={supported}
      >
        {(!state.active || state.open) && (
          <button
            type="button"
            className="dictation-record"
            disabled={!state.active && (!supported || disabled)}
            onClick={state.active ? stop : start}
          >
            {state.active ? "Stop dictation" : "Record"}
          </button>
        )}
        {state.status && state.open && <p role="status">{state.status}</p>}
        {state.partial && <p className="dictation-note">{state.partial}</p>}
        <label className="dictation-review">
          Review text
          <textarea
            aria-label="Review dictated text"
            value={state.text}
            disabled={state.active}
            onChange={(event) => state.update({ text: event.target.value })}
          />
        </label>
        <button
          type="button"
          className="dictation-record"
          disabled={!state.canInsert}
          onClick={() => onInsert(state.text)}
        >
          Insert into message
        </button>
      </DictationPanel>
      {state.active && !state.open && (
        <div
          className="voice-active-bar"
          role="group"
          aria-label="Active dictation"
        >
          <span role="status">Dictation: {state.status}</span>
          <button type="button" onClick={stop}>
            Stop dictation
          </button>
        </div>
      )}
    </>
  );
});

export { createBrowserDictation } from "./BrowserDictationStore.ts";
