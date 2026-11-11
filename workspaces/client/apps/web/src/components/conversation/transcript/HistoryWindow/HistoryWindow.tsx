import type { TranscriptStore } from "../../support/TurnHistoryStore.ts";
import { observer } from "mobx-react-lite";
import { Fragment, useEffect, useLayoutEffect, useRef } from "react";
import { HistoryContent } from "../HistoryContent/HistoryContent.tsx";
import { mountHistoryViewportDom } from "./HistoryViewportDom.ts";

/** Only DOM attachment and keyed row rendering belong to React. */
export default observer(function HistoryWindow({
  model,
}: {
  model: TranscriptStore;
}) {
  const body = useRef<HTMLDivElement>(null);
  const dom = useRef<ReturnType<typeof mountHistoryViewportDom> | null>(null);
  const { scrollContainer, storageKey, hasRecentScrollInput } = model.input;
  const { rows, offsets, indices, active } = model;
  const { revealedMessage } = model;
  useEffect(() => {
    const root = scrollContainer?.current,
      node = body.current;
    if (!root || !node) return;
    dom.current = mountHistoryViewportDom(model, node, root);
    dom.current.observeRows();
    return () => {
      dom.current?.stop();
      dom.current = null;
    };
  }, [model, scrollContainer, storageKey, hasRecentScrollInput]);
  useLayoutEffect(() => {
    dom.current?.update();
  }, [model, rows, offsets, indices]);
  useLayoutEffect(() => dom.current?.observeRows(), [model, active, indices]);
  let previous = 0;
  return (
    <div
      ref={body}
      className={active ? "history-window" : "history-window-static"}
      data-history-window={active || undefined}
    >
      {(active ? indices : rows.map((_, index) => index)).map((index) => {
        // Indices come from rows; offsets contains each row boundary plus its end.
        const row = rows[index]!;
        const gap = active ? offsets[index]! - offsets[previous]! : 0;
        previous = index + 1;
        return (
          <Fragment key={row.id}>
            {gap > 0 && (
              <div
                className="history-window-spacer"
                style={{ height: gap }}
                aria-hidden="true"
              />
            )}
            <div className="history-window-row" data-history-row={row.id}>
              <HistoryContent
                group={row}
                store={model}
                revealedMessage={revealedMessage}
              />
            </div>
          </Fragment>
        );
      })}
      {active && (
        <div
          className="history-window-spacer"
          style={{ height: offsets[rows.length]! - offsets[previous]! }}
          aria-hidden="true"
        />
      )}
    </div>
  );
});
