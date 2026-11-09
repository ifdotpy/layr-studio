import type { IncomingMessage, ServerResponse } from "node:http";
import type { AgentEntityDto } from "../../src/generated/api.ts";
import {
  handleEntitySyncFixtureRequest,
  updateEntitySyncFixture,
  type EntityFixtureState,
  type ResourceRef,
} from "../playwright.ts";

/** Test controls update the server's facts; clients receive them only over SSE. */
export function agentSseServer(
  workspaceId = "1234567890abcdef1234567890abcdef",
) {
  const snapshot: EntityFixtureState = {
    stateDir: "fixture",
    chats: [],
    edges: [],
    runtime: { agents: [] },
  };
  let notify: (refs: ResourceRef[]) => void = () => {};
  const publishSnapshot = (value: EntityFixtureState) => {
    Object.assign(snapshot, value);
    updateEntitySyncFixture(snapshot);
    notify([{ kind: "state" }]);
  };
  return {
    publishSnapshot,
    handle(request: IncomingMessage, response: ServerResponse) {
      const path = new URL(request.url!, "http://fixture").pathname;
      if (
        (path === "/fixture/agent" || path === "/fixture/publish-state") &&
        request.method === "POST"
      ) {
        let body = "";
        request.on("data", (chunk) => {
          body += chunk;
        });
        request.on("end", () => {
          if (path === "/fixture/publish-state") {
            const value: EntityFixtureState = JSON.parse(body);
            publishSnapshot(value);
            response.end("{}");
            return;
          }
          const agent: AgentEntityDto = JSON.parse(body);
          snapshot.runtime!.agents = [agent];
          updateEntitySyncFixture(snapshot);
          notify([
            { kind: "state" },
            { kind: "agentDetail", agentId: agent.id },
          ]);
          response.end("{}");
        });
        return true;
      }
      return handleEntitySyncFixtureRequest(request, response, {
        snapshot,
        workspaceId,
        onStreamReady: (accept) => {
          notify = accept;
        },
      });
    },
  };
}
