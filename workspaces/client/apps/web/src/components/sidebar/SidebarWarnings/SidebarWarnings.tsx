import { observable, observableRef } from "mobx";
import { observer } from "mobx-react-lite";
import { createContext, useContext, type ReactNode } from "react";
import { createPortal } from "react-dom";
import ConversationWarnings from "../../conversation/ConversationWarnings/ConversationWarnings.tsx";
import { Owned } from "../../ui/Owned/Owned.tsx";

const createWarningTarget = () =>
  observable(
    {
      target: null as HTMLElement | null,
      setTarget(target: HTMLElement | null) {
        this.target = target;
      },
    },
    { target: observableRef },
    { autoBind: true },
  );
const Target = createContext<ReturnType<typeof createWarningTarget> | null>(
  null,
);

/** The active server retains API ownership while the shared sidebar supplies the target. */
export function SidebarWarningsProvider({ children }: { children: ReactNode }) {
  return (
    <Owned create={() => createWarningTarget()}>
      {(target) => <Target.Provider value={target}>{children}</Target.Provider>}
    </Owned>
  );
}
export function SidebarWarningsTarget() {
  const target = useContext(Target);
  return <span className="sidebar-warnings-target" ref={target?.setTarget} />;
}
export default observer(function SidebarWarnings() {
  const target = useContext(Target)?.target;
  return target ? createPortal(<ConversationWarnings />, target) : null;
});
