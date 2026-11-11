import type { AgentDetailEntityDto } from "../../../../generated/observableResources.ts";
import { useServerApi } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import ErrorDescription from "../../../shell/ErrorDescription/ErrorDescription.tsx";
import { ownedView } from "../support/ownedView.tsx";
import { SafetyBufferingStore } from "./SafetyBufferingStore.ts";
import { Button, Modal } from "@mantine/core";
import { Clock3 } from "lucide-react";
import type { Agent } from "../../../../types.ts";

export default ownedView(
  (_props: {
    agent: Agent;
    detail?: AgentDetailEntityDto | undefined | Agent;
  }) => new SafetyBufferingStore(),
  function SafetyBuffering({ agent, detail = agent, model }) {
    const api = useServerApi();
    const { confirm, pending, error } = model;
    const { b, retry, turnId, active, visible, failed } = model.state(
      agent,
      detail,
    );
    if (!visible && !active && !failed) return null;
    const choose = (safety: "wait" | "retry" | "cancel") =>
      model.choose(api, agent.id, turnId, safety);
    return (
      <div
        className="native-safety"
        data-safety-state={active || failed ? retry?.stage : "waiting"}
      >
        <div className="native-safety-title" role="status">
          <Clock3 size={16} />
          <strong>
            {active
              ? "Changing the model"
              : failed
                ? "The model change could not finish"
                : "Codex is checking this request"}
          </strong>
        </div>
        <p>
          {active ? (
            "Waiting for Codex to confirm. You can continue in another chat."
          ) : failed ? (
            <ErrorDescription value={retry?.error} />
          ) : (
            "Extra safety checks can take longer. You can keep waiting."
          )}
        </p>
        {active && retry?.error && (
          <p>
            <ErrorDescription value={retry?.error} />
          </p>
        )}
        {active && retry?.rpcMethod !== "turn/start" && (
          <Button
            size="compact-xs"
            variant="subtle"
            loading={pending}
            onClick={() => void choose("cancel")}
          >
            Cancel model change
          </Button>
        )}
        {visible && !active && !failed && (
          <div className="native-error-actions">
            {!!b?.fasterModel && (
              <Button
                size="compact-xs"
                variant="light"
                onClick={() => model.setConfirm(true)}
              >
                Retry with a faster model
              </Button>
            )}
            <Button
              size="compact-xs"
              variant="subtle"
              loading={pending}
              onClick={() => void choose("wait")}
            >
              Keep waiting
            </Button>
            <a
              href="https://help.openai.com/en/articles/20001326"
              target="_blank"
              rel="noreferrer"
            >
              Learn more
            </a>
          </div>
        )}
        {error && <p role="alert">{error}</p>}
        <Modal
          opened={confirm && !!visible && !active}
          onClose={() => !pending && model.setConfirm(false)}
          title="Retry with a faster model"
          centered
        >
          <p>
            Codex offers {b?.fasterModel}. It may be less capable with complex
            requests.
          </p>
          <p>
            Studio will stop this turn and retry its input from the same prior
            context.
          </p>
          <div className="native-error-actions">
            <Button
              variant="subtle"
              disabled={pending}
              onClick={() => model.setConfirm(false)}
            >
              Keep waiting
            </Button>
            <Button loading={pending} onClick={() => void choose("retry")}>
              Stop and retry
            </Button>
          </div>
        </Modal>
      </div>
    );
  },
  (props) => props.agent.id,
);
