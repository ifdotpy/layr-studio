import { readServers, writeServers, type StudioServer } from "./registry.ts";
import { autorun } from "mobx";
import { resourceData } from "../store/runtime.ts";
import type { createServerStore } from "../store/server.ts";

export function validateServerName(label: string) {
  if (
    Array.from(label).length < 1 ||
    Array.from(label).length > 80 ||
    label !== label.trim() ||
    /[\p{C}\p{Zl}\p{Zp}\t\n\r\v\f]/u.test(label) ||
    /[^ ]/u.test(label.replace(/[^\s]/gu, ""))
  )
    throw new Error("Use 1 to 80 visible characters without outer spaces.");
}

function applyServerName(server: StudioServer, label: string) {
  validateServerName(label);
  const rows = readServers();
  if (
    rows.some(
      (row) =>
        row.id === server.id &&
        row.origin === server.origin &&
        row.credentialId === server.credentialId &&
        row.label !== label,
    )
  )
    writeServers(
      rows.map((row) =>
        row.id === server.id &&
        row.origin === server.origin &&
        row.credentialId === server.credentialId
          ? { ...row, label }
          : row,
      ),
    );
}

/** Each saved server owns its name through its existing discovery stream. */
export function watchServerName(
  server: StudioServer,
  store: ReturnType<typeof createServerStore>,
) {
  return autorun(() => {
    const identity = resourceData(store.multiServer().current)?.identity;
    if (identity?.serverId === server.id)
      applyServerName(server, identity.label);
  });
}
