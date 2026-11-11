import { useServerApi } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { Button, Group, TextInput } from "@mantine/core";
import { Owned } from "../../ui/Owned/Owned.tsx";
import { makeAutoObservable } from "mobx";
import { errorText, type PostResult } from "../../../api.ts";
import { parseGroups } from "../support/accountPreference.ts";
import type { Account } from "../support/Accounts.tsx";

type GroupResult = PostResult<"/api/accounts/group">;

export class AccountGroupDraft {
  text: string | null = null;
  busy = false;
  error = "";
  saved = false;
  constructor() {
    makeAutoObservable(this, {}, { autoBind: true });
  }
  get group() {
    const { groups, error } = parseGroups(this.text || "");
    return {
      name: groups[0] || null,
      error:
        error ||
        (groups.length > 1 ? "Use one group name for an account." : ""),
    };
  }
  setText(value: string) {
    this.text = value;
    this.saved = false;
  }
  *save(
    account: Account,
    post: ReturnType<typeof useServerApi>["post"],
    onSaved: (state: GroupResult) => void,
  ): Generator<Promise<GroupResult>, void, GroupResult> {
    if (
      this.text === null ||
      this.busy ||
      this.group.error ||
      this.group.name === (account.group || null)
    )
      return;
    this.busy = true;
    this.error = "";
    this.saved = false;
    try {
      onSaved(
        yield post("/api/accounts/group", {
          account_key: account.id,
          group: this.group.name,
        }),
      );
      this.saved = true;
    } catch (failure) {
      this.error = errorText(failure);
    } finally {
      this.busy = false;
    }
  }
}

/** The group of one account on one Studio server. */
export default function AccountGroup({
  account,
  serverLabel,
  onSaved,
}: {
  account: Account;
  serverLabel?: string;
  onSaved: (state: GroupResult) => void;
}) {
  const scope = serverLabel ? ` on ${serverLabel}` : "";
  const { post } = useServerApi();
  return (
    <Owned key={account.id} create={() => new AccountGroupDraft()}>
      {(draft) => (
        <form
          className="account-group"
          onSubmit={(event) => {
            event.preventDefault();
            void draft.save(account, post, onSaved);
          }}
        >
          <Group gap="xs" align="flex-end" wrap="nowrap">
            <TextInput
              label={`Group${scope}`}
              aria-label={`Group of ${account.email || account.label}${scope}`}
              description="Project account orders use this name, for example personal."
              placeholder="No group"
              value={draft.text ?? account.group ?? ""}
              maxLength={32}
              disabled={draft.busy}
              error={draft.group.error || undefined}
              onChange={(event) => draft.setText(event.currentTarget.value)}
            />
            {draft.text !== null &&
              draft.group.name !== (account.group || null) && (
                <Button
                  type="submit"
                  variant="light"
                  loading={draft.busy}
                  disabled={!!draft.group.error}
                >
                  Save group
                </Button>
              )}
          </Group>
          {draft.error && (
            <p className="account-action-error" role="alert">
              {draft.error}
            </p>
          )}
          {draft.saved && <p role="status">Group saved.</p>}
        </form>
      )}
    </Owned>
  );
}
