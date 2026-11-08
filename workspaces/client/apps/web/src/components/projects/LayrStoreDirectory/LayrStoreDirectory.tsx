import { Button, Group, Text } from "@mantine/core";
import { useEffect, useState } from "react";
import { errorText } from "../../../api.ts";
import DirectoryField from "../DirectoryField/DirectoryField.tsx";

export function LayrStoreDirectory({
  label,
  path,
  source,
  overridden,
  save,
}: {
  label: string;
  path: string | null;
  source: "project" | "default" | "built-in";
  overridden: boolean;
  save: (path: string | null) => Promise<void>;
}) {
  const [value, setValue] = useState(path ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => setValue(path ?? ""), [path]);
  const change = async (next: string | null) => {
    setBusy(true);
    setError("");
    try {
      await save(next);
    } catch (failure) {
      setError(errorText(failure));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div>
      <DirectoryField
        label={label}
        value={value}
        onChange={setValue}
        disabled={busy}
      />
      <Text size="xs" c="dimmed">
        Source: {source}
      </Text>
      <Group justify="flex-end">
        <Button
          disabled={busy || !value.trim() || value === path}
          onClick={() => void change(value.trim())}
        >
          Save
        </Button>
        <Button
          variant="subtle"
          loading={busy}
          disabled={!overridden}
          onClick={() => void change(null)}
        >
          Reset
        </Button>
      </Group>
      {error && <p role="alert">{error}</p>}
    </div>
  );
}
