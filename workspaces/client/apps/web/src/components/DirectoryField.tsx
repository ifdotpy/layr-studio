import {
  ActionIcon,
  Modal,
  TextInput,
  type TextInputProps,
} from "@mantine/core";
import { FolderOpen } from "lucide-react";
import { useState } from "react";
import { errorText } from "../api";
import { useServerFileDialogs } from "../servers/ServerServicesContext";
import ProjectDirectoryPicker from "./ProjectDirectoryPicker";
import "./directory-field.css";

type Props = Omit<
  TextInputProps,
  "value" | "onChange" | "rightSection" | "classNames"
> & {
  label: string;
  value: string;
  onChange: (path: string) => void;
};

/** A folder path field: type the path, or choose it in the server's folder picker. */
export default function DirectoryField({
  label,
  value,
  onChange,
  disabled,
  readOnly,
  ...input
}: Props) {
  const [choosing, setChoosing] = useState(false);
  const [error, setError] = useState("");
  const { canPickDirectory, pickDirectory } = useServerFileDialogs();
  return (
    <>
      <TextInput
        {...input}
        label={label}
        value={value}
        {...(disabled !== undefined ? { disabled } : {})}
        {...(readOnly !== undefined ? { readOnly } : {})}
        classNames={{ input: "directory-field-input" }}
        onChange={(event) => onChange(event.currentTarget.value)}
        rightSection={
          readOnly ? undefined : (
            <ActionIcon
              variant="subtle"
              aria-label={`Choose ${label.toLowerCase()}`}
              title="Choose a folder"
              disabled={!!disabled}
              onClick={() => {
                if (!canPickDirectory()) {
                  setChoosing(true);
                  return;
                }
                setError("");
                void pickDirectory()
                  .then((path) => {
                    if (path) onChange(path);
                  })
                  .catch((failure) => setError(errorText(failure)));
              }}
            >
              <FolderOpen size={16} />
            </ActionIcon>
          )
        }
      />
      {error && <p role="alert">{error}</p>}
      <Modal
        opened={choosing}
        onClose={() => setChoosing(false)}
        title={`Choose ${label.toLowerCase()}`}
      >
        {choosing && (
          <ProjectDirectoryPicker
            showServerSelector={false}
            initialPath={value.trim() || undefined}
            submitLabel="Use this folder"
            onSelect={async (path) => {
              onChange(path);
              setChoosing(false);
            }}
          />
        )}
      </Modal>
    </>
  );
}
