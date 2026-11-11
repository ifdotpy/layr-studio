import {
  createRecordingPreview,
  type RecordingPreview,
} from "./RecordingPreview.ts";
import { flow, reaction, observable } from "mobx";
import type { ServerServices } from "../../../servers/serverServices.ts";
import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { errorDetails } from "../../../errorPresentation.ts";
import type { Recording } from "../../../dictation/storage.ts";
import type { DictationStore } from "../DictationPanel/DictationPanel.tsx";

export function createNativeDictation(
  services: {
    recordings: ServerServices["recordings"];
    audio: Pick<ServerServices["audio"], "dictationBridge" | "captureAudio">;
  },
  state: DictationStore,
  chatId: string,
) {
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
  } = services.recordings;
  const { dictationBridge, captureAudio } = services.audio;
  const desktop = dictationBridge();
  const previews = observable.map<string, RecordingPreview>();
  const supported =
    !!desktop?.requestMicrophone &&
    !!desktop?.prepareTranscription &&
    !!desktop?.transcribeAudio;
  let capture: Awaited<ReturnType<typeof captureAudio>> | null = null;
  let mounted = true,
    active: Recording | null = null,
    durable: Recording | null = null;
  let queue = Promise.resolve();
  const load = flow(function* () {
    const result: Recording[] = yield listRecordings(chatId);
    const removed: Recording[] = yield deletedRecordings(chatId);
    const next = result.map((row) => {
      const previous = previews.get(row.id);
      if (previous && previous.row.samples === row.samples) {
        previous.update(row);
        return previous;
      }
      return createRecordingPreview(row, recordingAudio);
    });
    previews.replace(
      next.map((preview): [string, RecordingPreview] => [
        preview.row.id,
        preview,
      ]),
    );
    state.update({ deleted: removed });
  });
  const fail = (e: unknown) => {
    state.update({ error: e });
  };
  const stop = flow(function* () {
    let ready: Recording | undefined;
    state.update({ busy: "stop", active: false });
    const recorder = capture;
    capture = null;
    let finalError: unknown;
    if (recorder) {
      try {
        yield recorder.stop();
      } catch (error) {
        finalError = error;
      }
    }
    try {
      yield queue;
      const row = active;
      active = null;
      if (row) {
        const saved = durable || row;
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
          yield updateRecording(ready);
        } else yield deleteRecording(row.id);
      }
    } catch (error) {
      finalError ||= error;
    } finally {
      if (mounted) {
        state.update({ active: false, busy: "" });
        try {
          yield load();
        } catch (error) {
          finalError ||= error;
        }
      }
    }
    if (finalError) throw finalError;
    return ready;
  });
  const start = flow(function* () {
    state.update({ error: "" });
    if (!supported) {
      state.update({
        error: "Update the desktop app before you record dictation.",
      });
      return;
    }
    state.update({ busy: "start" });
    let index = 0;
    try {
      // Call the native permission bridge synchronously within the button gesture.
      if (desktop && !desktop.requestMicrophone)
        throw Error(
          "Install the updated desktop app to enable microphone access.",
        );
      if (desktop?.requestMicrophone) yield desktop.requestMicrophone();
      if (!mounted) return;
      const row: Recording = {
        id: randomUUID(),
        chatId,
        created: Date.now(),
        sampleRate: 16000,
        samples: 0,
        state: "recording",
      };
      yield saveRecording(row);
      if (!mounted) {
        yield deleteRecording(row.id);
        return;
      }
      active = row;
      durable = { ...row };
      state.update({ storageFailed: false });
      const recorder: Awaited<ReturnType<typeof captureAudio>> =
        yield captureAudio((pcm) => {
          if (!active || active.id !== row.id) return;
          row.samples += pcm.length;
          const snapshot = { ...row },
            number = index++;
          queue = queue
            .then(
              flow(function* () {
                if (state.storageFailed) return;
                yield saveChunk(snapshot, number, pcm);
                durable = snapshot;
              }),
            )
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
            state.update({
              error: "The 30 minute recording limit was reached.",
            });
            void stop().catch(fail);
          }
        });
      row.sampleRate = recorder.sampleRate;
      capture = recorder;
      if (!mounted) {
        yield stop();
        return;
      }
      state.update({ elapsed: 0, active: true, busy: "" });
    } catch (e) {
      yield stop().catch(() => {});
      fail(e);
      if (mounted) state.update({ busy: "" });
    }
  });
  const transcribe = flow(function* (
    row: Recording,
    reservation?: ReturnType<typeof prepare>,
  ) {
    const attempt = randomUUID();
    state.update({ busy: row.id, error: "" });
    state.update({ progress: "Preparing audio…", attempt });
    try {
      // Reserve the native gesture synchronously, then claim this saved recording.
      const prepared = reservation || prepare();
      if (!(yield beginTranscription(row, attempt)))
        throw Error("The recording was deleted.");
      const authorization: Awaited<ReturnType<typeof prepare>> = yield prepared;
      if (authorization.error) throw authorization.error;
      if (state.cancelledAttempt === attempt)
        throw Error("Transcription cancelled. Your recording is saved.");
      const audio: Blob = yield recordingAudio(row);
      if (state.cancelledAttempt === attempt)
        throw Error("Transcription cancelled. Your recording is saved.");
      const bytes: ArrayBuffer = yield audio.arrayBuffer();
      const result: Awaited<
        ReturnType<NonNullable<typeof desktop>["transcribeAudio"]>
      > = yield desktop!.transcribeAudio({
        id: attempt,
        permit: authorization.permit!,
        audio: bytes,
        locale: state.locale,
      });
      if (state.cancelledAttempt === attempt)
        throw Error("Transcription cancelled. Your recording is saved.");
      yield updateRecording(
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
      yield updateRecording(
        {
          ...row,
          transcriptionAttempt: attempt,
          state: "ready",
          error: errorDetails(e),
        },
        attempt,
      ).catch(() => {});
    } finally {
      if (mounted) {
        state.update({ busy: "", attempt: "", progress: "" });
        yield load().catch(fail);
      }
    }
  });
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
  const finish = flow(function* () {
    // Reserve native authorization while Stop is still a trusted button gesture.
    const reservation = prepare();
    const row: Recording | undefined = yield stop();
    if (row && mounted) yield transcribe(row, reservation);
  });

  let stops: (() => void)[] = [];
  return {
    state,
    get recordings() {
      return [...previews.values()];
    },
    supported,
    desktop,
    start,
    finish,
    transcribe,
    load,
    fail,
    updateRecording,
    trashRecording,
    restoreRecording,
    startOwner() {
      mounted = true;
      state.update({ error: "" });
      void load().catch(fail);
      stops = [
        desktop?.onTranscriptionProgress?.((value) => {
          if (value.id === state.attempt)
            state.update({
              progress: `${value.completed} of ${value.total} parts`,
            });
        }) || (() => {}),
        reaction(
          () => state.open,
          (open) => {
            if (open) void load().catch(fail);
          },
        ),
      ];
    },
    dispose() {
      mounted = false;
      stops.splice(0).forEach((stop) => stop());
      void stop().catch(() => {});
    },
  };
}
