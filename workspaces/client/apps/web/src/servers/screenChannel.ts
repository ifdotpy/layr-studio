import { action, flow, makeAutoObservable, observable } from "mobx";
import type { GetResult } from "../api.ts";
import type { ServerCommand } from "./navigation.ts";

export type ScreenMessage = { kind: string; [key: string]: unknown };
export type ScreenSearchResults = Pick<GetResult<"/api/search">, "results"> & {
  error?: string | undefined;
};
export type ScreenSearchResultsMessage = ScreenMessage &
  Partial<ScreenSearchResults> & {
    kind: "studio-server-search-results";
    requestId: string;
  };

/** Search replies come from the same renderer's generated API client. */
export function screenSearchResults(
  message: ScreenMessage,
): ScreenSearchResults {
  const value = message as ScreenSearchResultsMessage;
  return { results: value.results || [], error: value.error };
}
type CommandHandler = (command: ServerCommand) => void | Promise<void>;

/** One connection lifetime owns its commands, even after another screen opens. */
export function createScreenChannel(
  owner: string,
  publish: (message: ScreenMessage & { serverId: string }) => void,
) {
  let closed = false;
  const handler = observable.box<CommandHandler | undefined>(undefined, {
    deep: false,
  });
  let terminal: ((operation: "toggle" | "new") => void) | undefined;
  return makeAutoObservable(
    {
      owner,
      get ready() {
        return !closed && !!handler.get();
      },
      publish(message: ScreenMessage) {
        if (!closed) publish({ ...message, serverId: owner });
      },
      bind(next: CommandHandler) {
        if (closed) throw new Error("The server screen connection closed.");
        if (handler.get())
          throw new Error("The server screen is already mounted.");
        handler.set(next);
        return action(() => {
          if (handler.get() === next) handler.set(undefined);
        });
      },
      bindTerminal(next: (operation: "toggle" | "new") => void) {
        if (closed) throw new Error("The server screen connection closed.");
        terminal = next;
        return () => {
          if (terminal === next) terminal = undefined;
        };
      },
      execute: flow(function* (command: ServerCommand) {
        const run = handler.get();
        if (closed || !run)
          throw new Error(
            "The server screen is unavailable. The action was not sent.",
          );
        if (command.action === "terminal") {
          if (!terminal)
            throw new Error(
              "The server terminal is unavailable. The action was not sent.",
            );
          terminal(command.operation);
          return;
        }
        // No retries: the handler may have sent a mutation before its reply failed.
        yield run(command);
      }),
      close() {
        closed = true;
        handler.set(undefined);
        terminal = undefined;
      },
    },
    { owner: false },
    { autoBind: true },
  );
}
export type ScreenChannel = ReturnType<typeof createScreenChannel>;
