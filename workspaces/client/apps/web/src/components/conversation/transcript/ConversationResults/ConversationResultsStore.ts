import { makeAutoObservable, compareStructural } from "mobx";
import { derived } from "../../support/derived.ts";
import { PreviewSelection } from "../../support/PreviewSelection.ts";
import {
  conversationResults,
  type ConversationResult,
} from "./conversationResultModel.ts";
import type { Message } from "../../../../types.ts";
import type { useServerNet } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
export class ConversationResultsStore {
  readonly preview = new PreviewSelection();
  selection: { id: string; agent: string | undefined } | undefined;
  results = derived(conversationResults, { equals: compareStructural });
  view = derived((messages: Message[], agentId: string | undefined) => {
    const results = this.results(messages);
    const selected =
      this.selection?.agent === agentId
        ? results.find((result) => result.id === this.selection?.id)
        : undefined;
    return {
      results,
      selected,
      file: selected?.kind === "file" || selected?.kind === "asset",
    };
  });
  constructor() {
    makeAutoObservable(this, {}, { autoBind: true, deep: false });
  }
  close(net: ReturnType<typeof useServerNet>) {
    this.selection = undefined;
    this.preview.set(net, null);
  }
  select(
    result: ConversationResult,
    agent: string | undefined,
    net: ReturnType<typeof useServerNet>,
  ) {
    if (result.kind === "asset") this.preview.set(net, { asset: result.asset });
    else if (result.kind === "file")
      this.preview.set(net, { agent, path: result.path, line: result.line });
    this.selection = { id: result.id, agent };
  }
}
