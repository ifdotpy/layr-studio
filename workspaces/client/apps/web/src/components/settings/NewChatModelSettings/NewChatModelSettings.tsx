import { ServerAreasContextProvider } from "../../../app/ServerAreasProvider.tsx";
import { observer } from "mobx-react-lite";
import { Button, Stack } from "@mantine/core";
import { ServerServicesProvider } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { ExecutionSettings } from "../../agents/ExecutionSettings/ExecutionSettings.tsx";
import type { NewChatModelStore } from "./NewChatModelStore.ts";

export default observer(function NewChatModelSettings({
  model,
}: {
  model: NewChatModelStore;
}) {
  const { accounts, catalog, agent, error, modelError } = model.view;
  return (
    <ServerServicesProvider services={model.services}>
      <ServerAreasContextProvider value={model.areas}>
        <Stack
          gap="lg"
          className="new-chat-model-settings"
          aria-label="Agent settings"
        >
          <ExecutionSettings
            agentsUi={model.agentsUi}
            input={{ kind: "draft", draft: agent }}
            catalog={catalog}
            accounts={accounts?.accounts || []}
            presentation="panel"
            settingsRow
            showAccountSummary
            workerExcludedAccountKeys={model.workerExcludedAccountKeys}
            reviewerExcludedAccountKeys={model.reviewerExcludedAccountKeys}
            onWorkerExcludedAccountKeysChange={model.setWorkerExcluded}
            onReviewerExcludedAccountKeysChange={model.setReviewerExcluded}
            onDraftChange={model.patch}
            onAccountChange={model.setAccountKey}
          />
          {(error || modelError) && (
            <div role="alert">
              {error || modelError}
              <Button size="xs" onClick={() => void model.retry()}>
                Retry
              </Button>
            </div>
          )}
        </Stack>
      </ServerAreasContextProvider>
    </ServerServicesProvider>
  );
});

export { NewChatModelStore } from "./NewChatModelStore.ts";
