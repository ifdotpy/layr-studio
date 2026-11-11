import { WorkspaceResourceState as ResourceState } from "../WorkspaceResourceState/WorkspaceResourceState.tsx";
import { Badge, TextInput } from "@mantine/core";
import { Search, SlidersHorizontal, Wrench } from "lucide-react";
import { observer } from "mobx-react-lite";
import { useState } from "react";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";
import type { JsonValue } from "../../../types.ts";
import ErrorDescription from "../ErrorDescription/ErrorDescription.tsx";
import { isRecord, McpInventory } from "../McpInventory/McpInventory.tsx";
import type { Context } from "../Workspace/Workspace.tsx";

export const WorkspaceTools = observer(function WorkspaceTools(c: Context) {
  const { state, inventory } = useServerAreas().workspace.capabilities(
    c.selected?.id ?? "",
  );
  const [query, setQuery] = useState("");
  const managedTools = (state.data?.managed ?? []).filter((tool) =>
    isRecord(tool),
  );
  const observedNative = state.data?.observedNative ?? [];
  return (
    <>
      <ResourceState state={state} />
      <TextInput
        aria-label="Filter tools"
        placeholder="Filter tools"
        leftSection={<Search size={15} />}
        value={query}
        onChange={(e) => setQuery(e.target.value)}
      />
      {!!state.data?.errors?.length && (
        <details className="workspace-error">
          <summary>Some tools could not load. View details.</summary>
          {state.data.errors.map((error: unknown, i: number) => (
            <ErrorDescription key={i} value={error} />
          ))}
        </details>
      )}
      {state.data && (
        <>
          <h3 className="workspace-section-title">Orchestration tools</h3>
          <div className="workspace-tools">
            {managedTools
              .filter((tool) =>
                JSON.stringify(tool)
                  .toLowerCase()
                  .includes(query.toLowerCase()),
              )
              .map((tool, index) => (
                <details
                  key={`${inventoryName(tool) || "tool"}:${index}`}
                  className="workspace-tool"
                >
                  <summary>
                    <Wrench size={14} />
                    <strong>
                      {inventoryName(tool) || `Tool ${index + 1}`}
                    </strong>
                    <span
                      className="workspace-tool-description"
                      title={
                        typeof tool.description === "string"
                          ? tool.description
                          : undefined
                      }
                    >
                      {typeof tool.description === "string"
                        ? tool.description
                        : ""}
                    </span>
                  </summary>
                  <pre className="workspace-code">
                    {JSON.stringify(
                      tool.inputSchema ?? tool.parameters,
                      null,
                      2,
                    )}
                  </pre>
                </details>
              ))}
          </div>
          <h3 className="workspace-section-title">Observed native tools</h3>
          <p className="workspace-muted">{state.data.nativeInventory}</p>
          <div className="workspace-actions">
            {observedNative
              .filter((name) =>
                name.toLowerCase().includes(query.toLowerCase()),
              )
              .map((name: string) => (
                <Badge color="gray" variant="light" key={name}>
                  {name}
                </Badge>
              ))}
          </div>
          {!(state.data.observedNative || []).length && (
            <p className="workspace-muted">
              No native tool calls recorded for this agent.
            </p>
          )}
          <h3 className="workspace-section-title">Skills</h3>
          <Inventory value={state.data.skills} query={query} />
          <h3 className="workspace-section-title">Connected tools (MCP)</h3>
          <McpInventory key={c.selected?.id} state={inventory} query={query} />
        </>
      )}
    </>
  );
});

function Inventory({ value, query }: { value: JsonValue[]; query: string }) {
  if (!value.length)
    return <p className="workspace-muted">No entries returned by Codex.</p>;
  return (
    <div className="workspace-tools">
      {value
        .filter((entry: unknown) =>
          JSON.stringify(entry)?.toLowerCase().includes(query.toLowerCase()),
        )
        .map((entry: unknown, index: number) => (
          <details className="workspace-tool" key={index}>
            <summary>
              <SlidersHorizontal size={14} />
              <strong>{inventoryName(entry) || `Entry ${index + 1}`}</strong>
            </summary>
            <pre className="workspace-code">
              {JSON.stringify(entry, null, 2) ?? String(entry)}
            </pre>
          </details>
        ))}
    </div>
  );
}

function inventoryName(value: unknown): string | undefined {
  if (!isRecord(value)) return;
  return [value.name, value.cwd, value.serverName].find(
    (candidate): candidate is string =>
      typeof candidate === "string" && !!candidate,
  );
}
