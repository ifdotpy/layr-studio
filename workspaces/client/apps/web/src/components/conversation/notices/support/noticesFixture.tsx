import type { Message } from "../../../../types.ts";
import type { NativeNotice } from "../../../../app/notices/warnings.ts";
import { useState, type ReactNode } from "react";
import * as api from "../../../../servers/net/api.ts";
import {
  conversationActionsFixture,
  observableFixture,
} from "../../../shell/integration-support/observableFixture.tsx";
import { serializeStateCollections } from "../../../../generated/observableResources.ts";
import { useConversationActions } from "../../support/navigation.ts";
import { ServerAreasContextProvider as ConversationProvider } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import type { StudioErrorItem } from "../../../shell/StudioErrorList/StudioErrorList.tsx";
import ConversationWarnings from "../../ConversationWarnings/ConversationWarnings.tsx";

export type WarningsSource = {
  scope: string;
  accountKey: string;
  notices?: NativeNotice[];
  messages: Message[];
  openAgent?: (id: string) => void;
};

/** Static inputs feed the same composition as the real error leaves. */
export function StudioErrorsProvider({
  children,
  items = [],
}: {
  children?: ReactNode;
  items?: StudioErrorItem[];
}) {
  const [areas] = useState(() => {
    const models = observableFixture();
    models.apply(
      items.map((value) => ({
        collection: "studioError" as const,
        id: value.id,
        value,
      })),
    );
    const areas = conversationActionsFixture(() => {}, models.store);
    areas.scope.services.api.get = api.get;
    areas.scope.services.api.post = api.post;
    return areas;
  });
  return <ConversationProvider value={areas}>{children}</ConversationProvider>;
}

export function ConversationWarningsFixture(source: WarningsSource) {
  const parent = useConversationActions();
  const [areas] = useState(() => {
    const models = observableFixture([
      { id: "warnings", accountKey: source.accountKey },
    ]);
    const areas = conversationActionsFixture(
      source.openAgent || parent.open,
      models.store,
      parent.scope.services,
    );
    models.apply([
      ...serializeStateCollections(models.mirror.current),
      ...[...(parent.state?.studioErrors.values() ?? [])].map((value) => ({
        collection: "studioError" as const,
        id: value.id,
        value,
      })),
      {
        collection: "workspace",
        id: "current",
        value: { stateDir: source.scope, nativeNotices: source.notices || [] },
      },
    ]);
    areas.startup.workspaceId = source.scope;
    areas.navigation.selectedId = "warnings";
    areas.selected.history.items = source.messages;
    return areas;
  });
  return (
    <ConversationProvider value={areas}>
      <ConversationWarnings />
    </ConversationProvider>
  );
}
