import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { describe, expect, it } from "vitest";
import type { Message } from "../../../../types.ts";
import { ToolCard } from "./Activity.tsx";
import NativeMarkdown from "../../markdown/NativeMarkdown/NativeMarkdown.tsx";
import fixtures from "../../../../../../../../providers/fixtures/native-progress.json";

describe("native tool progress", () => {
  for (const provider of ["codex", "claude"]) {
    it(`renders ${provider} progress on the collapsed tool card`, () => {
      const item = {
        id: "agent:tool",
        role: "output",
        toolStatus: "running",
        text: JSON.stringify({
          id: "tool",
          type: "mcpToolCall",
          server: provider,
          tool: "search",
          status: "inProgress",
          progressMessage: "Reading documentation",
        }),
      } satisfies Message;
      const html = renderToStaticMarkup(
        createElement(MantineProvider, null, createElement(ToolCard, { item })),
      );
      expect(html).toContain("Reading documentation");
      expect(html).toContain('data-tool-status="running"');
      expect(html).toContain("search");
    });
  }
  for (const fixture of fixtures) {
    for (const row of fixture.rows) {
      it(`renders the shared projection of ${fixture.provider} ${row.id}`, () => {
        const item = {
          id: row.id,
          role: row.kind === "tool" ? "output" : "assistant",
          text: row.payload ? JSON.stringify(row.payload) : row.text,
          toolStatus: row.kind === "tool" ? "running" : undefined,
          streaming: true,
        } satisfies Message;
        const html = renderToStaticMarkup(
          createElement(
            MantineProvider,
            null,
            row.payload
              ? createElement(ToolCard, { item })
              : createElement(NativeMarkdown, {
                  text: item.text,
                  streaming: true,
                }),
          ),
        );
        expect(html).toContain(row.payload?.progressMessage || row.text);
      });
    }
  }
});
