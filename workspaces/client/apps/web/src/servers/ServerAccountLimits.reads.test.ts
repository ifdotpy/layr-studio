import { AccountStatus, ProviderKind } from "../generated/api.ts";
import { expect, it } from "vitest";
import { observableFixture } from "../components/observableFixture.tsx";
import { serverAccounts } from "./ServerAccountLimits.tsx";

function source(label: string) {
  const fixture = observableFixture();
  fixture.apply([
    {
      collection: "accounts",
      id: "current",
      value: {
        accounts: [
          {
            id: "same",
            label,
            provider: ProviderKind["codex"],
            status: AccountStatus["ready"],
            home: "",
            source: "fixture",
          },
        ],
        archivedAccounts: [],
        defaultAccountKey: "same",
        defaultAccountGroups: [],
        supportsDelete: true,
        supportsDisconnect: true,
      },
    },
  ]);
  return fixture;
}
it("an unavailable server cannot fall through to the local account owner", () => {
  const local = source("Local"),
    remote = source("Remote");

  try {
    expect(serverAccounts(local.store, "remote", () => undefined)).toEqual([]);
    expect(serverAccounts(local.store, "local")[0]?.label).toBe("Local");
    expect(
      serverAccounts(local.store, "remote", () => ({ store: remote.store }))[0]
        ?.label,
    ).toBe("Remote");
  } finally {
    local.store.dispose();
    remote.store.dispose();
  }
});
it("the same selector can replace credentials without retaining the previous account references", () => {
  const old = source("Old"),
    next = source("New");
  let current = { store: old.store };
  const select = () => current;

  try {
    const first = serverAccounts(old.store, "remote", select)[0];
    current = { store: next.store };
    const replacement = serverAccounts(old.store, "remote", select)[0];
    expect(replacement?.label).toBe("New");
    expect(replacement).not.toBe(first);
    expect(serverAccounts(old.store, "remote", select)[0]).toBe(replacement);
  } finally {
    old.store.dispose();
    next.store.dispose();
  }
});
