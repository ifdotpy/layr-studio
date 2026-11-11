import { observer } from "mobx-react-lite";
import { providerLabel } from "../../../../generated/providerKind.ts";
import {
  useServerStorage,
  useServerOwner,
} from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { useServerSettings } from "../../../../servers/ServerSettingsContext.tsx";
import { Button, Checkbox, Group, Stack, Text } from "@mantine/core";
import { useState } from "react";
import {
  readPreferenceWriteStatus,
  retryPreferenceWrites,
} from "../../../../sync/uiPreferenceStatus.ts";
import "./model-whitelist-settings.css";
import { useWorkerModels } from "../../../agents/support/WorkerModelPicker.tsx";
import { toModelOptions } from "../support/modelOption.ts";
import { ModelChoice } from "../../../../generated/api.ts";
import {
  isModelEnabled,
  modelChoiceKey,
  readModelChoices,
  writeModelChoices,
  useModelChoices,
} from "../support/modelChoices.ts";

function selectableModels(catalog: ReturnType<typeof useWorkerModels>) {
  const options = toModelOptions(catalog.models.filter((row) => !row.hidden));
  return [
    ...new Map(options.map((model) => [modelChoiceKey(model), model])).values(),
  ];
}

function saveModelSelection(
  storage: Storage,
  keys: string[],
  choice: ModelChoice,
) {
  try {
    const choices = { ...readModelChoices(storage) };
    for (const key of keys) choices[key] = choice;
    writeModelChoices(choices, storage);
    return "";
  } catch (failure) {
    return String(failure);
  }
}

export const ModelWhitelistSettings = observer(
  function ModelWhitelistSettings() {
    const server = useServerOwner();
    const settings = useServerSettings();
    const owner = server || settings?.server;
    const catalog = useWorkerModels("default", true, true);
    const choices = useModelChoices();
    const storage = useServerStorage();
    const status = readPreferenceWriteStatus(storage, "server");
    const [error, setError] = useState("");
    const models = selectableModels(catalog);
    const save = (keys: string[], choice: ModelChoice) =>
      setError(saveModelSelection(storage, keys, choice));
    return (
      <Stack
        gap="sm"
        className="model-whitelist-settings"
        aria-label="Models whitelist"
      >
        <Text fw={600}>Models on {owner?.label || "Local"}</Text>
        <Text size="sm">
          Choose the models shown in this server’s model pickers. Existing chats
          keep their model.
        </Text>
        <Text size="xs" c="dimmed" role="status" aria-live="polite">
          {status.state === "saving"
            ? "Saving model selection…"
            : status.state === "error"
              ? "Model selection is not saved."
              : "Model selection saved on this server."}
        </Text>
        {status.state === "error" && (
          <>
            <Text role="alert" c="red">
              {status.error}
            </Text>
            <Button
              variant="default"
              onClick={() => retryPreferenceWrites(storage)}
            >
              Retry saving
            </Button>
          </>
        )}
        <Group gap="sm">
          {(
            [
              [ModelChoice.whitelisted, "default", "Choose all models"],
              [ModelChoice.blacklisted, "subtle", "Clear selection"],
            ] as const
          ).map(([choice, variant, label]) => (
            <Button
              key={choice}
              variant={variant}
              disabled={!models.length}
              onClick={() => save(models.map(modelChoiceKey), choice)}
            >
              {label}
            </Button>
          ))}
        </Group>
        <Text size="xs" c="dimmed">
          New models appear enabled until you hide them.
        </Text>
        {models.length > 0 &&
          !models.some((model) => isModelEnabled(model, choices)) && (
            <Text size="sm">Choose a model to start new chats.</Text>
          )}
        {catalog.loading && <Text role="status">Loading models…</Text>}
        {(error || catalog.error) && (
          <Text role="alert" c="red">
            {error || catalog.error}
          </Text>
        )}
        {catalog.error && (
          <Button variant="default" onClick={catalog.retry}>
            Retry
          </Button>
        )}
        {[...new Set(models.map((model) => model.provider))].map((provider) => (
          <Stack key={provider} gap="xs" className="model-whitelist-provider">
            <Text fw={600}>{providerLabel(provider)}</Text>
            <div className="model-whitelist-grid">
              {models
                .filter((model) => model.provider === provider)
                .map((model) => {
                  const key = modelChoiceKey(model);
                  return (
                    <Checkbox
                      key={key}
                      label={model.label}
                      checked={isModelEnabled(model, choices)}
                      onChange={(event) =>
                        save(
                          [key],
                          event.currentTarget.checked
                            ? ModelChoice.whitelisted
                            : ModelChoice.blacklisted,
                        )
                      }
                    />
                  );
                })}
            </div>
          </Stack>
        ))}
      </Stack>
    );
  },
);
