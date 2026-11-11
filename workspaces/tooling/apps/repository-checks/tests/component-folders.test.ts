import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import {
  checkComponentFolders,
  componentFolderPolicy,
} from "../check-component-folders.ts";
import type { ComponentFolderPolicy } from "../component-folder-policy.ts";

function check(
  files: Record<string, string>,
  policy: ComponentFolderPolicy = { sharedSupport: {}, multiple: {} },
) {
  const root = mkdtempSync(path.join(tmpdir(), "studio-component-folders-"));
  try {
    for (const [name, text] of Object.entries(files)) {
      const file = path.join(root, "components", name);
      mkdirSync(path.dirname(file), { recursive: true });
      writeFileSync(file, text);
    }
    return checkComponentFolders(root, policy);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}
const view = (name: string) => `export function ${name}() { return <div />; }`;
test("valid leaf retains its private component and companions", () => {
  assert.deepEqual(
    check({
      "Button/Button.tsx":
        view("Button") + "\nfunction Icon() { return <svg />; }",
      "Button/Button.render.test.tsx": "",
      "Button/Button.stories.tsx": "",
      "Button/useButton.ts": "export const useButton = () => true;",
      "Button/button.css": "",
      "Button/README.md": "Button",
    }),
    [],
  );
});
test("grouping directories contain only subdirectories", () => {
  assert.deepEqual(check({ "controls/Button/Button.tsx": view("Button") }), []);
});
test("two exported components in one leaf are rejected", () => {
  assert.match(
    check({ "Button/Button.tsx": view("Button") + view("Icon") }).join("\n"),
    /exported components/,
  );
});
test("owned view and section wrappers retain their named component identity", () => {
  assert.deepEqual(
    check({
      "Button/Button.tsx":
        "export default ownedView(() => new State(), function Button() { return <div />; });",
    }),
    [],
  );
  assert.deepEqual(
    check({
      "Button/Button.tsx":
        "export const Button = section(observer(function Button() { return <div />; }));",
    }),
    [],
  );
  assert.match(
    check({
      "Button/Button.tsx":
        "export default ownedView(() => new State(), function Icon() { return <div />; });",
    }).join("\n"),
    /name.*Button/,
  );
});
for (const wrapper of ["ownedView", "section"]) {
  test(`default ${wrapper} preserves a component identifier callback`, () => {
    const call =
      wrapper === "ownedView"
        ? "ownedView(() => new State(), Button)"
        : "section(Button)";
    assert.deepEqual(
      check({
        "Button/Button.tsx": `function Button() { return <div />; } export default ${call};`,
      }),
      [],
    );
  });
  for (const [callback, prelude] of [
    ["render", "function render() { return <div />; }"],
    ["Hidden", "function render() { return <div />; } const Hidden = render;"],
    ["Hidden", 'import { Hidden } from "./hidden-view.tsx";'],
  ])
    test(`default ${wrapper} retains ownership with ${prelude}`, () => {
      const call =
        wrapper === "ownedView"
          ? `ownedView(() => new State(), ${callback})`
          : `section(${callback})`;
      const declaration = `${prelude} export default ${call};`;
      assert.match(
        check({ "support/hidden.tsx": declaration }).join("\n"),
        /support exports a product component/,
      );
      assert.match(
        check({ "../app/hidden.tsx": declaration }).join("\n"),
        /exported component outside src\/components/,
      );
      assert.match(
        check({ "Button/Button.tsx": view("Button") + declaration }).join("\n"),
        /exported components/,
      );
    });
  for (const render of [
    "function render() { return <div />; }",
    "function Icon() { return <div />; }",
    "function RENDER() { return <div />; }",
    "function () { return <div />; }",
    "() => <div />",
    "() => { return <div />; }",
    "() => <><span /></>",
  ]) {
    const call =
      wrapper === "ownedView"
        ? `ownedView(() => new State(), ${render})`
        : `section(${render})`;
    for (const declaration of [
      `export default ${call};`,
      `export const hidden = ${call};`,
    ]) {
      test(`wrapped view cannot hide ownership: ${declaration}`, () => {
        assert.match(
          check({ "support/hidden.tsx": declaration }).join("\n"),
          /support exports a product component/,
        );
        assert.match(
          check({ "../app/hidden.tsx": declaration }).join("\n"),
          /exported component outside src\/components/,
        );
        assert.match(
          check({ "Button/Button.tsx": view("Button") + declaration }).join(
            "\n",
          ),
          /exported components/,
        );
      });
    }
  }
}
test("mixed files and subdirectories are rejected", () => {
  assert.match(
    check({
      "controls/readme.md": "",
      "controls/Button/Button.tsx": view("Button"),
    }).join("\n"),
    /files and subdirectories/,
  );
});
test("component names match their folder", () => {
  assert.match(
    check({ "Button/Button.tsx": view("Icon") }).join("\n"),
    /name.*Button/,
  );
});
test("barrels are rejected", () => {
  assert.match(
    check({
      "Button/Button.tsx": view("Button"),
      "Button/index.ts": 'export { Button } from "./Button.tsx";',
    }).join("\n"),
    /barrel/,
  );
});
test("unlisted support folders are rejected", () => {
  assert.match(
    check({ "controls/helpers/model.ts": "export const value = 1;" }).join(
      "\n",
    ),
    /unlisted support/,
  );
});

test("renderer source follows its declared folder policy", () => {
  const src = new URL("../../../../client/apps/web/src/", import.meta.url);
  assert.deepEqual(checkComponentFolders(src.pathname), []);
});

test("shared private leaves have an explicit exception with its required reason", () => {
  const root = mkdtempSync(path.join(tmpdir(), "studio-component-exception-"));
  try {
    mkdirSync(path.join(root, "components", "Button"), { recursive: true });
    writeFileSync(
      path.join(root, "components", "Button", "Button.tsx"),
      view("Button") + view("Icon"),
    );
    assert.deepEqual(
      checkComponentFolders(root, {
        sharedSupport: {},
        multiple: {
          "components/Button/Button.tsx": "exports share private leaves",
        },
      }),
      [],
    );
    assert.match(
      checkComponentFolders(root, {
        sharedSupport: {},
        multiple: { "components/Button/Button.tsx": "unrelated reason" },
      }).join("\n"),
      /exported components/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("a private companion cannot be imported by another product component", () => {
  assert.match(
    check({
      "Button/Button.tsx": view("Button"),
      "Button/model.ts": "export const value = 1;",
      "Icon/Icon.tsx":
        'import { value } from "../Button/model.ts"; export function Icon() { return <span>{value}</span>; }',
    }).join("\n"),
    /private companion.*shared/,
  );
});

test("conventional feature support accepts helpers and rejects product components", () => {
  const root = mkdtempSync(path.join(tmpdir(), "studio-component-support-"));
  try {
    const support = path.join(root, "components", "controls", "support");
    mkdirSync(support, { recursive: true });
    writeFileSync(path.join(support, "model.ts"), "export const value = 1;");
    const policy = { sharedSupport: {}, multiple: {} };
    assert.deepEqual(checkComponentFolders(root, policy), []);
    writeFileSync(path.join(support, "Icon.tsx"), view("Icon"));
    assert.match(
      checkComponentFolders(root, policy).join("\n"),
      /support exports a product component/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("an extra arrow component returning null is rejected", () => {
  assert.match(
    check({
      "Button/Button.tsx": view("Button") + "export const Other = () => null;",
    }).join("\n"),
    /exported components/,
  );
});
test("a private JSX component cannot hide in a companion", () => {
  assert.match(
    check({
      "Button/Button.tsx": view("Button"),
      "Button/helper.tsx":
        "function Other() { return <span />; } export const value = 1;",
    }).join("\n"),
    /component.*primary/,
  );
});
test("another product component cannot import private CSS", () => {
  assert.match(
    check({
      "Button/Button.tsx": view("Button"),
      "Button/button.css": "",
      "Icon/Icon.tsx": 'import "../Button/button.css";' + view("Icon"),
    }).join("\n"),
    /private companion.*shared/,
  );
});
test("exported components outside the component tree are rejected", () => {
  assert.match(
    check({ "../Outside.tsx": view("Outside") }).join("\n"),
    /outside.*components/,
  );
});

test("an extra default arrow component is rejected", () => {
  assert.match(
    check({
      "Button/Button.tsx": view("Button") + "export default () => null;",
    }).join("\n"),
    /exported components/,
  );
});
test("nested private components cannot hide in a companion", () => {
  assert.match(
    check({
      "Button/Button.tsx": view("Button"),
      "Button/helper.tsx":
        "export function make() { function Inner() { return <span />; } return Inner; }",
    }).join("\n"),
    /component.*primary/,
  );
});
test("support imported from outside its group is rejected", () => {
  assert.match(
    check({
      "controls/support/model.ts": "export const value = 1;",
      "Icon/Icon.tsx":
        'import { value } from "../controls/support/model.ts"; export function Icon() { return <span>{value}</span>; }',
    }).join("\n"),
    /outside support group/,
  );
});

test("the stylesheet order owner has one narrow import-only allowance", () => {
  const files = {
    "Button/Button.tsx": view("Button"),
    "Button/button.css": "",
    "../rendererStyles.ts": 'import "./components/Button/button.css";',
  };
  const policy = {
    sharedSupport: {},
    multiple: {},
    cssOrderOwner: "rendererStyles.ts",
  };
  assert.deepEqual(check(files, policy), []);
  assert.match(
    check(
      {
        ...files,
        "../rendererStyles.ts":
          files["../rendererStyles.ts"] + "export const value = 1;",
      },
      policy,
    ).join("\n"),
    /only CSS imports/,
  );
});
test("type-only support cannot load a stylesheet", () => {
  assert.match(
    check(
      {
        "controls/support/props.ts":
          'import "./props.css"; export type Props = { name: string };',
        "controls/support/props.css": "",
      },
      { sharedSupport: {}, multiple: {} },
    ).join("\n"),
    /type-only support.*cannot load styles/,
  );
});

test("support is available to sibling components and named shared APIs", () => {
  assert.deepEqual(
    check({
      "controls/support/model.ts": "export const value = 1;",
      "controls/Button/Button.tsx":
        'import { value } from "../support/model.ts";' + view("Button"),
    }),
    [],
  );
  assert.deepEqual(
    check(
      {
        "controls/support/model.ts": "export const value = 1;",
        "Icon/Icon.tsx":
          'import { value } from "../controls/support/model.ts";' +
          view("Icon"),
      },
      {
        sharedSupport: {
          "components/controls/support": "Shared controls contract.",
        },
        multiple: {},
      },
    ),
    [],
  );
});
test("integration witnesses can import support across product groups", () => {
  assert.deepEqual(
    check({
      "controls/support/model.ts": "export const value = 1;",
      "../outside.test.ts":
        'import { value } from "./components/controls/support/model.ts"; void value;',
    }),
    [],
  );
});

test("a parenthesized arrow component is still a component", () => {
  assert.match(
    check({
      "Button/Button.tsx":
        view("Button") + "export const Other = (() => null);",
    }).join("\n"),
    /exported components/,
  );
});
test("a satisfies-wrapped arrow component is still a component", () => {
  assert.match(
    check({
      "Button/Button.tsx":
        view("Button") +
        "export const Other = (() => null) satisfies React.FC;",
    }).join("\n"),
    /exported components/,
  );
});
test("PureComponent declarations count alongside function components", () => {
  assert.match(
    check({
      "Button/Button.tsx":
        view("Button") +
        "export class Other extends React.PureComponent { render() { return null; } }",
    }).join("\n"),
    /exported components/,
  );
});
test("shared APIs do not exempt future private feature support", () => {
  assert.match(
    check(
      {
        "accounts/new-private-feature/support/internal.ts":
          "export const value = 1;",
        "Other/Other.tsx":
          'import { value } from "../accounts/new-private-feature/support/internal.ts";' +
          view("Other"),
      },
      { sharedSupport: componentFolderPolicy.sharedSupport, multiple: {} },
    ).join("\n"),
    /outside support group/,
  );
});

test("a component cannot load the entry-point stylesheet owner", () => {
  assert.match(
    check(
      {
        "Button/Button.tsx":
          'import "../../rendererStyles.ts";' + view("Button"),
        "Button/button.css": "",
        "../rendererStyles.ts": 'import "./components/Button/button.css";',
      },
      { sharedSupport: {}, multiple: {}, cssOrderOwner: "rendererStyles.ts" },
    ).join("\n"),
    /stylesheet owner.*entry point/,
  );
});
test("an as-wrapped arrow component is still a component", () => {
  assert.match(
    check({
      "Button/Button.tsx":
        view("Button") + "export const Other = (() => null) as React.FC;",
    }).join("\n"),
    /exported components/,
  );
});

test("wrapped React class expressions count as components", () => {
  assert.match(
    check({
      "Button/Button.tsx":
        view("Button") +
        "export const Other = (class extends React.PureComponent { render() { return null; } });",
    }).join("\n"),
    /exported components/,
  );
});
