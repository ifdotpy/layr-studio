import { whileObserved } from "../../../observed.ts";
import { flowResult, makeAutoObservable, observableRef } from "mobx";
import { hasNativeClient } from "../../../native/client.ts";
import { saveFile } from "../../../fileActions.ts";
import type { ServerServices } from "../../../servers/serverServices.ts";
import type { Recording } from "../../../dictation/storage.ts";

export function createRecordingPreview(
  row: Recording,
  readAudio: ServerServices["recordings"]["recordingAudio"],
) {
  let audioBytes: ArrayBuffer | null = null;
  const preview = makeAutoObservable(
    {
      row,
      url: "",
      error: "",
      update(row: Recording) {
        this.row = row;
      },
      *load() {
        try {
          const blob: Blob = yield readAudio(this.row);
          audioBytes = hasNativeClient() ? yield blob.arrayBuffer() : null;
          this.url = URL.createObjectURL(blob);
        } catch (error) {
          this.error = String(error);
        }
      },
      *download() {
        if (!audioBytes) {
          this.error = "The saved recording is still loading.";
          return;
        }
        this.error = "";
        try {
          yield saveFile({
            name: `dictation-${this.row.id}.wav`,
            data: audioBytes,
            mime: "audio/wav",
          });
        } catch (error) {
          this.error = String(error);
        }
      },
      release() {
        if (this.url) URL.revokeObjectURL(this.url);
        this.url = "";
        audioBytes = null;
      },
    },
    { row: observableRef },
    { autoBind: true },
  );
  whileObserved(preview, "url", () => {
    const task = flowResult(preview.load());
    void task.catch(() => {});
    return () => {
      task.cancel();
      preview.release();
    };
  });
  return preview;
}
export type RecordingPreview = ReturnType<typeof createRecordingPreview>;
