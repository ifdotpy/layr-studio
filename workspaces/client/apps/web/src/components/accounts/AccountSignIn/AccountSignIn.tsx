import { observer } from "mobx-react-lite";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";
import { now } from "../../../clock.ts";
import ErrorDescription from "../../shell/ErrorDescription/ErrorDescription.tsx";
import { Button } from "@mantine/core";
import { Check, Copy, ExternalLink, Plus, RefreshCw, X } from "lucide-react";
import type { Account } from "../support/Accounts.tsx";
import type { PostResult } from "../../../api.ts";
export type LoginReceipt = PostResult<"/api/accounts/login">;
export function deviceCodeCountdownSeconds(
  expiresAt: unknown,
  now: number,
): number | null {
  if (
    typeof expiresAt !== "number" ||
    !Number.isFinite(expiresAt) ||
    expiresAt <= 0 ||
    !Number.isFinite(now)
  )
    return null;
  return Math.max(0, Math.ceil(expiresAt - now));
}

export default observer(function AccountSignIn({
  targetAccount,
}: {
  scope?: string | undefined;
  targetAccount?: Account;
  emailHint?: string;
  label?: string;
  onConnected?: (account: Account) => void;
}) {
  const flow = useServerAreas().accountsUi.codexFlow;
  if (!flow) return null;
  const { receipt, busy, error, copied, expectedEmail, account, url } = flow;
  const connected = ["ready", "duplicate"].includes(receipt?.status || "");
  const expirySeconds = deviceCodeCountdownSeconds(
    receipt?.expiresAt,
    now() / 1000,
  );
  return (
    <section
      className="account-add"
      aria-label={targetAccount ? "Restore account sign-in" : "Add an account"}
    >
      <div className="account-add-heading">
        <div>
          <strong>{targetAccount ? "Sign in again" : "Codex"}</strong>
          <p>
            {targetAccount
              ? `Use ${targetAccount.email || targetAccount.label}. Your chats keep this account.`
              : expectedEmail
                ? `Studio checks that you sign in as ${expectedEmail}.`
                : "Use a separate login and account limits."}
          </p>
        </div>
        <Button
          variant="filled"
          leftSection={<Plus size={15} />}
          loading={busy === "start"}
          disabled={
            !!busy || ["starting", "pending"].includes(receipt?.status || "")
          }
          onClick={() => void flow.run("start")}
        >
          {targetAccount ? "Start sign-in" : "Sign in"}
        </Button>
      </div>
      {receipt && (
        <section className="account-login" aria-label="Account sign-in">
          {connected ? (
            <p role="status">
              <Check size={16} />{" "}
              {account?.disconnected
                ? "This account is saved but disconnected. Reconnect it in Accounts to use it for new chats."
                : targetAccount
                  ? "Sign-in restored. Send a new instruction to continue."
                  : receipt.status === "duplicate"
                    ? "This account is already connected."
                    : "Account connected."}{" "}
              {account?.email}
            </p>
          ) : receipt.status === "cancelled" ? (
            <p role="status">Sign-in cancelled.</p>
          ) : receipt.status === "error" ? (
            <ErrorDescription
              value={receipt.error || "Sign-in failed. Try again."}
            />
          ) : (
            <>
              {receipt.userCode && url ? (
                <>
                  <strong>Complete sign-in in your browser</strong>
                  <p>
                    {targetAccount
                      ? `Use ${targetAccount.email || targetAccount.label}, then enter this code.`
                      : "Use the new account, then enter this code."}
                  </p>
                  <div className="account-login-code">
                    <code>{receipt.userCode}</code>
                    <Button
                      size="compact-xs"
                      variant="subtle"
                      leftSection={<Copy size={13} />}
                      onClick={() => void flow.run("copy")}
                    >
                      {copied ? "Copied" : "Copy code"}
                    </Button>
                  </div>
                  <Button
                    variant="filled"
                    component="a"
                    href={url}
                    target="_blank"
                    rel="noopener noreferrer"
                    size="compact-sm"
                    rightSection={<ExternalLink size={13} />}
                  >
                    Open sign-in page
                  </Button>
                  <small role="status">Waiting for sign-in…</small>
                  {expirySeconds !== null && (
                    <small role="timer" aria-label="Code expiry countdown">
                      {expirySeconds === 0
                        ? "Code expired"
                        : `Code expires in ${Math.floor(expirySeconds / 60)}:${String(expirySeconds % 60).padStart(2, "0")}`}
                    </small>
                  )}
                </>
              ) : (
                <p role="status">
                  {receipt.status === "starting"
                    ? "Starting sign-in…"
                    : "The sign-in response is unconfirmed."}
                </p>
              )}
              {Boolean(receipt.error) && (
                <ErrorDescription value={receipt.error} />
              )}
              <div className="account-login-actions">
                <Button
                  size="compact-xs"
                  variant="subtle"
                  leftSection={<RefreshCw size={13} />}
                  disabled={!!busy}
                  onClick={() => void flow.run("check")}
                >
                  Check status
                </Button>
                {receipt.loginId && (
                  <Button
                    size="compact-xs"
                    variant="subtle"
                    leftSection={<X size={13} />}
                    loading={busy === "cancel"}
                    disabled={!!busy}
                    onClick={() => void flow.run("cancel")}
                  >
                    Cancel sign-in
                  </Button>
                )}
              </div>
            </>
          )}
        </section>
      )}
      {error && (
        <p className="account-action-error" role="alert">
          {error}
        </p>
      )}
      {!targetAccount &&
        receipt?.status === "error" &&
        receipt.error?.includes("different account") &&
        receipt.email && (
          <div className="account-wrong-identity" role="alert">
            <p>
              You signed in as {receipt.email}. This account expects{" "}
              {expectedEmail}. Studio did not save the sign-in.
            </p>
            <Button
              variant="default"
              disabled={!!busy}
              onClick={() => void flow.run("start", receipt.email!)}
            >
              Keep {receipt.email}
            </Button>
          </div>
        )}
    </section>
  );
});
