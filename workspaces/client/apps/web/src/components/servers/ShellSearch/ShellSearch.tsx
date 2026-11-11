import { Button, Modal, TextInput } from "@mantine/core";
import { observer } from "mobx-react-lite";
import { textField } from "../../../stores/fields.ts";

import { ShellState } from "../../../app/shell/ShellState.ts";

export const ShellSearch = observer(function ShellSearch({
  shell,
}: {
  shell: ShellState;
}) {
  return (
    <Modal
      opened={shell.searchOpen}
      onClose={() => shell.set({ searchOpen: false })}
      title="Search all servers"
      aria-label="Search all servers"
      size="lg"
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          shell.submitSearch();
        }}
      >
        <TextInput
          autoFocus
          label="Search all messages"
          {...textField(shell, "searchQuery")}
        />
        <Button type="submit" disabled={!shell.searchQuery.trim()}>
          Search
        </Button>
      </form>
      {shell.searchRequest &&
        shell.servers.map((server) => (
          <section key={server.id}>
            <h3>{server.label}</h3>
            {!shell.searchResults[server.id] && (
              <p role="status">Searching this server...</p>
            )}
            {shell.searchResults[server.id]?.error && (
              <p role="alert">{shell.searchResults[server.id]?.error}</p>
            )}
            {shell.searchResults[server.id]?.results.map((result, index) => (
              <button
                className="server-search-result"
                key={`${result.id}:${index}`}
                onClick={() => {
                  shell.send(server.id, {
                    action: "open",
                    id: result.room || result.agent,
                    messageId: result.id,
                  });
                  shell.set({ searchOpen: false });
                }}
              >
                <small>{result.kind}</small>
                <p>{result.text}</p>
              </button>
            ))}
            {shell.searchResults[server.id]?.results.length === 0 &&
              !shell.searchResults[server.id]?.error && <p>No results.</p>}
          </section>
        ))}
    </Modal>
  );
});
