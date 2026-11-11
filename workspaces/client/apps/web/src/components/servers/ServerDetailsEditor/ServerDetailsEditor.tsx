import Disclosure from "../../ui/Disclosure/Disclosure.tsx";
import { useRef, useState } from "react";
import { TextInput } from "@mantine/core";
import { ActionButton } from "../../ui/ActionButton/ActionButton.tsx";
import { validateServerName } from "../../../servers/serverNames.ts";
import { validateServerAlias } from "../../../servers/serverAliases.ts";
import { errorText } from "../../../api.ts";

/** Edits share a form; each field keeps its own confirmed value and save owner. */
export default function ServerDetailsEditor({
  name,
  alias,
  usedAliases,
  disabled,
  saveName,
  saveAlias,
}: {
  name: string;
  alias: string;
  usedAliases: string[];
  disabled?: boolean;
  saveName?: ((value: string) => Promise<boolean>) | undefined;
  saveAlias: (value: string) => Promise<boolean>;
}) {
  const nameField = useField(name),
    aliasField = useField(alias);
  const label = nameField.value,
    shortName = aliasField.value;
  const [nameError, setNameError] = useState("");
  const [aliasError, setAliasError] = useState("");
  const [busy, setBusy] = useState(false);
  const lock = useRef(false);
  const nameDirty = !!saveName && nameField.dirty;
  const aliasDirty = aliasField.dirty;
  const save = async () => {
    if (lock.current || disabled || (!nameDirty && !aliasDirty)) return;
    let invalid = false;
    setNameError("");
    setAliasError("");
    if (nameDirty) {
      try {
        validateServerName(label);
      } catch (error) {
        setNameError(errorText(error));
        invalid = true;
      }
    }
    if (aliasDirty) {
      try {
        validateServerAlias(shortName, usedAliases);
      } catch (error) {
        setAliasError(errorText(error));
        invalid = true;
      }
    }
    if (invalid) return;
    lock.current = true;
    setBusy(true);
    try {
      if (nameDirty && saveName) {
        try {
          if (!(await saveName(label))) {
            setNameError("Check the server name or retry the saved request.");
            return;
          }
          nameField.confirm();
        } catch (error) {
          setNameError(errorText(error));
          return;
        }
      }
      if (aliasDirty) {
        try {
          if (!(await saveAlias(shortName))) {
            setAliasError(
              "The alias result is unknown. Check server settings or retry the saved request.",
            );
            return;
          }
          aliasField.confirm();
        } catch (error) {
          setAliasError(errorText(error));
        }
      }
    } finally {
      lock.current = false;
      setBusy(false);
    }
  };
  return (
    <Disclosure className="server-settings-edit" summary="Edit">
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <div className="server-settings-edit-fields">
          {saveName && (
            <div>
              <TextInput
                label="Server name"
                description="Use 1 to 80 visible characters."
                value={label}
                error={nameError}
                disabled={disabled || busy}
                autoComplete="off"
                onChange={(event) => {
                  nameField.change(event.currentTarget.value);
                  setNameError("");
                }}
              />
              {nameField.saved && !nameDirty && (
                <small role="status">Name saved.</small>
              )}
            </div>
          )}
          <div>
            <TextInput
              label="Server alias"
              description="Use 1 to 3 uppercase letters."
              value={shortName}
              error={aliasError}
              disabled={disabled || busy}
              autoComplete="off"
              onChange={(event) => {
                aliasField.change(event.currentTarget.value);
                setAliasError("");
              }}
            />
            {aliasField.saved && !aliasDirty && (
              <small role="status">Alias saved.</small>
            )}
          </div>
        </div>
        <ActionButton
          type="submit"
          actionRole="secondary"
          loading={busy}
          disabled={disabled || (!nameDirty && !aliasDirty)}
        >
          Save
        </ActionButton>
      </form>
    </Disclosure>
  );
}

/** A new confirmed source replaces only its field; another field keeps its draft. */
function useField(source: string) {
  const [edit, set] = useState({ source, value: source, saved: false });
  const value = edit.source === source ? edit.value : source;
  return {
    value,
    dirty: edit.source === source && !edit.saved && value !== source,
    saved: edit.saved,
    change: (value: string) => set({ source, value, saved: false }),
    confirm: () => set({ source, value, saved: true }),
  };
}
