import { Owned } from "../../../ui/Owned/Owned.tsx";
import { observer } from "mobx-react-lite";
import { createElement, type ReactNode } from "react";

/** Compose transcript leaves with the shared side-effect-free mount owner. */
export function ownedView<P extends object, M>(
  create: (props: P) => M,
  view: (props: P & { model: M }) => ReactNode,
  identity?: (props: P) => string,
) {
  const renderView = observer(view);
  return observer((props: P) => (
    <Owned key={identity?.(props)} create={() => create(props)}>
      {(model) => createElement(renderView, { ...props, model })}
    </Owned>
  ));
}
