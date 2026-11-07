import type { FormEvent } from "react";
export type ChatRowActions = {
  prepare: () => void;
  open: () => void;
  pin: () => void;
  rename: (event: FormEvent) => void;
  name: (value: string) => void;
  cancelRename: () => void;
  beginRename: () => void;
  unread: () => void;
  move: () => void;
  changeProject: () => void;
  archive: () => void;
  remove: () => void;
};
