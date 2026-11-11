import { WorkspaceResourceState as ResourceState } from "../WorkspaceResourceState/WorkspaceResourceState.tsx";
import { observer } from "mobx-react-lite";
import { isRecord } from "../McpInventory/McpInventory.tsx";
import type { Context } from "../Workspace/Workspace.tsx";
import { WorkspaceSection as Empty } from "../WorkspaceSection/WorkspaceSection.tsx";
import { WorkspaceStatus as Status } from "../WorkspaceStatus/WorkspaceStatus.tsx";

export const WorkspacePlan = observer(function WorkspacePlan(c: Context) {
  const state = c.model.planState;
  const native = state.data?.native;
  const steps = native?.plan ?? [];
  const explanation = native?.explanation ?? "";
  return (
    <>
      <ResourceState state={state} />
      {steps.length || explanation ? (
        <section className="workspace-result">
          <h3>Agent plan</h3>
          {explanation && <p className="workspace-prose">{explanation}</p>}
          {steps.map((step, i) => {
            const detail = isRecord(step) ? step : null;
            const status =
              typeof detail?.status === "string" ? detail.status : "pending";
            const text =
              typeof detail?.step === "string"
                ? detail.step
                : JSON.stringify(step);
            return (
              <div className="workspace-plan-step" key={i}>
                <Status value={status} />
                <span>{text}</span>
              </div>
            );
          })}
        </section>
      ) : (
        !state.loading && <Empty>This agent has not reported a plan.</Empty>
      )}
    </>
  );
});
