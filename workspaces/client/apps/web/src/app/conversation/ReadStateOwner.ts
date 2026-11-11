import { flowValue } from "../../flowValue.ts";
import type { ServerAreas } from "../ServerAreasProvider.tsx";
import {
  makeAutoObservable,
  observable,
  observableRef,
  reaction,
  flow,
} from "mobx";
import { MarkUnreadEligibility } from "../../generated/api.ts";
import { ApiError, errorText, type PostResult } from "../../api.ts";
import { resourceData } from "../../store/runtime.ts";
import type { Agent } from "../../types.ts";
export type ChatReadState = NonNullable<Agent["readState"]>;
export type ChatReadProof = {
  id: string;
  threadId: string;
  turnId: string;
};
export const completed = (
  agent: Pick<Agent, "id" | "markRead">,
): ChatReadProof | null =>
  agent.markRead
    ? {
        id: agent.id,
        threadId: agent.markRead.threadId,
        turnId: agent.markRead.turnId,
      }
    : null;
const sameResult = (a: ChatReadProof, b: ChatReadProof) =>
  a.id === b.id && a.threadId === b.threadId && a.turnId === b.turnId;
export function matchingCompletedAgent(
  agent: Agent | undefined,
  proof: ChatReadProof,
): Agent | null {
  const result = agent && completed(agent);
  return result && sameResult(result, proof) ? agent : null;
}
export class ReadStateOwner {
  mounted = true;
  selected: string | null = null;
  scope = this.createScope();
  readonly areas;
  constructor(areas: ServerAreas) {
    this.areas = areas;
    makeAutoObservable(
      this,
      { areas: false, scope: observableRef, mounted: false },
      { autoBind: true },
    );
  }
  createScope() {
    return {
      states: observable.map<string, ChatReadState>({}, { deep: false }),
      requests: observable.map<string, Promise<void>>({}, { deep: false }),
      pendingReadTargets: observable.map<string, ChatReadState>(
        {},
        { deep: false },
      ),
      suppressed: new Set<string>(),
      attempted: new Set<string>(),
      uncertain: observable.map<string, ChatReadState>({}, { deep: false }),
    };
  }
  get marking() {
    return new Set(this.scope.requests.keys());
  }
  get pendingReadTargets(): ReadonlyMap<string, ChatReadState> {
    return this.scope.pendingReadTargets;
  }
  summary(id: string) {
    return resourceData(this.areas.scope.store.state().current)?.agents.get(id);
  }
  readStateFor(agent: Agent): ChatReadState | null {
    const saved = agent.readState ?? null,
      known = this.scope.states.get(agent.id);
    return saved && (!known || saved.revision > known.revision)
      ? saved
      : known || saved;
  }
  confirmedState(id: string, expected: ChatReadState): ChatReadState | null {
    const agent = matchingCompletedAgent(this.summary(id), { id, ...expected });
    const state = agent?.readState;
    return state &&
      state.threadId === expected.threadId &&
      state.turnId === expected.turnId &&
      (state.revision > expected.revision ||
        (state.revision === expected.revision && state.read === expected.read))
      ? { ...state }
      : null;
  }
  start() {
    const { startup } = this.areas;
    this.mounted = true;
    const reset = reaction(
      () => JSON.stringify([startup.creationScope, startup.workspaceId]),
      () => {
        this.scope = this.createScope();
      },
    );
    const confirmations = reaction(
      () =>
        [...this.scope.uncertain].map(([id, expected]) => ({
          id,
          state: this.confirmedState(id, expected),
        })),
      (entries) => {
        for (const { id, state } of entries)
          if (state) {
            this.scope.states.set(id, state);
            this.scope.uncertain.delete(id);
          }
      },
    );
    const selection = reaction(
      () => this.areas.navigation.selectedId,
      (opened) => {
        this.scope.suppressed.clear();
        this.scope.attempted.clear();
        if (!opened || this.selected !== opened) return;
        this.selected = null;
        const agent = this.summary(opened),
          proof = agent && completed(agent);
        if (proof) void this.write(proof, true);
      },
    );
    return () => {
      this.mounted = false;
      reset();
      confirmations();
      selection();
    };
  }
  write: (
    proof: ChatReadProof,
    read: boolean,
    target?: ChatReadState,
  ) => Promise<void> = flow(function* (
    this: ReadStateOwner,
    proof: ChatReadProof,
    read: boolean,
    target?: ChatReadState,
  ) {
    const { navigation } = this.areas;
    const current = this.scope;
    const valid = () => this.mounted && this.scope === current;
    const pending = current.requests.get(proof.id);
    if (pending) {
      if (read) return;
      yield* flowValue(pending);
      if (!valid()) return;
      yield* flowValue(this.write(proof, read, target));
      return;
    }
    const agent = this.summary(proof.id);
    const result = agent && completed(agent);
    if (
      !target &&
      (!agent || !agent.markRead || !result || !sameResult(result, proof))
    )
      return;
    if (
      read &&
      (navigation.selectedId !== proof.id ||
        current.suppressed.has(proof.id) ||
        document.visibilityState !== "visible" ||
        !document.hasFocus())
    )
      return;
    const saved = target ?? (agent ? this.readStateFor(agent) : null);
    if (
      saved?.threadId === proof.threadId &&
      saved.turnId === proof.turnId &&
      saved.read === read &&
      !current.uncertain.has(proof.id)
    )
      return;
    const attempt = JSON.stringify([
      proof.id,
      proof.threadId,
      proof.turnId,
      saved?.revision || 0,
    ]);
    if (read && current.attempted.has(attempt)) return;
    if (read) current.attempted.add(attempt);
    if (read)
      current.pendingReadTargets.set(proof.id, {
        threadId: proof.threadId,
        turnId: proof.turnId,
        revision: saved?.revision || 0,
        read: false,
      });
    const expected: ChatReadState = {
      threadId: proof.threadId,
      turnId: proof.turnId,
      read,
      revision: saved?.revision || 0,
    };
    const request = flow(function* (this: ReadStateOwner) {
      try {
        const body = {
          id: proof.id,
          read_state: {
            thread_id: proof.threadId,
            turn_id: proof.turnId,
            read,
            expected_revision: expected.revision,
          },
        };
        const requestWorkspace = this.areas.startup.workspaceId;
        let response: PostResult<"/api/organization">;
        for (let retry = 0; ; retry++) {
          try {
            response = yield* flowValue(
              this.areas.scope.services.api.post("/api/organization", body, {
                workspaceId: requestWorkspace,
                timeoutMs: 15000,
              }),
            );
            break;
          } catch (error) {
            const transient =
              error instanceof TypeError ||
              (error instanceof ApiError &&
                (error.status >= 500 || [408, 429].includes(error.status)));
            if (!read || !transient || retry >= 2) throw error;
            current.uncertain.set(proof.id, expected);
            yield* flowValue(
              new Promise((resolve) => setTimeout(resolve, 1000 * (retry + 1))),
            );
            // Retry the exact compare-and-set body; a queued Unread follows it.
            if (!valid()) return;
            if (navigation.selectedId !== proof.id) throw error;
          }
        }
        if (!valid()) return;
        const state = response.readState;
        if (
          response.id !== proof.id ||
          !state ||
          state.threadId !== proof.threadId ||
          state.turnId !== proof.turnId ||
          state.read !== read ||
          state.revision < expected.revision
        )
          throw new Error("The server did not confirm the read status.");
        const known = current.states.get(proof.id);
        if (!known || state.revision >= known.revision)
          current.states.set(proof.id, state);
        current.uncertain.delete(proof.id);
      } catch (error) {
        if (valid()) {
          current.attempted.delete(attempt);
          current.uncertain.set(proof.id, expected);
          const state = this.confirmedState(proof.id, expected);
          if (state) {
            current.states.set(proof.id, state);
            current.uncertain.delete(proof.id);
          } else this.areas.notices.notify(errorText(error));
        }
      }
    }).call(this);
    current.requests.set(proof.id, request);
    try {
      yield* flowValue(request);
    } finally {
      if (current.requests.get(proof.id) === request)
        current.requests.delete(proof.id);
      current.pendingReadTargets.delete(proof.id);
    }
  });
  markUnread = flow(function* (this: ReadStateOwner, agent: Agent) {
    const model = this.summary(agent.id);
    const proof = model && completed(model);
    if (!proof || model?.markUnread !== MarkUnreadEligibility["allowed"])
      return;
    if (this.areas.navigation.selectedId === agent.id)
      this.scope.suppressed.add(agent.id);
    yield* flowValue(this.write(proof, false));
  });
  markSidebarUnread: (id: string, target: ChatReadState) => Promise<void> =
    flow(function* (this: ReadStateOwner, id: string, target: ChatReadState) {
      // Generated nested models update in place; a queued CAS keeps the click's proof.
      const captured = { ...target };
      if (this.areas.navigation.selectedId === id)
        this.scope.suppressed.add(id);
      yield* flowValue(
        this.write(
          { id, threadId: captured.threadId, turnId: captured.turnId },
          false,
          captured,
        ),
      );
    });
  observeRead(proof: ChatReadProof) {
    if (this.selected !== proof.id) return;
    this.selected = null;
    void this.write(proof, true);
  }
  select(id: string) {
    this.selected = id;
    if (this.areas.navigation.selectedId !== id) return;
    const agent = this.summary(id);
    const proof = agent && completed(agent);
    if (proof) {
      this.selected = null;
      void this.write(proof, true);
    }
  }
}
