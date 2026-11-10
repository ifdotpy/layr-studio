import { useServerActivity } from "../../../servers/activity.ts";
import { observer } from "mobx-react-lite";
import {
  DictationPanel,
  DictationStore,
} from "../DictationPanel/DictationPanel.tsx";
import type { DictationProps } from "../Dictation/Dictation.tsx";
import { useEffect, useRef, useState } from "react";

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
  chatId,
  onInsert,
  disabled = false,
}: DictationProps) {
  const [state] = useState(() => new DictationStore());
  useServerActivity(state.active);
  const recognition = useRef<Recognition | null>(null);
  const stopTimer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );
  const Constructor =
    (window as SpeechWindow).SpeechRecognition ||
    (window as SpeechWindow).webkitSpeechRecognition;
  const supported = window.isSecureContext && !!Constructor;
  useEffect(() => {
    state.update({ text: "", active: false });
    return () => {
      clearTimeout(stopTimer.current);
      const current = recognition.current;
      recognition.current = null;
      if (current) {
        current.onresult = null;
        current.onend = null;
        current.onerror = null;
        current.onstart = null;
        current.abort();
      }
    };
  }, [chatId]);
  const start = () => {
    if (!supported || !Constructor || recognition.current) return;
    state.update({ error: "", partial: "" });
    state.update({ status: "Starting microphone…", active: true });
    const current = new Constructor();
    const prefix = state.text.trim();
    recognition.current = current;
    current.lang = state.locale;
    current.continuous = true;
    current.interimResults = true;
    current.onstart = () => {
      if (recognition.current === current)
        state.update({ status: "Listening" });
    };
    current.onresult = (event) => {
      if (recognition.current !== current) return;
      const final: string[] = [],
        pending: string[] = [];
      Array.from(event.results).forEach((result) =>
        (result.isFinal ? final : pending).push(result[0].transcript),
      );
      state.update({
        text: [prefix, final.join(" ")].filter(Boolean).join("\n"),
      });
      state.update({ partial: pending.join(" ") });
    };
    current.onerror = (event) => {
      if (recognition.current !== current) return;
      const messages: Record<string, string> = {
        "not-allowed":
          "Microphone access was denied. Allow access in the browser settings, then try again.",
        "service-not-allowed":
          "Speech recognition is unavailable in this browser. Use keyboard dictation or the desktop app.",
        network:
          "The browser speech service lost its connection. The text already shown remains available here.",
        "no-speech":
          "No speech was detected. Check the microphone, then try again.",
        "audio-capture":
          "The microphone is unavailable. Check its connection and browser access.",
        "language-not-supported":
          "The browser speech service does not support this language.",
      };
      if (event.error !== "aborted")
        state.update({
          error:
            messages[event.error] ||
            "Dictation stopped. The text already shown remains available here.",
        });
      if (event.error !== "aborted") current.abort();
    };
    current.onend = () => {
      if (recognition.current !== current) return;
      clearTimeout(stopTimer.current);
      recognition.current = null;
      state.update({ active: false });
      state.update({ partial: "" });
      state.update({ status: "Microphone stopped. Review the text." });
    };
    try {
      current.start();
    } catch (failure) {
      recognition.current = null;
      state.update({ active: false, status: "", error: String(failure) });
    }
  };
  const stop = () => {
    const current = recognition.current;
    if (!current) return;
    state.update({ status: "Finishing text…" });
    current.stop();
    stopTimer.current = setTimeout(() => {
      if (recognition.current !== current) return;
      current.abort();
      recognition.current = null;
      state.update({ active: false });
      state.update({ partial: "" });
      state.update({ status: "Microphone stopped. Review the text." });
      state.update({
        error:
          "The browser did not return the final words. Check the text before you insert it.",
      });
    }, 5000);
  };
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
