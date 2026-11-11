// @vitest-environment happy-dom
import { StudioErrorsProvider } from "../../conversation/notices/support/noticesFixture.tsx";
import { StudioErrorLevel } from "../../../generated/api.ts";
import { appendAppLog, clearAppLog } from "../../../appLog.ts";
import { StudioErrorEffectCertainty as GeneratedApiStudioErrorEffectCertainty } from "../../../generated/api.ts";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { expect, test } from "vitest";
import {
  StudioErrorList,
  actionLabel,
  useStudioErrors,
  type StudioErrorItem,
} from "./StudioErrorList.tsx";

const lost: StudioErrorItem = {
  id: "monitor:watch",
  kind: "monitor_outcome_lost",
  level: StudioErrorLevel.error,
  reason:
    "Studio lost the result of this command. The command may have run. Studio does not run it again.",
  action: "check",
  agentId: "worker",
  agentName: "Fixture worker",
  monitorId: "watch",
  eventId: null,
  detail:
    "Command outcome unknown after losing the provider result; not rerun.",
  subject: "cargo test",
  ruleId: null,
  count: 1,
  firstAt: 1_792_700_000,
  at: 1_792_700_000,
  diagnostics: {
    context: {
      source: "monitor",
      requestId: "original-command",
      turnId: null,
      epoch: 0,
    },
    certainty: GeneratedApiStudioErrorEffectCertainty.unknown,
    causes: [],
  },
};

const disconnected: StudioErrorItem = {
  ...lost,
  id: "agent:worker:turn",
  kind: "turn_disconnected",
  level: StudioErrorLevel.error,
  reason: "The provider connection ended during the turn.",
  action: "resume",
  monitorId: null,
  subject: "",
};

function render(items: StudioErrorItem[]) {
  return renderToStaticMarkup(
    createElement(
      MantineProvider,
      null,
      createElement(
        StudioErrorsProvider,
        { items },
        createElement(StudioErrorList),
      ),
    ),
  );
}

test("each row shows its reason and one safe action", () => {
  const markup = render([lost, disconnected]);
  expect(markup).toContain("Fixture worker");
  expect(markup).toContain("Studio does not run it again.");
  expect(markup).toContain("cargo test");
  expect(markup).toContain('data-kind="monitor_outcome_lost"');
  expect(markup).toContain("Delete selected");
  expect(actionLabel(lost)).toBe("Open");
  expect(actionLabel(disconnected)).toBe("Resume");
});

test("a repeated failure shows its count and time range once", () => {
  const repeated: StudioErrorItem = {
    ...lost,
    id: "monitors:rule:watch:monitor_not_recorded",
    kind: "monitor_not_recorded",
    ruleId: "watch",
    count: 74,
    firstAt: 1_792_600_000,
  };
  const markup = render([repeated]);
  expect(markup).toContain("74 times");
  expect(markup).toContain(new Date(1_792_600_000 * 1000).toLocaleString());
  expect(markup).toContain(new Date(1_792_700_000 * 1000).toLocaleString());
  expect(render([lost])).not.toContain("times");
});

test("the full diagnostic record is available in expandable details", () => {
  const markup = render([lost]);
  expect(markup).toContain("<details");
  expect(markup).toContain("<summary>Details</summary>");
  expect(markup).toContain(lost.id);
  expect(markup).toContain("<pre");
});

test("server incidents expose details without inventing an agent action", () => {
  const incident: StudioErrorItem = {
    id: "http:failed-read",
    kind: "http_failure",
    level: StudioErrorLevel.error,
    action: "details",
    reason: "The server could not complete this request.",
    detail: "store_data",
    subject: "GET /api/monitor/log",
    at: 1,
    diagnostics: {
      context: {
        source: "http",
        route: "GET /api/monitor/log",
        requestId: "exact-read",
        status: 503,
        class: "store_data",
      },
      certainty: GeneratedApiStudioErrorEffectCertainty.read_only,
      causes: [
        {
          kind: "stored_record",
          object: "monitors",
          id: "monitor",
          pointer: "/log",
        },
      ],
    },
  };
  const markup = render([incident]);
  expect(markup).toContain("Select error http:failed-read for Server");
  expect(markup).toContain("exact-read");
  expect(markup).toContain("/log");
  expect(markup).not.toContain("agentId");
  expect(markup).not.toContain(">Open<");
  expect(markup).not.toContain(">Resume<");
});

test("an empty list says that nothing needs the owner", () => {
  expect(render([])).toContain(
    "No agent work ended because of a Studio error.",
  );
});

test("rows provide named checkboxes and bulk controls", () => {
  const markup = render([lost, disconnected]);
  expect(markup).toContain('type="checkbox"');
  expect(markup).toContain(`Select error ${lost.id} for Fixture worker`);
  expect(markup).toContain("Select all");
  expect(markup).toMatch(/<button[^>]*disabled[^>]*>[^]*?Delete selected/);
});

