import { Button, MantineProvider } from "@mantine/core";
import {
  Children,
  createElement,
  isValidElement,
  type ReactElement,
  type ReactNode,
} from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { theme } from "../../../theme.ts";
import type { Agent } from "../../../types.ts";
import ProjectFolderDialog from "./ProjectFolderDialog.tsx";

const agent = (values: Partial<Agent>) =>
  ({
    id: "chat",
    isLead: true,
    cwd: "/Users/me/app",
    projectPath: "/Users/me/app",
    ...values,
  }) as Agent;

const render = (element: ReactElement) =>
  renderToStaticMarkup(createElement(MantineProvider, { theme }, element));

function buttons(node: ReactNode): ReactElement<{ onClick: () => void }>[] {
  if (!isValidElement<{ children?: ReactNode }>(node)) return [];
  if (node.type === Button)
    return [node as ReactElement<{ onClick: () => void }>];
  return Children.toArray(node.props.children).flatMap(buttons);
}

describe("Project folder dialog", () => {
  it("keeps the native folder and its folder button", () => {
    const onReveal = vi.fn();
    const native = agent({ executionMode: "native" });
    const markup = render(
      createElement(ProjectFolderDialog, {
        agent: native,
        canReveal: true,
        onReveal,
      }),
    );
    expect(markup).toContain("<p>/Users/me/app</p>");
    expect(markup).toContain("To use another folder, start a new chat.");
    expect(markup).toContain("Show folder");
    const [button] = buttons(
      ProjectFolderDialog({ agent: native, canReveal: true, onReveal }),
    );
    button!.props.onClick();
    expect(onReveal).toHaveBeenCalledWith("/Users/me/app");

    expect(
      render(
        createElement(ProjectFolderDialog, {
          agent: agent({}),
          canReveal: false,
          onReveal,
        }),
      ),
    ).not.toContain("Show folder");
  });

  it("does not offer a folder for a legacy VM chat", () => {
    const markup = render(
      createElement(ProjectFolderDialog, {
        agent: agent({
          legacyVmWorkspace: true,
          projectPath: "/guest/path",
        }),
        canReveal: true,
        onReveal: vi.fn(),
      }),
    );
    expect(markup).toContain("folder is unavailable");
    expect(markup).not.toContain("/guest/path");
    expect(markup).not.toContain("Show folder");
  });
});
