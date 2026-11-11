import { observer } from "mobx-react-lite";
import { Button, Group, Modal, Stack, TextInput } from "@mantine/core";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";
import type { Account } from "../support/Accounts.tsx";
import { ClaudeAuthorizationCodeForm } from "../ClaudeAuthorizationCodeForm/ClaudeAuthorizationCodeForm.tsx";

/** New-profile and saved-profile Claude sign-ins share one concrete subscription flow. */
export default observer(function ClaudeSignIn({
  account,
  email = account?.email || "",
  serverLabel,
  opened = true,
  onClose,
}: {
  account?: Account | undefined;
  scope?: string | undefined;
  label?: string;
  email?: string;
  serverLabel?: string | undefined;
  opened?: boolean;
  onClose: () => void;
  onReady?: () => unknown;
}) {
  const flow = useServerAreas().accountsUi.claudeFlow;
  if (!flow) return null;
  const { receipt, busy, requestId, url } = flow;
  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={`Sign in to Claude · ${flow.displayLabel}${account && serverLabel ? ` · ${serverLabel}` : ""}`}
      centered
    >
      <Stack gap="sm">
        {serverLabel &&
          (account ? (
            <p>Server: {serverLabel}</p>
          ) : (
            <TextInput label="Server" value={serverLabel} readOnly />
          ))}
        {account ? (
          <p>
            Use your Claude subscription for{" "}
            <strong>{account.email || account.label}</strong>.
          </p>
        ) : (
          flow.displayEmail && <p>Sign in as {flow.displayEmail}.</p>
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
                Claude returned an unsupported sign-in{" "}
                {account ? "URL" : "link"}.
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
              {(!requestId || (flow.terminal && !account)) && (
                <Button
                  onClick={() => void flow.start(flow.displayEmail)}
                  loading={busy === "start"}
                  disabled={!!busy}
                >
                  {receipt ? "Start again" : "Start sign-in"}
                </Button>
              )}
              {flow.pending && (
                <Button
                  variant="default"
                  disabled={!!busy}
                  onClick={flow.check}
                >
                  Check sign-in status
                </Button>
              )}
              {requestId && !receipt && !flow.codeSubmissionUncertain && (
                <Button
                  onClick={() => void flow.start(flow.displayEmail)}
                  disabled={!!busy}
                  loading={busy === "start"}
                >
                  Retry sign-in
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
              {flow.unknownTooLong && !flow.codeSubmissionUncertain && (
                <Button
                  variant="default"
                  disabled={!!busy}
                  onClick={() => void flow.start(flow.displayEmail, true)}
                >
                  Start again
                </Button>
              )}
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
                disabled={!!busy}
                onClick={() => void flow.start(receipt.email || "", true)}
              >
                Keep {receipt.email}
              </Button>
            )}
          </>
        )}
        {flow.displayError && <p role="alert">{flow.displayError}</p>}
        {account && flow.terminal && (
          <Button disabled={!!busy} onClick={() => void flow.start()}>
            Start new sign-in
          </Button>
        )}
        <Group justify="flex-end">
          <Button variant="subtle" onClick={onClose}>
            Close
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
});
