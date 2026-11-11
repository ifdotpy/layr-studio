import { makeAutoObservable } from "mobx";
import { flowValue } from "../../../flowValue.ts";
import { errorText, type PostBody, type PostResult } from "../../../api.ts";
import type { ServerApi } from "../../../servers/net/api.ts";
import type { Account } from "../../accounts/support/Accounts.tsx";

type Options = NonNullable<PostBody<"/api/claude/profiles">["options"]>;
export function createClaudeProfile(account?: Account) {
  const initial = account?.claudeOptions || {};
  const draft = {
    label: account?.label || "Claude Code",
    binaryPath: initial.binaryPath || "",
    configDir: initial.configDir || "",
    launchArgs: initial.launchArgs || "",
    window: initial.autoCompactWindow || "",
    models: (initial.customModels || [])
      .map(
        (model) =>
          model.id +
          (model.label && model.label !== model.id ? " | " + model.label : ""),
      )
      .join("\n"),
    busy: false,
    error: "",
    saved: false,
    setConfigDir(configDir: string) {
      this.configDir = configDir;
    },
    setWindow(window: string | number) {
      this.window = window;
    },
    changed() {
      this.saved = false;
    },
    *save(
      post: ServerApi["post"],
      account?: Account,
      onSaved?: (result: PostResult<"/api/claude/profiles">) => void,
    ) {
      if (this.busy) return;
      this.busy = true;
      this.error = "";
      this.saved = false;
      try {
        const options: Options = {
          ...account?.claudeOptions,
          binaryPath: this.binaryPath.trim(),
          configDir: this.configDir.trim(),
          launchArgs: this.launchArgs.trim(),
          customModels: this.models
            .split("\n")
            .filter((line) => line.trim())
            .map((line) => {
              const [id = "", ...parts] = line
                .split("|")
                .map((part) => part.trim());
              return { id, label: parts.join(" | ") || id };
            }),
        };
        if (this.window === "") delete options.autoCompactWindow;
        else options.autoCompactWindow = Number(this.window);
        const result = yield* flowValue(
          post("/api/claude/profiles", {
            options,
            ...(account
              ? { account_key: account.id }
              : { label: this.label.trim() }),
          }),
        );
        onSaved?.(result);
        this.saved = true;
      } catch (failure) {
        this.error = errorText(failure);
      } finally {
        this.busy = false;
      }
    },
  };
  return makeAutoObservable(draft, {}, { autoBind: true });
}
