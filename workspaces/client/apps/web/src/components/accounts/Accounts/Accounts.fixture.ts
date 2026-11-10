import { vi } from "vitest";
import {
  conversationActionsFixture,
  observableFixture,
} from "../../shell/integration-support/observableFixture.tsx";
import type { AccountsState } from "../support/Accounts.tsx";
export const personal = {
  id: "personal",
  provider: "claude",
  email: "same@example.test",
  label: "Personal",
  status: "ready",
  home: "/fixture/personal",
  source: "fixture",
} as const;
export const team = {
  ...personal,
  id: "team",
  label: "Team",
  home: "/fixture/team",
} as const;
export function accountsFixture() {
  const models = observableFixture();
  const areas = conversationActionsFixture(undefined, models.store);
  const post = vi.fn().mockResolvedValue({});
  areas.scope.services.api.post = post;
  const publish = (value: AccountsState) =>
    models.apply([{ collection: "accounts", id: "current", value }]);
  const summary = {
    accounts: [personal, team],
    archivedAccounts: [],
    defaultAccountKey: personal.id,
    defaultAccountGroups: [],
    supportsDelete: true,
    supportsDisconnect: true,
  };
  publish(summary);
  return {
    areas,
    post,
    summary,
    publish,
    ui: areas.accountsUi,
    close() {
      areas.dispose();
      areas.scope.services.close();
      models.store.dispose();
    },
  };
}
