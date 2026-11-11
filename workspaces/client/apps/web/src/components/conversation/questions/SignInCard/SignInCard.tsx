import { resourceData } from "../../../../store/runtime.ts";
import { useServerStore } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { useRef } from "react";
import { Button } from "@mantine/core";
import { KeyRound } from "lucide-react";
import { observer } from "mobx-react-lite";
import type { RequestView } from "../RequestArea/RequestArea.tsx";
import { useServerServices } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { canForwardSignIn } from "../support/openSignIn.ts";
import {
  SIGN_IN_FLOW_LABEL,
  SIGN_IN_STATE_LABEL,
  acceptBody,
  currentSignInWindow,
  pasteBody,
  portInUse,
  resultTarget,
  signInDeadline,
  signInErrorText,
  signInFlow,
  signInState,
  timeLeftLabel,
  verificationPage,
  waitingChatsLabel,
} from "../support/signInCardModel.ts";

import { now } from "../../../../clock.ts";

/** A sign-in request of an agent. The link opens in
 * the system browser after the approval; a window that cannot hold the redirect port takes the
 * address of the page that did not load and sends it to the command. */
export default observer(function SignInCard({ view }: { view: RequestView }) {
  const details = view.browserOpen!;
  const { post, notify } = view;
  const sending = view.disabled,
    error = view.error;
  const services = useServerServices();
  const serverName = services?.owner.alias || services?.owner.label;
  const store = useServerStore();
  const agents = resourceData(store.state().current)?.agents;
  const flow = signInFlow(details);
  const state = signInState(details, sending);
  const timeLeft = timeLeftLabel(signInDeadline(details), now());
  const waiting = waitingChatsLabel(details.joined, {
    get: (id) => agents?.get(id)?.name || "",
  });
  // The pasted address stays in this request view only, never in drafts or storage.
  const address = view.signInAddress;
  const copied = view.signInCopy.copied();
  const field = useRef<HTMLInputElement>(null);
  const codeText = useRef<HTMLElement>(null);
  const open = (paste = false) =>
    void post(acceptBody(currentSignInWindow(canForwardSignIn()), paste));
  const copyCode = async () => {
    try {
      await view.signInCopy.copy(details.userCode || "");
      if (view.signInCopy.failed) throw new Error("Copy failed");
    } catch {
      const node = codeText.current;
      const selection = window.getSelection();
      if (node && selection) {
        const range = document.createRange();
        range.selectNodeContents(node);
        selection.removeAllRanges();
        selection.addRange(range);
      }
      notify("Copy the selected code.");
    }
  };
  const pasteFromClipboard = async () => {
    if (!(await view.pasteSignIn())) field.current?.focus();
  };
  const send = (event: React.FormEvent) => {
    event.preventDefault();
    void view.sendSignIn();
  };
  const errorBlock = error ? (
    <p className="sign-in-card-error" role="alert">
      {signInErrorText(error)}
    </p>
  ) : null;
  return (
    <section
      className="sign-in-card"
      data-sign-in-state={state}
      aria-label="Sign-in request"
    >
      <header className="sign-in-card-head">
        <KeyRound size={16} className="sign-in-card-icon" />
        <strong>Sign-in request</strong>
        <span className="sign-in-card-chat">{view.owner?.name || "Codex"}</span>
        <span className="sign-in-card-state">{SIGN_IN_STATE_LABEL[state]}</span>
      </header>
      <p className="sign-in-card-host">{details.host}</p>
      {waiting && <p className="sign-in-card-joined">{waiting}</p>}
      <dl className="sign-in-card-rows">
        <div>
          <dt>Flow</dt>
          <dd>{SIGN_IN_FLOW_LABEL[flow]}</dd>
        </div>
        {details.clientId && (
          <div>
            <dt>Client</dt>
            <dd title={details.clientId}>{details.clientId}</dd>
          </div>
        )}
        {details.scope && (
          <div>
            <dt>Access</dt>
            <dd title={details.scope}>{details.scope}</dd>
          </div>
        )}
        <div>
          <dt>Result goes to</dt>
          <dd>{resultTarget(details, serverName)}</dd>
        </div>
        {timeLeft && (
          <div>
            <dt>Time left</dt>
            <dd>{timeLeft}</dd>
          </div>
        )}
      </dl>
      {flow === "device" && (
        <div className="sign-in-card-device">
          <span>Code</span>
          <code ref={codeText} className="sign-in-card-code">
            {details.userCode}
          </code>
          <p>Enter it on {verificationPage(details.url)}</p>
        </div>
      )}
      <details className="sign-in-card-link">
        <summary>Full link</summary>
        <code>{details.url}</code>
      </details>
      {state === "approval" ? (
        <>
          {errorBlock}
          <div className="sign-in-card-actions">
            {flow === "device" && (
              <Button
                variant="default"
                disabled={sending}
                onClick={() => void copyCode()}
              >
                {copied ? "Code copied" : "Copy code"}
              </Button>
            )}
            {portInUse(error) && (
              <Button
                variant="default"
                disabled={sending}
                onClick={() => open(true)}
              >
                Paste instead
              </Button>
            )}
            <Button
              variant="default"
              disabled={sending}
              onClick={() => void post({ decision: "decline" })}
            >
              Decline
            </Button>
            <Button variant="filled" disabled={sending} onClick={() => open()}>
              {flow === "device" ? "Open page" : "Open in browser"}
            </Button>
          </div>
        </>
      ) : (
        <form className="sign-in-card-paste" onSubmit={(e) => void send(e)}>
          <p>
            Sign in on the page that opened. At the end the browser shows a page
            that does not load. Copy its address and paste it here.
          </p>
          <label>
            <span>Address of the page that did not load</span>
            <input
              ref={field}
              type="url"
              inputMode="url"
              autoCapitalize="off"
              autoCorrect="off"
              autoComplete="off"
              spellCheck={false}
              placeholder={`http://localhost:${details.port ?? ""}${details.redirectPath ?? "/"}?code=…`}
              value={address}
              disabled={state === "sending"}
              onChange={(event) => view.setSignInAddress(event.target.value)}
            />
          </label>
          {errorBlock}
          <div className="sign-in-card-actions">
            <Button
              variant="subtle"
              disabled={sending}
              onClick={() => void post({ decision: "decline" })}
            >
              Cancel sign-in
            </Button>
            <Button
              variant="default"
              disabled={sending}
              onClick={() => open(true)}
            >
              Open the page again
            </Button>
            <Button
              variant="default"
              disabled={state === "sending"}
              onClick={() => void pasteFromClipboard()}
            >
              Paste
            </Button>
            <Button
              type="submit"
              variant="filled"
              disabled={state === "sending" || !pasteBody(address)}
              loading={sending}
            >
              Send to command
            </Button>
          </div>
        </form>
      )}
    </section>
  );
});
