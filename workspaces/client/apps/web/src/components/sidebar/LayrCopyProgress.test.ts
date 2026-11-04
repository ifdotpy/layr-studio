import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, test } from "vitest";
import LayrCopyProgress from "./LayrCopyProgress";

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
