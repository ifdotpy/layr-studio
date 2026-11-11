import { makeAutoObservable } from "mobx";
import { errorText } from "../../../api.ts";
import { validateServerAlias } from "../../../servers/serverAliases.ts";
import { validateServerName } from "../../../servers/serverNames.ts";

export type ServerDetailsProps = {
  name: string;
  alias: string;
  usedAliases: string[];
  disabled?: boolean;
  saveName?: ((value: string) => Promise<boolean>) | undefined;
  saveAlias: (value: string) => Promise<boolean>;
};

/** Confirmed source changes replace only the matching field's draft. */
class FieldDraft {
  edit = { source: "", value: "", baseline: "", saved: false };
  error = "";
  constructor() {
    makeAutoObservable(this, {}, { autoBind: true });
  }
  value(source: string) {
    return this.edit.source === source ? this.edit.value : source;
  }
  dirty(source: string) {
    return (
      this.edit.source === source && this.edit.value !== this.edit.baseline
    );
  }
  change(source: string, value: string) {
    this.edit = {
      source,
      value,
      baseline: this.edit.source === source ? this.edit.baseline : source,
      saved: false,
    };
    this.error = "";
  }
  confirm(source: string, value: string) {
    this.edit = { source, value, baseline: value, saved: true };
  }
}

export class ServerDetailsForm {
  name = new FieldDraft();
  alias = new FieldDraft();
  busy = false;
  constructor() {
    makeAutoObservable(this, {}, { autoBind: true });
  }
  *save(props: ServerDetailsProps) {
    if (this.busy || props.disabled) return;
    const fields = [
      {
        field: this.name,
        source: props.name,
        save: props.saveName,
        validate: validateServerName,
        unknown: "Check the server name or retry the saved request.",
      },
      {
        field: this.alias,
        source: props.alias,
        save: props.saveAlias,
        validate: (value: string) =>
          validateServerAlias(value, props.usedAliases),
        unknown:
          "The alias result is unknown. Check server settings or retry the saved request.",
      },
    ];
    for (const { field, source, save, validate } of fields) {
      field.error = "";
      if (save && field.dirty(source)) {
        try {
          validate(field.value(source));
        } catch (error) {
          field.error = errorText(error);
        }
      }
    }
    if (fields.some(({ field }) => field.error)) return;
    this.busy = true;
    try {
      for (const { field, source, save, unknown } of fields) {
        if (!save || !field.dirty(source)) continue;
        const value = field.value(source);
        try {
          const saved: boolean = yield save(value);
          if (!saved) {
            field.error = unknown;
            return;
          }
          field.confirm(source, value);
        } catch (error) {
          field.error = errorText(error);
          return;
        }
      }
    } finally {
      this.busy = false;
    }
  }
}
