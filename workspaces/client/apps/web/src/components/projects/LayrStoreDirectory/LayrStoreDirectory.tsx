import { Button, Group, Text } from "@mantine/core";
import { Owned } from "../../ui/Owned/Owned.tsx";
import { makeAutoObservable } from "mobx";
import { AsyncFeedback } from "../../../stores/AsyncFeedback.ts";
import DirectoryField from "../DirectoryField/DirectoryField.tsx";

type Props = {
  label: string;
  path: string | null;
  source: "project" | "default" | "built-in";
  overridden: boolean;
  save: (path: string | null) => Promise<void>;
};

function createDirectoryDraft(value: string) {
  return makeAutoObservable(
    {
      value,
      feedback: new AsyncFeedback(),
      setValue(value: string) {
        this.value = value;
      },
    },
    {},
    { autoBind: true },
  );
}
export function LayrStoreDirectory({
  label,
  path,
  source,
  overridden,
  save,
}: Props) {
  return (
    <Owned key={path} create={() => createDirectoryDraft(path ?? "")}>
      {({ value, feedback, setValue }) => (
        <div>
          <DirectoryField
            label={label}
            value={value}
            onChange={setValue}
            disabled={feedback.pending}
          />
          <Text size="xs" c="dimmed">
            Source: {source}
          </Text>
          <Group justify="flex-end">
            <Button
              disabled={feedback.pending || !value.trim() || value === path}
              onClick={() => void feedback.run(() => save(value.trim()))}
            >
              Save
            </Button>
            <Button
              variant="subtle"
              loading={feedback.pending}
              disabled={!overridden}
              onClick={() => void feedback.run(() => save(null))}
            >
              Reset
            </Button>
          </Group>
          {feedback.error && <p role="alert">{feedback.error}</p>}
        </div>
      )}
    </Owned>
  );
}
