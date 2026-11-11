import { now } from "../../../clock.ts";
import type { ReactNode } from "react";
import { observer } from "mobx-react-lite";
import { Popover } from "@mantine/core";
import { Mic } from "lucide-react";
import ErrorDescription from "../../shell/ErrorDescription/ErrorDescription.tsx";
import { makeAutoObservable, observableRef } from "mobx";
import type { Recording } from "../../../dictation/storage.ts";

// One scope-local state owner shared by native and browser dictation.
export class DictationStore {
  open = false;
  active = false;
  status = "";
  error: unknown = "";
  text = "";
  partial = "";
  locale = navigator.language;
  deleted: Recording[] = [];
  busy = "";
  progress = "";
  attempt = "";
  cancelledAttempt = "";
  startedAt = 0;
  get elapsed() {
    return this.active ? Math.floor((now() - this.startedAt) / 1000) : 0;
  }
  set elapsed(seconds: number) {
    this.startedAt = Date.now() - seconds * 1000;
  }
  storageFailed = false;
  constructor() {
    makeAutoObservable(this, { deleted: observableRef }, { autoBind: true });
  }
  get canInsert() {
    return !this.active && !!this.text.trim();
  }
  update(values: Partial<DictationStore>) {
    Object.assign(this, values);
  }
}

const dictationLocales = [
  "en-US",
  "ru-RU",
  "de-DE",
  "fr-FR",
  "es-ES",
  "pl-PL",
  "uk-UA",
] as const;
let languageNames: { locale: string; names: Intl.DisplayNames } | undefined;
export function dictationLanguageOptions() {
  const locale = navigator.language;
  if (languageNames?.locale !== locale)
    languageNames = {
      locale,
      names: new Intl.DisplayNames([locale], { type: "language" }),
    };
  const names = languageNames.names;
  return [...new Set([locale, ...dictationLocales])].map((code) => (
    <option key={code} value={code}>
      {names.of(code) || code}
    </option>
  ));
}

export const duration = (seconds: number) =>
  `${Math.floor(seconds / 60)}:${String(Math.floor(seconds % 60)).padStart(2, "0")}`;

export const DictationPanel = observer(function DictationPanel({
  state,
  disabled,
  locked,
  supported,
  saved = false,
  children,
}: {
  state: DictationStore;
  disabled: boolean;
  locked: boolean;
  supported: boolean;
  saved?: boolean;
  children: ReactNode;
}) {
  return (
    <Popover
      opened={state.open}
      onChange={(open) => state.update({ open })}
      position="top-start"
      width={360}
      withinPortal
    >
      <Popover.Target>
        <button
          type="button"
          className={`dictation-trigger ${state.active ? "is-recording" : ""}`}
          aria-label="Dictation"
          title="Dictation"
          disabled={disabled && !locked}
          onClick={() => state.update({ open: !state.open })}
        >
          <Mic size={18} />
          {saved && state.active && <span>{duration(state.elapsed)}</span>}
        </button>
      </Popover.Target>
      <Popover.Dropdown className="dictation-popover">
        <header>
          <strong>Dictation</strong>
          {saved && <span>Audio saved on this device</span>}
        </header>
        <p className="dictation-note">
          {saved
            ? "Stop to transcribe."
            : "Your browser converts speech to text. Its speech service may get the audio."}
        </p>
        {!supported && (
          <p role="status">
            {saved
              ? "Update the desktop app before you record dictation."
              : "Dictation is unavailable in this browser. Use keyboard dictation or the desktop app."}
          </p>
        )}
        <label className="dictation-language">
          Language{" "}
          <select
            aria-label="Dictation language"
            value={state.locale}
            disabled={locked}
            onChange={(event) => state.update({ locale: event.target.value })}
          >
            {dictationLanguageOptions()}
          </select>
        </label>
        {!!state.error && (
          <p role="alert" className="dictation-error">
            <ErrorDescription value={state.error} />
          </p>
        )}
        {children}
      </Popover.Dropdown>
    </Popover>
  );
});
