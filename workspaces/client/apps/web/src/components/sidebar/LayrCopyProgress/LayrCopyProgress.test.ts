import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, test } from "vitest";
import LayrCopyProgress from "./LayrCopyProgress.tsx";

test("renders server phases with determinate and indeterminate progress", () => {
  const known = renderToStaticMarkup(
    createElement(LayrCopyProgress, {
      progress: { phase: "staging", done: 2, total: 4, unit: "files" },
    }),
  );
  expect(known).toContain("Copying files");
  expect(known).toContain('value="50"');
  const unknown = renderToStaticMarkup(
    createElement(LayrCopyProgress, {
      progress: { phase: "importing_commits", done: 50, unit: "commits" },
    }),
  );
  expect(unknown).toContain("Importing history");
  expect(unknown).not.toContain("value=");
});

test("renders line creation and workspace copy phases", () => {
  for (const [phase, label] of [
    ["creating_line", "Creating workspace line"],
    ["archiving", "Building archive"],
    ["uploading", "Uploading archive"],
    ["importing_project", "Importing project"],
  ] as const) {
    const markup = renderToStaticMarkup(
      createElement(LayrCopyProgress, {
        progress: { phase, done: 0, unit: "bytes" },
      }),
    );
    expect(markup).toContain(label);
    expect(markup).not.toContain("value=");
  }
  const upload = renderToStaticMarkup(
    createElement(LayrCopyProgress, {
      progress: { phase: "uploading", done: 512, total: 1024, unit: "bytes" },
    }),
  );
  expect(upload).toContain("512 bytes");
  expect(upload).toContain('value="50"');
});
