import { textField } from "../../../../stores/fields.ts";
import type { SearchSource } from "../support/SearchOverlay.tsx";
import { useServerApi } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import {
  Badge,
  Button,
  Loader,
  Modal,
  TextInput,
  UnstyledButton,
} from "@mantine/core";
import { Search } from "lucide-react";
import { Owned } from "../../../ui/Owned/Owned.tsx";
import { SearchStore } from "./SearchStore.ts";
import { observer } from "mobx-react-lite";
import { useServerStore } from "../../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { resourceData } from "../../../../store/runtime.ts";

export const SearchContent = observer(function SearchContent({
  source: c,
}: {
  source: SearchSource;
}) {
  const api = useServerApi(),
    store = useServerStore();
  return (
    <Owned create={() => new SearchStore()}>
      {(s) => (
        <>
          <form
            className="workspace-toolbar"
            onSubmit={(e) => {
              e.preventDefault();
              s.submit(api, store);
            }}
          >
            <TextInput
              className="workspace-grow"
              autoFocus
              aria-label="Search all conversations"
              placeholder="Search messages, work, and agent chats"
              {...textField(s, "query")}
              leftSection={<Search size={16} />}
            />
            <Button
              variant="filled"
              type="submit"
              disabled={!s.query.trim()}
              loading={s.state.loading}
            >
              Search
            </Button>
          </form>
          {s.state.error ? (
            <p role="alert" className="workspace-error">
              {s.state.error}
            </p>
          ) : s.state.loading && !s.state.data ? (
            <Loader size="sm" />
          ) : null}
          {(s.state.data?.results || []).map((result, index) => (
            <UnstyledButton
              className="workspace-row"
              key={`${result.kind}:${result.id}:${index}`}
              onClick={() => void s.loadSource(api, result)}
            >
              <div className="workspace-row-head">
                <Badge size="xs" variant="light" color="gray">
                  {result.kind}
                </Badge>
                <small>
                  <SearchAgentName id={result.agent || ""} />
                </small>
              </div>
              <p className="workspace-prose">{result.text}</p>
            </UnstyledButton>
          ))}
          {s.search && s.state.data && !s.state.data.results?.length && (
            <div className="workspace-empty">No results for “{s.search}”.</div>
          )}
          {!s.search && (
            <div className="workspace-empty">
              Searches all messages, archived chats too.
            </div>
          )}
          <Modal
            opened={!!s.source}
            onClose={s.closeSource}
            title="Search source"
            size="lg"
          >
            {s.source && (
              <>
                <div className="workspace-toolbar">
                  <Badge variant="light" color="gray">
                    {s.source.kind ||
                      ("type" in s.source && typeof s.source.type === "string"
                        ? s.source.type
                        : "")}
                  </Badge>
                  <small className="workspace-muted">{s.source.id}</small>
                </div>
                {s.sourceError && (
                  <p role="alert" className="workspace-error">
                    {s.sourceError}
                  </p>
                )}
                {s.source.loading ? (
                  <Loader size="sm" />
                ) : (
                  <p className="workspace-prose">
                    {typeof s.source.text === "string"
                      ? s.source.text
                      : JSON.stringify(s.source, null, 2)}
                  </p>
                )}
                <Button variant="light" onClick={() => s.openSource(c)}>
                  Open {s.source.kind === "plan" ? "plan" : "chat"}
                </Button>
              </>
            )}
          </Modal>
        </>
      )}
    </Owned>
  );
});

const SearchAgentName = observer(function SearchAgentName({
  id,
}: {
  id: string;
}) {
  return (
    resourceData(useServerStore().state().current)?.agents.get(id)?.name ||
    id ||
    "Unassigned"
  );
});
