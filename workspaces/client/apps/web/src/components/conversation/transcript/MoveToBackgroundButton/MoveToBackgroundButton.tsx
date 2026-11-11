import { ownedView } from "../support/ownedView.tsx";
import { BackgroundCommand } from "./BackgroundCommand.ts";
import { ArrowDownToLine, LoaderCircle } from "lucide-react";
import { useServerApi } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
/** Move one running command to the background. The agent continues at once. */
export default ownedView(
  (_props: { agentId: string; turnId?: string | undefined; itemId: string }) =>
    new BackgroundCommand(),
  function MoveToBackgroundButton({ agentId, turnId, itemId, model }) {
    const api = useServerApi();
    const { busy, done, result, message } = model;
    return (
      <span className="tool-background" data-background-result={result?.kind}>
        {message && (
          <small className="tool-background-message" role="status">
            {message}
          </small>
        )}
        <button
          type="button"
          className="tool-background-button"
          onClick={(event) => {
            event.preventDefault();
            event.stopPropagation();
            void model.run(api, agentId, turnId, itemId);
          }}
          disabled={busy || done}
          title="Keep the command running and let the agent continue. The agent gets the result when the command ends."
        >
          {busy ? (
            <LoaderCircle size={12} className="spin" />
          ) : (
            <ArrowDownToLine size={12} />
          )}
          {busy
            ? "Moving…"
            : done
              ? "Moved to background"
              : result?.kind === "uncertain"
                ? "Check result"
                : "Move to background"}
        </button>
      </span>
    );
  },
  (props) => `${props.agentId}:${props.turnId}:${props.itemId}`,
);
