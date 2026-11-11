import { TextInput } from "@mantine/core";
import { ActionButton } from "../../ui/ActionButton/ActionButton.tsx";
import Disclosure from "../../ui/Disclosure/Disclosure.tsx";
import { Owned } from "../../ui/Owned/Owned.tsx";
import {
  ServerDetailsForm,
  type ServerDetailsProps,
} from "./ServerDetailsForm.ts";

export default function ServerDetailsEditor(props: ServerDetailsProps) {
  return (
    <Owned create={() => new ServerDetailsForm()}>
      {(form) => {
        const { disabled, saveName, name, alias } = props;
        const nameDirty = !!saveName && form.name.dirty(name);
        const aliasDirty = form.alias.dirty(alias);
        return (
          <Disclosure className="server-settings-edit" summary="Edit">
            <form
              onSubmit={(event) => {
                event.preventDefault();
                void form.save(props);
              }}
            >
              <div className="server-settings-edit-fields">
                {[...(saveName ? ["name" as const] : []), "alias" as const].map(
                  (key) => {
                    const field = form[key];
                    const source = props[key];
                    const label = key === "name" ? "Name" : "Alias";
                    return (
                      <div key={key}>
                        <TextInput
                          label={`Server ${key}`}
                          description={
                            key === "name"
                              ? "Use 1 to 80 visible characters."
                              : "Use 1 to 3 uppercase letters."
                          }
                          value={field.value(source)}
                          error={field.error}
                          disabled={disabled || form.busy}
                          autoComplete="off"
                          onChange={(event) =>
                            field.change(source, event.currentTarget.value)
                          }
                        />
                        {field.edit.saved && !field.dirty(source) && (
                          <small role="status">{label} saved.</small>
                        )}
                      </div>
                    );
                  },
                )}
              </div>
              <ActionButton
                type="submit"
                actionRole="secondary"
                loading={form.busy}
                disabled={disabled || (!nameDirty && !aliasDirty)}
              >
                Save
              </ActionButton>
            </form>
          </Disclosure>
        );
      }}
    </Owned>
  );
}
