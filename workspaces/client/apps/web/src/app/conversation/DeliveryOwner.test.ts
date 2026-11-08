// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from "vitest";
import { ownerFixture } from "./ownerFixture.ts";
import type { SendMessageBody } from "../../commands.ts";
import { required } from "../../components/conversation/ComposerAttachments/componentTestFixtures.ts";
import { MessageSendStatus } from "../../generated/api.ts";

afterEach(() => vi.restoreAllMocks());
it("retains a definitive server rejection and gives a new send its own identity", async () => {
  const f = ownerFixture();
  const bodies: SendMessageBody[] = [];
  vi.spyOn(f.services.outbox, "durableSend").mockImplementation(
    async (body) => {
      bodies.push(body);
      return {
        kind: "server",
        id: body.id,
        status: MessageSendStatus.failed,
        error: "Rejected",
      };
    },
  );
  try {
    f.delivery.editDraft("same", "lead");
    await expect(f.delivery.send()).rejects.toThrow("Rejected");
    expect(f.delivery.outgoingMessages[0]?.status).toEqual({
      source: "server",
      status: MessageSendStatus.failed,
    });
    await expect(f.delivery.send()).rejects.toThrow("Rejected");
    expect(required(bodies[1]).id).not.toBe(required(bodies[0]).id);
  } finally {
    f.dispose();
  }
});
it("keeps newer text and holds the send lock until the durable callback completes", async () => {
  const f = ownerFixture();
  let persist: (() => void | Promise<void>) | undefined;
  let complete: (() => void) | undefined;
  const callback = Promise.withResolvers<void>();
  const saved = vi.fn(() => callback.promise);
  vi.spyOn(f.services.outbox, "durableSend").mockImplementation(
    (body, _, onPersist) => {
      persist = onPersist;
      return new Promise((resolve) => {
        complete = () => resolve({ kind: "confirmed", id: body.id });
      });
    },
  );
  try {
    f.delivery.editDraft("first", "lead");
    const send = f.delivery.send({ onPersist: saved });
    expect(f.delivery.isSending).toBe(true);
    expect(f.drafts.getDraft("lead")).toBe("first");
    expect(saved).not.toHaveBeenCalled();
    f.delivery.editDraft("newer", "lead");
    const persisted = required(persist)();
    expect(f.delivery.isSending).toBe(true);
    callback.resolve();
    await persisted;
    expect(f.drafts.getDraft("lead")).toBe("newer");
    expect(f.delivery.draftResetVersions.get("lead") ?? 0).toBe(0);
    expect(saved).toHaveBeenCalledOnce();
    expect(f.delivery.isSending).toBe(false);
    required(complete)();
    await send;
  } finally {
    f.dispose();
  }
});
it("reuses the first payload after a storage failure and issues a fresh identity after persistence", async () => {
  const f = ownerFixture();
  const bodies: SendMessageBody[] = [];
  const send = vi
    .spyOn(f.services.outbox, "durableSend")
    .mockImplementationOnce(async (body) => {
      bodies.push(body);
      throw new TypeError("storage unavailable");
    });
  try {
    f.delivery.editDraft("same", "lead");
    await expect(f.delivery.send()).rejects.toThrow("storage unavailable");
    send.mockImplementation(async (body, _, persist) => {
      bodies.push(body);
      await persist?.();
      return { kind: "confirmed", id: body.id };
    });
    await f.delivery.send();
    expect(bodies[1]).toBe(bodies[0]);
    f.delivery.editDraft("same", "lead");
    await f.delivery.send();
    expect(required(bodies[2]).id).not.toBe(required(bodies[0]).id);
    expect(f.drafts.getDraft("lead")).toBe("");
    expect(f.delivery.draftResetVersions.get("lead")).toBe(2);
  } finally {
    f.dispose();
  }
});
it("allows first-send creation and clears only the captured new-chat draft", async () => {
  const f = ownerFixture();
  f.navigation.selectedId = null;
  let persist: (() => void | Promise<void>) | undefined;
  let complete: (() => void) | undefined;
  vi.spyOn(f.services.outbox, "durableSend").mockImplementation(
    (body, _, onPersist) => {
      persist = onPersist;
      return new Promise((resolve) => {
        complete = () => resolve({ kind: "confirmed", id: body.id });
      });
    },
  );
  try {
    f.delivery.editDraft("first draft", "new");
    const sending = f.delivery.send();
    await Promise.resolve();
    expect(f.ensureConversation).toHaveBeenCalledOnce();
    f.delivery.editDraft("new draft", "new");
    await required(persist)();
    required(complete)();
    await sending;
    expect(f.drafts.getDraft("new")).toBe("new draft");
  } finally {
    f.dispose();
  }
});
