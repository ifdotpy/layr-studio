import { whileObserved } from "../../../observed.ts";
import {
  ActionIcon,
  Modal,
  TextInput,
  type TextInputProps,
} from "@mantine/core";
import { FolderOpen } from "lucide-react";
import { Owned } from "../../ui/Owned/Owned.tsx";
import { makeAutoObservable, observableRef } from "mobx";
import { AsyncFeedback } from "../../../stores/AsyncFeedback.ts";
import {
  useServerNet,
  useServerFileDialogs,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import ProjectDirectoryPicker, {
  browseDirectory,
  type DirectoryBrowse,
} from "../ProjectDirectoryPicker/ProjectDirectoryPicker.tsx";

type Props = Omit<
  TextInputProps,
  "value" | "onChange" | "rightSection" | "classNames"
> & {
  label: string;
  value: string;
  onChange: (path: string) => void;
};

function createDirectoryField(
  net: ReturnType<typeof useServerNet>,
  dialogs: ReturnType<typeof useServerFileDialogs>,
) {
  let active = false;
  const initial: { choosing: DirectoryBrowse | null } = { choosing: null };
  const draft = makeAutoObservable(
    {
      ...initial,
      feedback: new AsyncFeedback(),
      close() {
        this.choosing = null;
      },
      choose(value: string, onChange: Props["onChange"]) {
        if (!dialogs.canPickDirectory()) {
          this.choosing = browseDirectory(net, value.trim() || undefined);
          return;
        }
        void this.feedback.run(
          () => dialogs.pickDirectory(),
          (path) => {
            if (path && active) onChange(path);
          },
        );
      },
    },
    { choosing: observableRef },
    { autoBind: true },
  );
  whileObserved(draft, "choosing", () => {
    active = true;
    return () => {
      active = false;
    };
  });
  return draft;
}

/** A folder path field: type the path, or choose it in the server's folder picker. */
export default function DirectoryField({
  label,
  value,
  onChange,
  disabled,
  readOnly,
  ...input
}: Props) {
  const net = useServerNet(),
    dialogs = useServerFileDialogs();
  return (
    <Owned create={() => createDirectoryField(net, dialogs)}>
      {(draft) => (
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
                  onClick={() => void draft.choose(value, onChange)}
                >
                  <FolderOpen size={16} />
                </ActionIcon>
              )
            }
          />
          {draft.feedback.error && <p role="alert">{draft.feedback.error}</p>}
          <Modal
            opened={!!draft.choosing}
            onClose={draft.close}
            title={`Choose ${label.toLowerCase()}`}
          >
            {draft.choosing && (
              <ProjectDirectoryPicker
                initialBrowse={draft.choosing}
                serverChoices={[{ id: "local", label: "This server" }]}
                showServerSelector={false}
                initialPath={value.trim() || undefined}
                submitLabel="Use this folder"
                onSelect={async (path) => {
                  onChange(path);
                  draft.close();
                }}
              />
            )}
          </Modal>
        </>
      )}
    </Owned>
  );
}
