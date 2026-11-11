import { textField } from "../../../stores/fields.ts";
import ClaudeOrganization from "../../accounts/ClaudeOrganization/ClaudeOrganization.tsx";
import DirectoryField from "../../projects/DirectoryField/DirectoryField.tsx";
import Disclosure from "../../ui/Disclosure/Disclosure.tsx";
import {
  useServerApi,
  useServerOwner,
} from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { Button, NumberInput, Stack, Textarea, TextInput } from "@mantine/core";
import { Owned } from "../../ui/Owned/Owned.tsx";
import { createClaudeProfile } from "./ClaudeProfileStore.ts";
import type { PostResult } from "../../../api.ts";
import type { Account } from "../../accounts/support/Accounts.tsx";

type ClaudeProfileResult = PostResult<"/api/claude/profiles">;

export default function ClaudeProfile({
  account,
  onSaved,
  expanded = false,
}: {
  account?: Account;
  onSaved?: (state: ClaudeProfileResult) => void;
  expanded?: boolean;
}) {
  const { post } = useServerApi();
  const server = useServerOwner();

  return (
    <Owned
      key={JSON.stringify([server?.id, account?.id])}
      create={() => createClaudeProfile(account)}
    >
      {(s) => (
        <Disclosure
          className="account-register claude-profile"
          defaultOpen={expanded}
          summary={
            expanded
              ? "Profile settings"
              : account
                ? "Configure Claude"
                : "Claude"
          }
        >
          <form
            onChange={s.changed}
            onSubmit={(event) => {
              event.preventDefault();
              void s.save(post, account, onSaved);
            }}
          >
            <Stack gap="sm" w="100%">
              <small>
                {account
                  ? "To change these paths, add another profile."
                  : "Sign in to this Claude Code configuration with your subscription before you add it."}
              </small>
              {account && <strong>{account.email || account.label}</strong>}
              <ClaudeOrganization account={account} />
              {!account && (
                <TextInput
                  label="Profile name"
                  {...textField(s, "label")}
                  maxLength={200}
                  required
                  disabled={s.busy}
                />
              )}
              <TextInput
                label="Claude executable"
                placeholder="Installed Claude Code"
                {...textField(s, "binaryPath")}
                readOnly={!!account}
                disabled={s.busy}
              />
              <DirectoryField
                label="Claude configuration directory"
                placeholder="Default Claude configuration"
                value={s.configDir}
                onChange={s.setConfigDir}
                readOnly={!!account}
                disabled={s.busy}
              />
              <fieldset className="claude-advanced">
                <legend>Advanced</legend>
                <NumberInput
                  label="Automatic compaction threshold (tokens)"
                  placeholder="Claude default"
                  value={s.window}
                  onChange={s.setWindow}
                  min={100000}
                  max={1000000}
                  allowDecimal={false}
                  disabled={s.busy}
                />
                <Textarea
                  label="Custom models"
                  description="One model ID per line. Add | and a display name if needed."
                  {...textField(s, "models")}
                  autosize
                  minRows={2}
                  disabled={s.busy}
                />
                <TextInput
                  label="Launch arguments"
                  placeholder="--add-dir /path/to/project"
                  {...textField(s, "launchArgs")}
                  disabled={s.busy}
                />
              </fieldset>
              {s.error && (
                <p className="account-action-error" role="alert">
                  {s.error}
                </p>
              )}
              {s.saved && <p role="status">Claude profile saved.</p>}
              <Button
                variant="filled"
                type="submit"
                loading={s.busy}
                disabled={!s.label.trim() || s.busy}
              >
                {account ? "Save Claude profile" : "Add Claude profile"}
              </Button>
            </Stack>
          </form>
        </Disclosure>
      )}
    </Owned>
  );
}
