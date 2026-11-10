import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { hasNativeClient } from "../../../native/client.ts";
import { saveFile } from "../../../fileActions.ts";
import { useServerAudio } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { useServerActivity } from "../../../servers/activity.ts";
import { localTime } from "../../../local-time.ts";
import ErrorDescription from "../../shell/ErrorDescription/ErrorDescription.tsx";
import { errorDetails } from "../../../errorPresentation.ts";
import { observer } from "mobx-react-lite";
import {
  DictationPanel,
  DictationStore,
  duration,
} from "../DictationPanel/DictationPanel.tsx";
import { useEffect, useRef, useState } from "react";
import { Download, Mic, Square, Trash2 } from "lucide-react";
import type { Recording } from "../../../dictation/storage.ts";
import { useServerRecordings } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";

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
  chatId,
  onInsert,
  disabled = false,
}: DictationProps) {
  const {
    deleteRecording,
    listRecordings,
    recordingAudio,
    saveChunk,
    saveRecording,
    updateRecording,
    beginTranscription,
    trashRecording,
    restoreRecording,
    deletedRecordings,
  } = useServerRecordings();
  const { dictationBridge, captureAudio } = useServerAudio();
  const desktop = dictationBridge();
  const [state] = useState(() => new DictationStore());
  const supported =
    !!desktop?.requestMicrophone &&
    !!desktop?.prepareTranscription &&
    !!desktop?.transcribeAudio;
  const capture = useRef<Awaited<ReturnType<typeof captureAudio>> | null>(null);
  const mounted = useRef(true),
    active = useRef<Recording | null>(null),
    queue = useRef(Promise.resolve());
  const durable = useRef<Recording | null>(null);
  useServerActivity(state.active || !!state.busy || state.storageFailed);
  const scope = useRef(chatId);
  scope.current = chatId;
  const load = async () => {
    const result = await listRecordings(chatId);
    const removed = await deletedRecordings(chatId);
    if (mounted.current && scope.current === chatId) {
      state.update({ rows: result, deleted: removed });
    }
  };
  const fail = (e: unknown) => {
    if (mounted.current) state.update({ error: e });
  };
  const stop = async () => {
    let ready: Recording | undefined;
    if (mounted.current) {
      state.update({ busy: "stop", active: false });
    }
    const recorder = capture.current;
    capture.current = null;
    let finalError: unknown;
    if (recorder) {
      try {
        await recorder.stop();
      } catch (error) {
        finalError = error;
      }
    }
    try {
      await queue.current;
      const row = active.current;
      active.current = null;
      if (row) {
        const saved = durable.current || row;
        if (saved.samples) {
          ready = {
            ...saved,
            state: "ready",
            ...(finalError
              ? {
                  error: errorDetails(finalError),
                }
              : {}),
          };
          await updateRecording(ready);
        } else await deleteRecording(row.id);
      }
    } catch (error) {
      finalError ||= error;
    } finally {
      if (mounted.current) {
        state.update({ active: false, busy: "" });
        try {
          await load();
        } catch (error) {
          finalError ||= error;
        }
      }
    }
    if (finalError) throw finalError;
    return ready;
  };
  useEffect(() => {
    mounted.current = true;
    state.update({ rows: [], error: "" });
    void load().catch(fail);
    return () => {
      mounted.current = false;
      void stop().catch(() => {});
    };
  }, [chatId]);
  useEffect(
    () =>
      desktop?.onTranscriptionProgress?.((value) => {
        if (value.id === state.attempt)
          state.update({
            progress: `${value.completed} of ${value.total} parts`,
          });
      }),
    [],
  );
  useEffect(() => {
    if (state.open) void load().catch(fail);
  }, [state.open]);
  useEffect(() => {
    if (!state.active) return;
    const id = setInterval(
      () => state.update({ elapsed: state.elapsed + 1 }),
      1000,
    );
    return () => clearInterval(id);
  }, [state.active]);
  const start = async () => {
    state.update({ error: "" });
    if (!supported) {
      state.update({
        error: "Update the desktop app before you record dictation.",
      });
      return;
    }
    state.update({ busy: "start" });
    const owner = chatId;
    let index = 0;
    try {
      // Call the native permission bridge synchronously within the button gesture.
      if (desktop && !desktop.requestMicrophone)
        throw Error(
          "Install the updated desktop app to enable microphone access.",
        );
      if (desktop?.requestMicrophone) await desktop.requestMicrophone();
      if (!mounted.current || scope.current !== owner) return;
      const row: Recording = {
        id: randomUUID(),
        chatId: owner,
        created: Date.now(),
        sampleRate: 16000,
        samples: 0,
        state: "recording",
      };
      await saveRecording(row);
      if (!mounted.current || scope.current !== owner) {
        await deleteRecording(row.id);
        return;
      }
      active.current = row;
      durable.current = { ...row };
      state.update({ storageFailed: false });
      const recorder = await captureAudio((pcm) => {
        if (!active.current || active.current.id !== row.id) return;
        row.samples += pcm.length;
        const snapshot = { ...row },
          number = index++;
        queue.current = queue.current
          .then(async () => {
            if (state.storageFailed) return;
            await saveChunk(snapshot, number, pcm);
            durable.current = snapshot;
          })
          .catch((e) => {
            state.update({ storageFailed: true });
            fail(
              Error(
                `Recording stopped: ${errorDetails(e)}. Saved audio remains available.`,
              ),
            );
            void stop().catch(fail);
          });
        if (row.samples / row.sampleRate >= 1800) {
          state.update({ error: "The 30 minute recording limit was reached." });
          void stop().catch(fail);
        }
      });
      row.sampleRate = recorder.sampleRate;
      capture.current = recorder;
      if (!mounted.current || scope.current !== owner) {
        await stop();
        return;
      }
      state.update({ elapsed: 0, active: true, busy: "" });
    } catch (e) {
      await stop().catch(() => {});
      fail(e);
      if (mounted.current) state.update({ busy: "" });
    }
  };
  const transcribe = async (
    row: Recording,
    reservation?: ReturnType<typeof prepare>,
  ) => {
    const owner = chatId,
      attempt = randomUUID();
    state.update({ busy: row.id, error: "" });
    state.update({ progress: "Preparing audio…", attempt });
    try {
      // Reserve the native gesture synchronously, then claim this saved recording.
      const prepared = reservation || prepare();
      if (!(await beginTranscription(row, attempt)))
        throw Error("The recording was deleted.");
      const authorization = await prepared;
      if (authorization.error) throw authorization.error;
      if (state.cancelledAttempt === attempt)
        throw Error("Transcription cancelled. Your recording is saved.");
      const audio = await recordingAudio(row);
      if (state.cancelledAttempt === attempt)
        throw Error("Transcription cancelled. Your recording is saved.");
      const result = await desktop!.transcribeAudio({
        id: attempt,
        permit: authorization.permit!,
        audio: await audio.arrayBuffer(),
        locale: state.locale,
      });
      if (state.cancelledAttempt === attempt)
        throw Error("Transcription cancelled. Your recording is saved.");
      await updateRecording(
        {
          ...row,
          transcriptionAttempt: attempt,
          state: "ready",
          transcript: result.text,
          reviewedText: undefined,
          error: undefined,
        },
        attempt,
      );
    } catch (e) {
      await updateRecording(
        {
          ...row,
          transcriptionAttempt: attempt,
          state: "ready",
          error: errorDetails(e),
        },
        attempt,
      ).catch(() => {});
    } finally {
      if (mounted.current && scope.current === owner) {
        state.update({ busy: "", attempt: "", progress: "" });
        await load().catch(fail);
      }
    }
  };
  function prepare() {
    return desktop?.transcribeAudio
      ? desktop.prepareTranscription().then(
          (permit) => ({ permit, error: undefined }),
          (error) => ({ permit: undefined, error }),
        )
      : Promise.resolve({
          permit: undefined,
          error: Error(
            "Transcription is unavailable in this app. Use an updated desktop app with transcription support. Your recording is saved.",
          ),
        });
  }
  const finish = async () => {
    // Reserve native authorization while Stop is still a trusted button gesture.
    const reservation = prepare();
    const row = await stop();
    if (row && mounted.current) await transcribe(row, reservation);
  };
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
        {state.rows.map((row) => (
          <RecordingItem
            key={row.id}
            row={row}
            busy={!!state.busy || state.active}
            transcribing={state.busy === row.id}
            onTranscribe={() => void transcribe(row)}
            onReview={(reviewedText) =>
              void updateRecording({ ...row, reviewedText }).catch(fail)
            }
            onDelete={() => void trashRecording(row).then(load).catch(fail)}
            onInsert={(text) => {
              if (row.transcript && row.chatId === scope.current) {
                try {
                  onInsert(text);
                } catch (e) {
                  fail(e);
                }
              }
            }}
          />
        ))}
      </div>
    </DictationPanel>
  );
});
function RecordingItem({
  row,
  busy,
  transcribing,
  onTranscribe,
  onDelete,
  onInsert,
  onReview,
}: {
  row: Recording;
  busy: boolean;
  transcribing: boolean;
  onTranscribe: () => void;
  onDelete: () => void;
  onInsert: (text: string) => void;
  onReview: (text: string) => void;
}) {
  const { recordingAudio } = useServerRecordings();
  const [url, setUrl] = useState("");
  const audioBytes = useRef<ArrayBuffer | null>(null);
  const [downloadError, setDownloadError] = useState("");
  const [review, setReview] = useState(
    row.reviewedText ?? row.transcript ?? "",
  );
  useEffect(
    () => setReview(row.reviewedText ?? row.transcript ?? ""),
    [row.transcript, row.reviewedText],
  );
  useEffect(() => {
    let alive = true,
      objectURL = "";
    audioBytes.current = null;
    void recordingAudio(row)
      .then(async (blob) => {
        if (hasNativeClient()) {
          const bytes = await blob.arrayBuffer();
          if (alive) audioBytes.current = bytes;
        }
        objectURL = URL.createObjectURL(blob);
        if (alive) setUrl(objectURL);
        else URL.revokeObjectURL(objectURL);
      })
      .catch((error) => {
        if (alive) setDownloadError(String(error));
      });
    return () => {
      alive = false;
      if (objectURL) URL.revokeObjectURL(objectURL);
    };
  }, [row.id, row.samples]);
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
            if (!audioBytes.current) {
              setDownloadError("The saved recording is still loading.");
              return;
            }
            setDownloadError("");
            void saveFile({
              name: `dictation-${row.id}.wav`,
              data: audioBytes.current,
              mime: "audio/wav",
            }).catch((error) => setDownloadError(String(error)));
          }}
        >
          <Download size={14} />
        </a>
        <button
          type="button"
          disabled={busy}
          onClick={onDelete}
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
              setReview(event.target.value);
              onReview(event.target.value);
            }}
          />
        </label>
      )}
      <div className="dictation-actions">
        <button
          type="button"
          disabled={busy || !row.samples}
          onClick={onTranscribe}
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
            onClick={() => onInsert(review)}
          >
            Insert into message
          </button>
        )}
      </div>
    </section>
  );
}
