import { createContext, useContext, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { observer } from "mobx-react-lite";
import ConversationWarnings from "../../conversation/ConversationWarnings/ConversationWarnings.tsx";

const Target = createContext<HTMLElement | null>(null);
const SetTarget = createContext<(target: HTMLElement | null) => void>(() => {});

/** The active server retains API ownership while the shared sidebar supplies the target. */
export function SidebarWarningsProvider({ children }: { children: ReactNode }) {
  const [target, setTarget] = useState<HTMLElement | null>(null);
  return (
    <SetTarget.Provider value={setTarget}>
      <Target.Provider value={target}>{children}</Target.Provider>
    </SetTarget.Provider>
  );
}

export function SidebarWarningsTarget() {
  const setTarget = useContext(SetTarget);
  return <span className="sidebar-warnings-target" ref={setTarget} />;
}

export default observer(function SidebarWarnings() {
  const target = useContext(Target);
  return target ? createPortal(<ConversationWarnings />, target) : null;
});
