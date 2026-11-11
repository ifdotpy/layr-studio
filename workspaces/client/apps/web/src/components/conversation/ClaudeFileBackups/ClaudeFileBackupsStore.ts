import { requestId as randomUUID } from "studio-claude-bridge/request-id.ts";
import { makeAutoObservable, observableRef, onBecomeObserved } from "mobx";
import { errorText, type PostResult, type ServerApi } from "../../../api.ts";

export class ClaudeFileBackupsStore {
  usage: PostResult<"/api/claude/file-backups"> | null = null;
  busy = false;
  message = "";
  requestId: string | null = null;
  active = true;
  readonly api;
  readonly agentId;
  constructor(api: Pick<ServerApi, "post">, agentId: string) {
    this.api = api;
    this.agentId = agentId;
    makeAutoObservable(
      this,
      { api: false, usage: observableRef },
      { autoBind: true },
    );
    onBecomeObserved(this, "usage", this.load);
  }
  *load() {
    try {
      const usage: NonNullable<typeof this.usage> = yield this.api.post(
        "/api/claude/file-backups",
        {
          id: this.agentId,
          release: false,
          request_id: randomUUID(),
        },
      );
      if (this.active) this.usage = usage;
    } catch (error) {
      if (this.active) this.message = errorText(error);
    }
  }
  *release(onReleased: () => void) {
    if (this.busy) return;
    this.busy = true;
    this.requestId ||= randomUUID();
    try {
      const usage: NonNullable<typeof this.usage> = yield this.api.post(
        "/api/claude/file-backups",
        {
          id: this.agentId,
          release: true,
          request_id: this.requestId,
        },
      );
      this.requestId = null;
      if (!this.active) return;
      this.usage = usage;
      this.message = usage.released
        ? `Released ${usage.released} backups.`
        : "No completed backups can be released in this chat. Check other Claude chats.";
      if (usage.released) onReleased();
    } catch (error) {
      if (this.active) this.message = errorText(error);
    } finally {
      this.busy = false;
    }
  }
  dispose() {
    this.active = false;
  }
}
