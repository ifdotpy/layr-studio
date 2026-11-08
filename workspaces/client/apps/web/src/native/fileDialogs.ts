import { hasNativeClient, nativeRequest } from "./client.ts";
import { isRemoteServerView } from "../servers/environment.ts";

export function createFileDialogs(owner: string) {
  const local = owner === "local";
  function canPickDirectory() {
    return (
      local &&
      (typeof window.codexDesktop?.pickDirectory === "function" ||
        hasNativeClient())
    );
  }
  function pickDirectory(): Promise<string | null> {
    if (!canPickDirectory())
      return Promise.reject(
        new Error("Native folder selection is unavailable."),
      );
    if (window.codexDesktop?.pickDirectory)
      return window.codexDesktop.pickDirectory();
    return nativeRequest("desktop.pickDirectory", {});
  }
  function canPickFiles() {
    return (
      local &&
      (typeof window.codexDesktop?.pickFiles === "function" ||
        hasNativeClient())
    );
  }
  async function pickFiles(): Promise<File[]> {
    if (!canPickFiles())
      throw new Error("Native file selection is unavailable.");
    const files = window.codexDesktop?.pickFiles
      ? (await window.codexDesktop.pickFiles()).map((file) => ({
          ...file,
          base64: file.data,
        }))
      : await nativeRequest<{ name: string; mime: string; base64: string }[]>(
          "desktop.pickAttachments",
          {},
        );
    return files.map(
      (file) =>
        new File(
          [Uint8Array.from(atob(file.base64), (c) => c.charCodeAt(0))],
          file.name,
          { type: file.mime },
        ),
    );
  }

  function canRevealPath() {
    return local && (!!window.codexDesktop?.revealPath || hasNativeClient());
  }
  async function revealPath(path: string): Promise<void> {
    if (!canRevealPath())
      throw new Error(
        "Local folder reveal is unavailable in this server view.",
      );
    if (window.codexDesktop?.revealPath)
      return window.codexDesktop.revealPath(path);
    await nativeRequest("desktop.revealPath", { value: path });
  }

  return {
    canPickDirectory,
    pickDirectory,
    canPickFiles,
    pickFiles,
    canRevealPath,
    revealPath,
  };
}

export const {
  canPickDirectory,
  pickDirectory,
  canPickFiles,
  pickFiles,
  canRevealPath,
  revealPath,
} = createFileDialogs(isRemoteServerView ? "remote" : "local");