// The warning owner must start from the stream projection, with no HTTP hydration.
test("the server-scoped warning owner consumes streamed error rows", () => {
  function Count() {
    return createElement(
      "span",
      null,
      `Streamed errors: ${useStudioErrors().items.length}`,
    );
  }
  const markup = renderToStaticMarkup(
    createElement(
      StudioErrorsProvider,
      { items: [lost, disconnected] },
      createElement(Count),
    ),
  );
  expect(markup).toContain("Streamed errors: 2");
});

test("levels are counted and displayed in severity order", () => {
  const warning: StudioErrorItem = {
    ...lost,
    id: "warning",
    level: StudioErrorLevel.warning,
    at: 9,
  };
  const critical: StudioErrorItem = {
    ...lost,
    id: "critical",
    level: StudioErrorLevel.critical,
    at: 1,
  };
  const markup = render([warning, critical, lost]);
  expect(markup).toContain("Critical (1)");
  expect(markup).toContain("Error (1)");
  expect(markup).toContain("Warning (1)");
  expect(markup.indexOf('data-kind="monitor_outcome_lost"')).toBeGreaterThan(
    -1,
  );
  expect(markup.indexOf("Select error critical")).toBeLessThan(
    markup.indexOf("Select error warning"),
  );
});

test("local logs share the list without a dismissal checkbox", () => {
  appendAppLog({
    at: 1234,
    title: "Reconnected",
    reason: "Connection restored",
  });
  try {
    const markup = render([]);
    expect(markup).toContain("Log (1)");
    expect(markup).toContain("Reconnected");
    expect(markup).toContain("Clear local logs");
    expect(markup).not.toContain('type="checkbox"');
  } finally {
    clearAppLog();
  }
});

test("local transfer failures appear as errors", () => {
  appendAppLog({
    at: 1,
    level: "error",
    title: "Account transfer blocked",
    reason: "Worker: Receipt missing",
  });
  try {
    const markup = render([]);
    expect(markup).toContain("Error (1)");
    expect(markup).toContain("Worker: Receipt missing");
  } finally {
    clearAppLog();
  }
});

test("the real error leaf preserves actions, selection and the dismissal cap", async () => {
  const { act } = await import("react");
  const { createRoot } = await import("react-dom/client");
  const { vi } = await import("vitest");
  const { conversationActionsFixture, observableFixture } =
    await import("../integration-support/observableFixture.tsx");
  const { ServerAreasContextProvider: ConversationProvider } =
    await import("../../servers/ServerServicesProvider/ServerServicesProvider.tsx");
  const { STUDIO_ERRORS_DISMISS_MAX_IDS: cap } =
    await import("../../../generated/sharedValues.ts");
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const open = vi.fn();
  const models = observableFixture();
  const areas = conversationActionsFixture(open, models.store);
  const post = vi.fn().mockResolvedValue({ unknownIds: ["already-gone"] });
  const get = vi.fn().mockResolvedValue({ items: [] });
  areas.scope.services.api.post = post;
  areas.scope.services.api.get = get;
  models.apply(
    [
      disconnected,
      ...Array.from({ length: cap }, (_, index) => ({
        ...lost,
        id: `error-${index}`,
      })),
    ].map((value) => ({ collection: "studioError", id: value.id, value })),
  );
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  const button = (text: string) => {
    const found = [...host.querySelectorAll("button")].find(
      (node) => node.textContent === text,
    );
    if (!found) throw new Error(`Missing button: ${text}`);
    return found;
  };
  try {
    await act(async () =>
      root.render(
        createElement(
          MantineProvider,
          null,
          createElement(
            ConversationProvider,
            { value: areas },
            createElement(StudioErrorList),
          ),
        ),
      ),
    );
    await act(async () => button("Open").click());
    expect(open).toHaveBeenCalledWith(lost.agentId);
    await act(async () => button("Resume").click());
    expect(post).toHaveBeenCalledWith("/api/connection-recovery", {
      id: disconnected.agentId,
    });
    expect(get).toHaveBeenCalledWith("/api/studio-errors");
    await act(async () => button("Select all").click());
    expect(host.textContent).toContain(`${cap} selected`);
    expect(
      host.querySelectorAll('input[type="checkbox"]:checked'),
    ).toHaveLength(cap);
    expect(
      host.querySelectorAll('input[type="checkbox"]:disabled'),
    ).toHaveLength(1);
    await act(async () => button("Delete selected").click());
    const dismissal = post.mock.calls.find(
      ([path]) => path === "/api/studio-errors/dismiss",
    );
    expect(dismissal?.[1].ids).toHaveLength(cap);
    expect(host.textContent).toContain("0 selected");
    expect(host.textContent).toContain("1 selected errors were already gone.");
  } finally {
    await act(async () => root.unmount());
    host.remove();
    areas.scope.services.close();
    areas.scope.store.dispose();
    vi.unstubAllGlobals();
  }
});
