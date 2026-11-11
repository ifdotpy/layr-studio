import { flowResult } from "mobx";
import { expect, test, vi } from "vitest";
import { ProviderKind } from "../../../generated/api.ts";
import type { PostResult } from "../../../api.ts";
import { OBSERVABLE_INTEREST_DELAY_MS } from "../../../store/interest.ts";
import { transientTestFixture } from "../../../sync/subscriptions/transientTestFixture.ts";
import { accountsFixture, personal } from "../Accounts/Accounts.fixture.ts";

function authFixture() {
  const owner = accountsFixture(),
    stream = transientTestFixture();
  const { services, store } = owner.areas.scope;
  services.store.state = store.state;
  services.store.authProgress = stream.store.authProgress;
  services.resources.publish = stream.subscriptions.publish;
  return {
    ...owner,
    close() {
      owner.close();
      stream.close();
    },
  };
}

test.each([false, true])(
  "Claude restoration uses the current disconnected flag after starting with %s",
  async (disconnected) => {
    vi.useFakeTimers();
    const fixture = authFixture();
    const account = { ...personal, disconnected };
    try {
      fixture.publish({ ...fixture.summary, accounts: [account] });
      fixture.ui.receive({
        action: "account-sign-in",
        accountKey: account.id,
        provider: ProviderKind.claude,
        email: null,
        label: account.label,
        serverLabel: "Fixture",
      });
      const flow = fixture.ui.claudeFlow;
      if (!flow) throw new Error("Missing saved Claude flow");
      fixture.post.mockImplementationOnce(
        async () =>
          ({
            requestId: flow.requestId,
            accountKey: account.id,
            status: "pending",
          }) satisfies PostResult<"/api/accounts/claude/login">,
      );
      await flowResult(flow.start());
      await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
      fixture.publish({
        ...fixture.summary,
        accounts: [{ ...account, disconnected: !disconnected }],
      });
      fixture.post.mockClear();
      fixture.areas.scope.services.resources.publish(
        { kind: "auth-progress", requestId: flow.requestId },
        { requestId: flow.requestId, accountKey: account.id, status: "ready" },
      );
      await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
      expect(flow.receipt?.status).toBe("ready");
      if (!disconnected)
        expect(fixture.post).toHaveBeenCalledWith("/api/accounts/reconnect", {
          account_key: account.id,
        });
      else expect(fixture.post).not.toHaveBeenCalled();
    } finally {
      fixture.close();
      vi.useRealTimers();
    }
  },
);

test.each([false, true])(
  "Codex duplicate completion only reconnects saved restoration=%s",
  async (restoring) => {
    vi.useFakeTimers();
    const fixture = authFixture();
    const account = {
      ...personal,
      provider: ProviderKind.codex,
      disconnected: true,
    };
    try {
      fixture.publish({ ...fixture.summary, accounts: [account] });
      if (restoring)
        fixture.ui.receive({
          action: "account-sign-in",
          accountKey: account.id,
          provider: ProviderKind.codex,
          email: null,
          label: account.label,
          serverLabel: "Fixture",
        });
      else fixture.ui.openAdd({ provider: ProviderKind.codex });
      const flow = fixture.ui.codexFlow;
      if (!flow) throw new Error("Missing Codex flow");
      flow.remember("codex-login");
      await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
      flow.accept({
        requestId: flow.requestId,
        accountKey: account.id,
        resolvedAccountKey: account.id,
        status: "duplicate",
      });
      await vi.advanceTimersByTimeAsync(OBSERVABLE_INTEREST_DELAY_MS);
      expect(flow.receipt?.status).toBe("duplicate");
      const reconnects = fixture.post.mock.calls.filter(
        ([path]) => path === "/api/accounts/reconnect",
      );
      expect(reconnects).toHaveLength(restoring ? 1 : 0);
    } finally {
      fixture.close();
      vi.useRealTimers();
    }
  },
);
