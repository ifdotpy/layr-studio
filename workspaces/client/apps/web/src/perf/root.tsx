import { configure } from "mobx";

// This root is included only by the performance build's renderer alias.
configure({
  enforceActions: "observed",
  computedRequiresReaction: true,
  reactionRequiresObservable: true,
  observableRequiresReaction: true,
});

export * from "react-dom/client";
