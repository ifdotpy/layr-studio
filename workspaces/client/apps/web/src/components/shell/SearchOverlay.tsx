import { useServerApi } from "../../servers/ServerServicesContext";
import {
  Badge,
  Button,
  Loader,
  Modal,
  TextInput,
  UnstyledButton,
} from "@mantine/core";
import { Search } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { errorText, type GetResult } from "../../api";
import { observer } from "mobx-react-lite";
import { useServerStore } from "../../servers/ServerServicesContext";
import { resourceData } from "../../store/runtime.ts";
import type { ReadSlot } from "../../store/runtime.ts";
import "./Workspace.css";
export interface SearchSource {
  readonly opened: boolean;
  select(id: string, messageId?: string): void;
  close(): void;
  navigate(section: string, agent?: string): void;
}
export default observer(function SearchOverlay({
  source,
}: {
  source: SearchSource;
}) {
  return (
    <Modal
      opened={source.opened}
      onClose={source.close}
      title="Search messages"
      size="lg"
    >
      <p className="workspace-muted">
        All messages and archived chats. Ctrl+K / ⌘K
      </p>
      {source.opened && <SearchContent source={source} />}
    </Modal>
  );
});
export const SearchContent = observer(function SearchContent({
  source: c,
}: {
  source: SearchSource;
}) {
  const api = useServerApi(),
    store = useServerStore();
  const { get } = api;

  const sourceRequest = useRef(0);
  useEffect(
    () => () => {
      sourceRequest.current++;
    },
    [],
  );
  type SearchResult = GetResult<"/api/search">["results"][number];
  type SearchItem = GetResult<"/api/search/item">;
  type SearchRecord =
    | (SearchResult & { loading: boolean })
    | (SearchItem & { loading: boolean });
  const [source, setSource] = useState<SearchRecord | null>(null),
    [sourceError, setSourceError] = useState("");
  const [query, setQuery] = useState(""),
    [search, setSearch] = useState("");
  const [read, setRead] = useState<ReadSlot<GetResult<"/api/search">> | null>(
    null,
  );
  const current = read?.current;
  const state = {
    data: current ? resourceData(current) : undefined,
    loading: current?.status === "loading",
    error: current?.status === "error" ? errorText(current.error) : "",
  };
  return (
    <>
      <form
        className="workspace-toolbar"
        onSubmit={(e) => {
          e.preventDefault();
          const q = query.trim();
          if (!q) return;
          setSearch(q);
          const next = store.reads.search(api, { q });
          setRead(next);
          void next.read().catch(() => {});
        }}
      >
        <TextInput
          className="workspace-grow"
          autoFocus
          aria-label="Search all conversations"
          placeholder="Search messages, work, and agent chats"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          leftSection={<Search size={16} />}
        />
        <Button
          variant="filled"
          type="submit"
          disabled={!query.trim()}
          loading={state.loading}
        >
          Search
        </Button>
      </form>
      {state.error ? (
        <p role="alert" className="workspace-error">
          {state.error}
        </p>
      ) : state.loading && !state.data ? (
        <Loader size="sm" />
      ) : null}
      {(state.data?.results || []).map((result, index) => (
        <UnstyledButton
          className="workspace-row"
          key={`${result.kind}:${result.id}:${index}`}
          onClick={async () => {
            const request = ++sourceRequest.current;
            setSource({ ...result, loading: true });
            setSourceError("");
            try {
              const record = await get("/api/search/item", {
                query: { id: result.id },
              });
              if (request === sourceRequest.current)
                setSource({ ...result, ...record, loading: false });
            } catch (e) {
              if (request === sourceRequest.current) {
                setSourceError(errorText(e));
                setSource({ ...result, loading: false });
              }
            }
          }}
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
      {search && state.data && !state.data.results?.length && (
        <div className="workspace-empty">No results for “{search}”.</div>
      )}
      {!search && (
        <div className="workspace-empty">
          Searches all messages, archived chats too.
        </div>
      )}
      <Modal
        opened={!!source}
        onClose={() => {
          sourceRequest.current++;
          setSource(null);
        }}
        title="Search source"
        size="lg"
      >
        {source && (
          <>
            <div className="workspace-toolbar">
              <Badge variant="light" color="gray">
                {source.kind ||
                  ("type" in source && typeof source.type === "string"
                    ? source.type
                    : "")}
              </Badge>
              <small className="workspace-muted">{source.id}</small>
            </div>
            {sourceError && (
              <p role="alert" className="workspace-error">
                {sourceError}
              </p>
            )}
            {source.loading ? (
              <Loader size="sm" />
            ) : (
              <p className="workspace-prose">
                {typeof source.text === "string"
                  ? source.text
                  : JSON.stringify(source, null, 2)}
              </p>
            )}
            <Button
              variant="light"
              onClick={() => {
                if (source.kind === "plan") c.navigate("plan", source.agent);
                else {
                  c.select(source.room || source.agent, source.id);
                  c.close();
                }
                setSource(null);
              }}
            >
              Open {source.kind === "plan" ? "plan" : "chat"}
            </Button>
          </>
        )}
      </Modal>
    </>
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
