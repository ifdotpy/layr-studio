import SignInDialog, { type SignInProps } from "../CodexSignIn/CodexSignIn.tsx";
import { observer } from "mobx-react-lite";
import { Button, Group } from "@mantine/core";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";
import { ClaudeAuthorizationCodeForm } from "../ClaudeAuthorizationCodeForm/ClaudeAuthorizationCodeForm.tsx";

/** New-profile and saved-profile Claude sign-ins share one concrete subscription flow. */
export default observer(function ClaudeSignIn({
  account,
  email = account?.email || "",
  serverLabel,
  opened = true,
  onClose,
}: SignInProps) {
  const flow = useServerAreas().accountsUi.claudeFlow;
  if (!flow) return null;
  const { receipt, busy, url } = flow;
  const startButton = (
    label: string,
    email = flow.displayEmail,
    forceNew = false,
    loading = false,
  ) => (
    <Button
      disabled={!!busy || !flow.mutationReady}
      loading={loading && busy === "start"}
      onClick={() => void flow.start(email, forceNew)}
    >
      {label}
    </Button>
  );
  return (
    <SignInDialog
      provider="Claude"
      label={flow.displayLabel}
      account={account}
      opened={opened}
      onClose={onClose}
      serverLabel={serverLabel}
    >
      {account ? (
        <p>
          Use your Claude subscription for{" "}
          <strong>{account.email || account.label}</strong>.
        </p>
      ) : (
        flow.displayEmail && <p>Sign in as {flow.displayEmail}.</p>
      )}
      {!flow.mutationReady && (
        <p role="status">
          Syncing. Sign-in will be available when sync finishes.
        </p>
      )}
      {receipt?.status === "ready" ? (
        <p role="status">
          {account
            ? `Signed in${receipt.email ? ` as ${receipt.email}` : ""}. You can retry the message.`
            : `Signed in as ${receipt.email || email || "Claude account"}.`}
        </p>
      ) : (
        <>
          {receipt?.status === "cancelled" && (
            <p role="status">Sign-in cancelled.</p>
          )}
          {url && (
            <Group>
              <Button
                component="a"
                href={url}
                target="_blank"
                rel="noopener noreferrer"
              >
                {account ? "Open Claude sign-in" : "Open sign-in page"}
              </Button>
              <Button variant="default" disabled={!!busy} onClick={flow.copy}>
                {flow.copied ? "Link copied" : "Copy link"}
              </Button>
            </Group>
          )}
          {receipt?.verificationUrl && !url && (
            <p role="alert">
              Claude returned an unsupported sign-in {account ? "URL" : "link"}.
            </p>
          )}
          {flow.statusMessage && <p role="status">{flow.statusMessage}</p>}
          {flow.canSubmitCode && (
            <ClaudeAuthorizationCodeForm
              value={flow.code}
              onChange={flow.setCode}
              onSubmit={() => void flow.run("code")}
              disabled={!!busy}
              loading={busy === "code"}
              submitLabel="Submit authorization response"
            />
          )}
          <Group>
            {flow.startLabel &&
              startButton(flow.startLabel, flow.displayEmail, false, true)}
            {flow.pending && (
              <Button variant="default" disabled={!!busy} onClick={flow.check}>
                Check sign-in status
              </Button>
            )}
            {flow.pending && (
              <Button
                variant="subtle"
                disabled={!!busy}
                loading={busy === "cancel"}
                onClick={() => void flow.run("cancel")}
              >
                {account ? "Cancel sign-in" : "Cancel"}
              </Button>
            )}
            {flow.unknownTooLong &&
              !flow.codeSubmissionUncertain &&
              startButton("Start again", flow.displayEmail, true)}
          </Group>
          {flow.unknownTooLong && !flow.codeSubmissionUncertain && (
            <p role="status">
              The server has not found this request. Retry it, start again, or
              cancel.
            </p>
          )}
          {!account && flow.wrong && receipt?.email && (
            <Button
              variant="default"
              disabled={!!busy || !flow.mutationReady}
              onClick={() => void flow.start(receipt.email || "", true)}
            >
              Keep {receipt.email}
            </Button>
          )}
        </>
      )}
      {flow.displayError && <p role="alert">{flow.displayError}</p>}
      {account &&
        flow.terminal &&
        startButton("Start new sign-in", flow.expectedEmail)}
      <Group justify="flex-end">
        <Button variant="subtle" onClick={onClose}>
          Close
        </Button>
      </Group>
    </SignInDialog>
  );
});
