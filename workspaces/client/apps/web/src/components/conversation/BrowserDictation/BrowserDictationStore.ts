import type { DictationStore } from "../DictationPanel/DictationPanel.tsx";
import type { Recognition, SpeechWindow } from "./BrowserDictation.tsx";
export function createBrowserDictation(state: DictationStore) {
  const recognition = { current: null as Recognition | null };
  let stopTimer: ReturnType<typeof setTimeout> | undefined;
  const Constructor =
    (window as SpeechWindow).SpeechRecognition ||
    (window as SpeechWindow).webkitSpeechRecognition;
  const supported = window.isSecureContext && !!Constructor;

  const start = () => {
    if (!supported || !Constructor || recognition.current) return;
    state.update({
      error: "",
      partial: "",
      status: "Starting microphone…",
      active: true,
    });
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
        partial: pending.join(" "),
      });
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
      clearTimeout(stopTimer);
      recognition.current = null;
      state.update({
        active: false,
        partial: "",
        status: "Microphone stopped. Review the text.",
      });
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
    stopTimer = setTimeout(() => {
      if (recognition.current !== current) return;
      current.abort();
      recognition.current = null;
      state.update({
        active: false,
        partial: "",
        status: "Microphone stopped. Review the text.",
      });
      state.update({
        error:
          "The browser did not return the final words. Check the text before you insert it.",
      });
    }, 5000);
  };
  return {
    state,
    supported,
    start,
    stop,
    dispose() {
      clearTimeout(stopTimer);
      const current = recognition.current;
      recognition.current = null;
      current?.abort();
    },
  };
}
