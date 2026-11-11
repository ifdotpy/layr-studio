import { makeAutoObservable } from "mobx";
import { ApiError, errorText } from "../../../../api.ts";
import { flowValue } from "../../../../flowValue.ts";
import type { ServerApi } from "../../../../servers/net/api.ts";
import type { ServerAreas } from "../../../../app/contracts.ts";
import type { ComplaintDetailResponse } from "../../../../generated/observableResources.ts";
import {
  complaintReplyRequest,
  complaintResponseStore,
  isDefinitiveComplaintRefusal,
  submitComplaintResponse,
  type UserComplaintResponse,
} from "../support/complaintReplyRequest.ts";

export type ComplaintReplySource = {
  area: ServerAreas;
  api: Pick<ServerApi, "get" | "post">;
  storage: Storage;
  scope: string;
  detail: ComplaintDetailResponse;
  requests: Map<string, UserComplaintResponse>;
  onResponse: (detail: ComplaintDetailResponse) => void;
};
export const complaintDraftKey = (scope: string, id: string) =>
  JSON.stringify([`studio-message-responses:${scope}`, id]);
export function createComplaintReply(previous?: UserComplaintResponse) {
  const draft = {
    edited: previous?.text,
    error: null as unknown,
    sending: false,
    retry: !!previous,
    text(saved: string | null): string {
      return this.edited ?? saved ?? "";
    },
    invalid(saved: string | null): boolean {
      const text = this.text(saved);
      return !text.trim() || text.length > 12000;
    },
    edit(text: string, area: ServerAreas, scope: string, id: string) {
      this.edited = text;
      area.unsentText.update("reply", complaintDraftKey(scope, id), text);
    },
    *submit(source: ComplaintReplySource) {
      const { area, api, storage, scope, detail, requests, onResponse } =
        source;
      const id = detail.id,
        draftKey = complaintDraftKey(scope, id);
      const responseStore = complaintResponseStore(storage, scope);
      const savedReply = area.unsentText.read("reply", draftKey);
      if (this.sending || this.invalid(savedReply)) return;
      const { get, post } = api;
      const forget = () => {
        const remaining = new Map(requests);
        remaining.delete(id);
        responseStore.write(remaining);
        requests.delete(id);
        this.retry = false;
      };
      const existing = requests.get(id);
      const payload = complaintReplyRequest(
        existing,
        detail,
        this.text(savedReply).trim(),
        existing?.status || "in_progress",
      );
      requests.set(id, payload);
      try {
        responseStore.write(requests);
      } catch (failure) {
        if (existing) requests.set(id, existing);
        else requests.delete(id);
        this.error = errorText(failure);
        return;
      }
      this.sending = true;
      this.error = "";
      try {
        const outcome = yield* flowValue(
          submitComplaintResponse(
            payload,
            (request) => post("/api/complaints", request),
            (id) => get("/api/complaint", { query: { id } }),
          ),
        );
        forget();
        yield* flowValue(
          area.unsentText.clear("reply", draftKey, payload.text),
        );
        if ((this.edited ?? savedReply ?? "") === payload.text)
          this.edited = "";
        if ("detail" in outcome) onResponse(outcome.detail);
        else area.notices.notify(errorText(outcome.reloadError));
      } catch (failure) {
        if (
          failure instanceof ApiError &&
          isDefinitiveComplaintRefusal(failure.status)
        ) {
          yield* flowValue(
            area.unsentText.clear("reply", draftKey, payload.text),
          );
          try {
            forget();
          } catch (storageFailure) {
            this.error = errorText(storageFailure);
            return;
          }
          if (
            failure.status === 409 ||
            /version|changed|stale/i.test(failure.message)
          ) {
            this.error =
              "This message changed. Review the latest response before you send again.";
            try {
              onResponse(
                yield* flowValue(get("/api/complaint", { query: { id: id } })),
              );
            } catch (refreshFailure) {
              this.error = errorText(refreshFailure);
            }
            return;
          }
          this.error = errorText(failure) || "The response was rejected.";
          return;
        }
        this.retry = requests.has(id);
        this.error = `${errorText(failure)}${this.retry ? " Retry sends the same response." : ""}`;
      } finally {
        this.sending = false;
      }
    },
  };
  return makeAutoObservable(draft, {}, { autoBind: true });
}
