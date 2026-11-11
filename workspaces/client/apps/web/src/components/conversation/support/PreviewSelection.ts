import { makeAutoObservable, observableRef } from "mobx";
import type { Net } from "../../../servers/net/net.ts";
import { FilePreviewState } from "./FilePreviewState.ts";
import type {
  FilePreviewSelection,
  PreviewTarget,
} from "../FilePreview/FilePreview.tsx";

export function openFilePreview(
  net: Net,
  target: PreviewTarget,
): FilePreviewSelection {
  const { agent, path, asset } = target;
  const slot = net.reads.file({ agent, path, asset });
  void slot.read().catch(() => {});
  return { target, slot, state: new FilePreviewState({ target, slot }) };
}

/** One action-selected preview, scoped to the server that opened it. */
export class PreviewSelection {
  selected: { net: Net; value: FilePreviewSelection } | null = null;

  constructor() {
    makeAutoObservable(this, { selected: observableRef }, { autoBind: true });
  }

  current(net: Net) {
    return this.selected?.net === net ? this.selected.value : null;
  }

  set(net: Net, target: PreviewTarget | null) {
    this.selected = target
      ? { net, value: openFilePreview(net, target) }
      : null;
  }
}
