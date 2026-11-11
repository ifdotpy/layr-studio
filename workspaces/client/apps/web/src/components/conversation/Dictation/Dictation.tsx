import type { createNativeDictation } from "./NativeDictationStore.ts";
import type { RecordingPreview } from "./RecordingPreview.ts";
import { useConversationStore } from "../support/ConversationStore.ts";
import { hasNativeClient } from "../../../native/client.ts";
import { useServerAudio } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { localTime } from "../../../local-time.ts";
import ErrorDescription from "../../shell/ErrorDescription/ErrorDescription.tsx";
import { observer } from "mobx-react-lite";
import { DictationPanel, duration } from "../DictationPanel/DictationPanel.tsx";
import { Download, Mic, Square, Trash2 } from "lucide-react";

import { BrowserDictation } from "../BrowserDictation/BrowserDictation.tsx";
// Older recordings store plain text. New structured diagnostics retain JSON
// in that same string field; decode only objects and arrays for presentation.
function recordingDiagnostic(value: unknown): unknown {
  if (typeof value !== "string" || !/^[\s]*[[{]/.test(value)) return value;
  try {
    const parsed: unknown = JSON.parse(value);
    return parsed !== null && typeof parsed === "object" ? parsed : value;
  } catch {
    return value;
  }
}
export type DictationProps = {
  chatId: string;
  onInsert: (text: string) => void;
  disabled?: boolean;
};
export function Dictation(props: DictationProps) {
  const { dictationBridge } = useServerAudio();
  const Component = dictationBridge() ? NativeDictation : BrowserDictation;
  return <Component {...props} />;
}
const NativeDictation = observer(function NativeDictation({
  onInsert,
  disabled = false,
}: DictationProps) {
  const control = useConversationStore().context.nativeDictation;
  if (!control) return null;
  const {
    state,
    supported,
    desktop,
    start,
    finish,
    load,
    fail,
    restoreRecording,
  } = control;
  return (
    <DictationPanel
      state={state}
      disabled={disabled}
      locked={state.active || !!state.busy}
      supported={supported}
      saved
    >
      <button
        type="button"
        className="dictation-record"
        disabled={!!state.busy || !supported}
        onClick={() => void (state.active ? finish() : start()).catch(fail)}
      >
        {state.active ? <Square size={14} /> : <Mic size={14} />}{" "}
        {state.active
          ? `Stop · ${duration(state.elapsed)}`
          : state.busy === "start"
            ? "Starting…"
            : "Record"}
      </button>
      {state.attempt && (
        <div className="dictation-progress">
          <p role="status">Transcribing… {state.progress}</p>
          <button
            type="button"
            onClick={() => {
              state.update({ cancelledAttempt: state.attempt });
              void desktop?.cancelTranscription?.(state.attempt).catch(fail);
            }}
          >
            Cancel transcription
          </button>
        </div>
      )}
      {!!state.deleted.length && (
        <div role="status" className="dictation-undo">
          {state.deleted.length} deleted. Undo is available for 24 hours.{" "}
          <button
            type="button"
            onClick={() =>
              void Promise.all(state.deleted.map(restoreRecording))
                .then(load)
                .catch(fail)
            }
          >
            Undo delete
          </button>
        </div>
      )}
      <div className="dictation-recordings">
        {control.recordings.map((preview) => (
          <RecordingItem
            key={preview.row.id}
            preview={preview}
            control={control}
            onInsert={onInsert}
          />
        ))}
      </div>
    </DictationPanel>
  );
});
const RecordingItem = observer(function RecordingItem({
  preview,
  control,
  onInsert,
}: {
  preview: RecordingPreview;
  control: ReturnType<typeof createNativeDictation>;
  onInsert: (text: string) => void;
}) {
  const { row, url, error: downloadError } = preview;
  const review = row.reviewedText ?? row.transcript ?? "";
  const busy = !!control.state.busy || control.state.active;
  const transcribing = control.state.busy === row.id;
  return (
    <section className="dictation-item">
      {downloadError && (
        <p role="alert" className="dictation-error">
          {downloadError}
        </p>
      )}
      <div className="dictation-item-heading">
        <span>
          {localTime(new Date(row.created), {
            hour: "2-digit",
            minute: "2-digit",
          })}{" "}
          · {duration(row.samples / row.sampleRate)}
          {row.state === "recording" ? " · Recovered" : ""}
        </span>
        <a
          href={url || undefined}
          download={`dictation-${row.id}.wav`}
          aria-label="Download recording"
          onClick={(event) => {
            if (!hasNativeClient()) return;
            event.preventDefault();
            void preview.download();
          }}
        >
          <Download size={14} />
        </a>
        <button
          type="button"
          disabled={busy}
          onClick={() =>
            void control
              .trashRecording(row)
              .then(control.load)
              .catch(control.fail)
          }
          aria-label="Delete recording"
        >
          <Trash2 size={14} />
        </button>
      </div>
      {url && <audio controls src={url} preload="metadata" />}
      {row.error && (
        <p role="alert" className="dictation-error">
          <ErrorDescription value={recordingDiagnostic(row.error)} />
        </p>
      )}
      {row.transcript && (
        <label className="dictation-review">
          Review text
          <textarea
            aria-label="Review dictated text"
            value={review}
            disabled={busy}
            onChange={(event) => {
              preview.update({ ...row, reviewedText: event.target.value });
              void control
                .updateRecording({ ...row, reviewedText: event.target.value })
                .catch(control.fail);
            }}
          />
        </label>
      )}
      <div className="dictation-actions">
        <button
          type="button"
          disabled={busy || !row.samples}
          onClick={() => void control.transcribe(row)}
        >
          {transcribing
            ? "Transcribing…"
            : row.error || row.transcript
              ? "Retry transcription"
              : "Transcribe"}
        </button>
        {row.transcript && (
          <button
            type="button"
            disabled={busy || !review.trim()}
            onClick={() => {
              try {
                onInsert(review);
              } catch (error) {
                control.fail(error);
              }
            }}
          >
            Insert into message
          </button>
        )}
      </div>
    </section>
  );
});

export { createNativeDictation } from "./NativeDictationStore.ts";
