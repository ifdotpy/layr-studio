import { transientTestFixture } from "../../../sync/subscriptions/transientTestFixture.ts";
import { createApiStorage } from "../../../apiStorage.ts";
import { autorun, flowResult } from "mobx";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ServerCommand } from "../../../servers/navigation.ts";
import { accountsFixture, personal } from "./Accounts.fixture.ts";
let fixture: ReturnType<typeof accountsFixture>;
beforeEach(() => {
  fixture = accountsFixture();
});
afterEach(() => fixture.close());
const command = (
  operation: Extract<ServerCommand, { action: "account-action" }>["operation"],
  accountKey = "team",
) =>
  ({
    action: "account-action",
    operation,
    accountKey,
    provider: "claude",
    email: personal.email,
    label: "Team",
  }) satisfies ServerCommand;

it("opens the exact Team profile for a configure command, not the first matching email", () => {
  fixture.ui.receive(command("configure-profile"));
  expect(fixture.ui.dialog).toEqual({
    kind: "configureProfile",
    accountKey: "team",
  });
  expect(fixture.ui.dialogAccount?.id).toBe("team");
});
it("does not open Add or a guessed profile for a keyless sign-in command", () => {
  // @ts-expect-error A malformed external command deliberately violates the exact-key contract.
  fixture.ui.receive({
    action: "account-sign-in",
    provider: "claude",
    email: personal.email,
    label: "Personal",
    serverLabel: "Fixture",
  });
  expect(fixture.ui.dialog).toBeNull();
  expect(fixture.ui.error).toBe(
    "This account is no longer saved on this server.",
  );
});
it("does not open Add or another profile when an exact sign-in key is missing", () => {
  fixture.ui.receive({
    action: "account-sign-in",
    accountKey: "deleted",
    provider: "claude",
    email: personal.email,
    label: "Personal",
    serverLabel: "Fixture",
  });
  expect(fixture.ui.dialog).toBeNull();
});
it("preserves an explicit cross-server Add request even when the email already has profiles", () => {
  fixture.ui.receive({
    action: "account-sign-in",
    forceAdd: true,
    provider: "claude",
    email: personal.email,
    label: "Another",
    serverLabel: "Other server",
  });
  expect(fixture.ui.dialog?.kind).toBe("add");
  expect(fixture.ui.addDraft.email).toBe(personal.email);
  expect(fixture.ui.addDraft.label).toBe("Another");
  expect(fixture.ui.addIntent?.serverLabel).toBe("Other server");
});
it("resolves an external default command without refreshing accounts and executes it once", async () => {
  const next = command("default");
  fixture.ui.receive(next);
  fixture.ui.receive(next);
  await Promise.resolve();
  await Promise.resolve();
  fixture.ui.receive(next);
  expect(fixture.post).toHaveBeenCalledExactlyOnceWith(
    "/api/accounts/default",
    { account_key: "team" },
  );
});
it("retries a rename with the identical submitted request after a lost reply", async () => {
  fixture.post.mockRejectedValueOnce(new Error("Reply lost"));
  fixture.ui.receive(command("rename"));
  fixture.ui.setRenameLabel("Renamed");
  await fixture.ui.saveRename();
  expect(fixture.ui.error).toBe("Reply lost");
  fixture.ui.setRenameLabel("Changed after uncertainty");
  expect(fixture.ui.renameLabel).toBe("Renamed");
  fixture.ui.close();
  fixture.ui.receive(command("rename"));
  await fixture.ui.saveRename();
  expect(fixture.post).toHaveBeenCalledTimes(2);
  expect(fixture.post.mock.calls[1]).toEqual(fixture.post.mock.calls[0]);
  expect(fixture.ui.dialog).toBeNull();
});
it("locks synchronous duplicate submits and close while an action is pending", async () => {
  let reject!: (reason: Error) => void;
  fixture.post.mockImplementationOnce(
    () =>
      new Promise((_resolve, fail) => {
        reject = fail;
      }),
  );
  fixture.ui.receive(command("rename"));
  fixture.ui.setRenameLabel("Renamed");
  const pending = fixture.ui.saveRename();
  fixture.ui.saveRename();
  fixture.ui.close();
  fixture.ui.receive(command("remove"));
  expect(fixture.ui.dialog?.kind).toBe("rename");
  expect(fixture.ui.error).toContain("Wait");
  expect(fixture.post).toHaveBeenCalledOnce();
  reject(new Error("Lost reply"));
  await pending;
  expect(fixture.ui.pending).toBe("");
});
it("reads dialog accounts and default replacements directly from generated updates", () => {
  fixture.ui.receive(command("disconnect", "personal"));
  expect(fixture.ui.targetsDefault).toBe(true);
  expect(fixture.ui.replacementAccount?.id).toBe("team");
  fixture.publish({
    ...fixture.summary,
    accounts: fixture.summary.accounts.map((account) => ({
      ...account,
      label: "Updated",
      ...(account.id === "team" ? { disconnected: true } : {}),
    })),
  });
  expect(fixture.ui.dialogAccount?.label).toBe("Updated");
  expect(fixture.ui.canConfirmRemoval).toBe(false);
});

it.each(["rename", "configure-profile", "disconnect", "remove"] as const)(
  "disposes the owner-created sign-in flow before a %s dialog replaces it",
  async (operation) => {
    vi.useFakeTimers();
    const stream = transientTestFixture();
    const services = fixture.areas.scope.services;
    vi.spyOn(services, "store", "get").mockReturnValue(stream.store);
    vi.spyOn(services.resources, "publish").mockImplementation(
      stream.subscriptions.publish,
    );
    Object.assign(services.api, createApiStorage(services.storage));
    let stopReading = () => {};
    try {
      fixture.publish({
        ...fixture.summary,
        accounts: fixture.summary.accounts.map((account) => ({
          ...account,
          disconnected: true,
        })),
      });
      fixture.ui.receive({
        action: "account-sign-in",
        accountKey: "personal",
        provider: "claude",
        email: personal.email,
        label: "Personal",
        serverLabel: "Fixture",
      });
      const flow = fixture.ui.claudeFlow;
      if (!flow)
        throw new Error("The account owner did not create a sign-in flow");
      stopReading = autorun(() => {
        void flow.receipt;
      });
      fixture.post.mockImplementationOnce(
        async (
          _path: unknown,
          body: import("../../../api.ts").PostBody<"/api/accounts/claude/login">,
        ) => ({
          requestId: body.login_id,
          accountKey: "personal",
          status: "pending",
        }),
      );
      await flowResult(flow.start());
      await vi.advanceTimersByTimeAsync(100);
      fixture.ui.receive(command(operation, "personal"));
      fixture.areas.scope.services.resources.publish(
        { kind: "auth-progress", requestId: flow.requestId },
        { requestId: flow.requestId, accountKey: "personal", status: "ready" },
      );
      await vi.advanceTimersByTimeAsync(100);
      expect(flow.receipt?.status).toBe("ready");
      expect(fixture.ui.claudeFlow === null).toBe(true);
      expect(
        fixture.post.mock.calls.some(
          ([path]) => path === "/api/accounts/reconnect",
        ),
      ).toBe(false);
    } finally {
      stopReading();
      fixture.ui.dispose();
      stream.close();
      vi.restoreAllMocks();
      vi.useRealTimers();
    }
  },
);
