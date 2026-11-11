import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import ts from "typescript";

import {
  componentFolderPolicy,
  type ComponentFolderPolicy as Policy,
} from "./component-folder-policy.ts";
export { componentFolderPolicy } from "./component-folder-policy.ts";

function unwrap(expression: ts.Expression): ts.Expression {
  while (
    ts.isParenthesizedExpression(expression) ||
    ts.isAsExpression(expression) ||
    ts.isSatisfiesExpression(expression) ||
    ts.isTypeAssertionExpression(expression) ||
    ts.isNonNullExpression(expression)
  )
    expression = expression.expression;
  return expression;
}

function wrappedView(expression: ts.Expression): boolean {
  const call = unwrap(expression);
  if (!ts.isCallExpression(call)) return false;
  const wrapper = call.expression.getText();
  const render = /(?:^|\.)ownedView$/.test(wrapper)
    ? call.arguments[1]
    : /(?:^|\.)section$/.test(wrapper)
      ? call.arguments[0]
      : undefined;
  if (!render) return false;
  const containsJsx = (node: ts.Node): boolean =>
    ts.isJsxElement(node) ||
    ts.isJsxSelfClosingElement(node) ||
    ts.isJsxFragment(node) ||
    !!ts.forEachChild(node, containsJsx);
  return containsJsx(render);
}

function componentNames(
  file: string,
  text: string,
  exportedOnly: boolean,
): string[] {
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);
  const declarations = new Map<string, ts.Node>();
  const aliases = new Map<string, string>();
  const exported = new Set<string>();
  const wrapped = new Set<string>();
  let wrappedCallback: ts.Identifier | undefined;
  const isPublic = (node: ts.Node) =>
    ts.canHaveModifiers(node) &&
    ts.getModifiers(node)?.some((m) => m.kind === ts.SyntaxKind.ExportKeyword);
  for (const node of source.statements) {
    if (ts.isFunctionDeclaration(node) || ts.isClassDeclaration(node)) {
      const name =
        node.name?.text ?? (isPublic(node) ? "__default" : undefined);
      if (name) {
        declarations.set(name, node);
        if (isPublic(node)) exported.add(name);
      }
    }
    if (ts.isVariableStatement(node))
      for (const declaration of node.declarationList.declarations) {
        if (!ts.isIdentifier(declaration.name)) continue;
        const name = declaration.name.text;
        declarations.set(name, declaration);
        if (declaration.initializer && wrappedView(declaration.initializer))
          wrapped.add(name);
        if (isPublic(node)) exported.add(name);
        if (
          declaration.initializer &&
          ts.isIdentifier(unwrap(declaration.initializer))
        )
          aliases.set(name, unwrap(declaration.initializer).getText(source));
      }
    if (ts.isExportAssignment(node)) {
      let expression: ts.Expression = unwrap(node.expression);
      let wrappedDefault = false;
      while (ts.isCallExpression(expression) && expression.arguments[0]) {
        wrappedDefault ||= /(?:^|\.)(?:ownedView|section)$/.test(
          expression.expression.getText(),
        );
        expression = unwrap(
          /(?:^|\.)ownedView$/.test(expression.expression.getText())
            ? (expression.arguments[1] ?? expression.arguments[0])
            : expression.arguments[0],
        );
      }
      if (ts.isIdentifier(expression) && wrappedDefault)
        wrappedCallback = expression;
      else if (ts.isIdentifier(expression)) exported.add(expression.text);
      else {
        const callback = ts.isFunctionExpression(expression)
          ? expression.name?.text
          : undefined;
        const name =
          callback && /^[A-Z]/.test(callback) && /[a-z]/.test(callback)
            ? callback
            : "__default";
        declarations.set(name, expression);
        exported.add(name);
      }
    }
    if (
      ts.isExportDeclaration(node) &&
      !node.moduleSpecifier &&
      node.exportClause &&
      ts.isNamedExports(node.exportClause)
    )
      for (const entry of node.exportClause.elements)
        exported.add(entry.propertyName?.text ?? entry.name.text);
  }
  if (wrappedCallback) {
    // A wrapped default keeps the callback's name only when it resolves to a
    // local PascalCase declaration; otherwise the wrapper itself is the view.
    let target = wrappedCallback.text;
    const seen = new Set<string>();
    while (aliases.has(target) && !seen.has(target)) {
      seen.add(target);
      target = aliases.get(target) ?? target;
    }
    if (
      /^[A-Z]/.test(target) &&
      /[a-z]/.test(target) &&
      declarations.has(target)
    )
      exported.add(wrappedCallback.text);
    else {
      declarations.set("__default", wrappedCallback);
      wrapped.add("__default");
      exported.add("__default");
    }
  }
  if (!exportedOnly) {
    const collect = (node: ts.Node) => {
      if (
        (ts.isFunctionDeclaration(node) || ts.isClassDeclaration(node)) &&
        node.name
      )
        declarations.set(node.name.text, node);
      if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name)) {
        declarations.set(node.name.text, node);
        if (node.initializer && wrappedView(node.initializer))
          wrapped.add(node.name.text);
      }
      ts.forEachChild(node, collect);
    };
    ts.forEachChild(source, collect);
  }
  const names = new Set<string>();
  for (let name of exportedOnly ? exported : declarations.keys()) {
    const seen = new Set<string>();
    while (aliases.has(name) && !seen.has(name)) {
      seen.add(name);
      name = aliases.get(name) ?? name;
    }
    if (
      name !== "__default" &&
      !wrapped.has(name) &&
      (!/^[A-Z]/.test(name) || !/[a-z]/.test(name))
    )
      continue;
    const declaration = declarations.get(name);
    if (!declaration) continue;
    if (ts.isClassDeclaration(declaration)) {
      if (
        declaration.heritageClauses?.some((c) =>
          /\b(?:Component|PureComponent)\b/.test(c.getText()),
        )
      )
        names.add(name);
      continue;
    }
    const value = ts.isVariableDeclaration(declaration)
      ? declaration.initializer && unwrap(declaration.initializer)
      : declaration;
    if (
      value &&
      (wrapped.has(name) ||
        ts.isFunctionDeclaration(value) ||
        ts.isFunctionExpression(value) ||
        ts.isArrowFunction(value) ||
        (ts.isClassExpression(value) &&
          value.heritageClauses?.some((clause) =>
            /\b(?:Component|PureComponent)\b/.test(clause.getText()),
          )) ||
        (ts.isCallExpression(value) &&
          (wrappedView(value) ||
            /(?:^|\.)(?:observer|memo|forwardRef|section)$/.test(
              value.expression.getText(),
            ))))
    )
      names.add(name);
  }
  return [...names].sort();
}

export function exportedComponents(file: string, text: string): string[] {
  return componentNames(file, text, true);
}

function isSupportDirectory(directory: string): boolean {
  const name = path.basename(directory);
  return name === "support" || name.endsWith("-support");
}

export function checkComponentFolders(
  src: string,
  policy: Policy = componentFolderPolicy,
): string[] {
  const errors: string[] = [];
  const relative = (file: string) =>
    path.relative(src, file).split(path.sep).join("/");
  const sources: string[] = [];
  function scan(directory: string) {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const file = path.join(directory, entry.name);
      if (entry.isDirectory()) scan(file);
      else if (/\.(?:ts|tsx)$/.test(entry.name)) sources.push(file);
    }
  }
  scan(src);
  for (const file of sources) {
    const source = ts.createSourceFile(
      file,
      readFileSync(file, "utf8"),
      ts.ScriptTarget.Latest,
      true,
    );
    if (
      path.basename(file) === "index.ts" ||
      (source.statements.length > 0 &&
        source.statements.every(
          (n) => ts.isExportDeclaration(n) && n.moduleSpecifier,
        ))
    )
      errors.push(`${relative(file)}: barrel; import its target directly`);
  }
  for (const file of sources) {
    if (
      /\.(?:test|spec|stories|fixture|test-entry)\.|Fixture|\/generated\/|\.d\.ts$/.test(
        file,
      )
    )
      continue;
    if (
      !relative(file).startsWith("components/") &&
      exportedComponents(file, readFileSync(file, "utf8")).length
    )
      errors.push(
        `${relative(file)}: exported component outside src/components`,
      );
  }
  if (policy.cssOrderOwner) {
    const owner = path.join(src, policy.cssOrderOwner);
    if (!existsSync(owner))
      errors.push(`${policy.cssOrderOwner}: missing stylesheet order owner`);
    else {
      const source = ts.createSourceFile(
        owner,
        readFileSync(owner, "utf8"),
        ts.ScriptTarget.Latest,
        true,
      );
      if (
        !source.statements.every(
          (node) =>
            ts.isImportDeclaration(node) &&
            !node.importClause &&
            ts.isStringLiteral(node.moduleSpecifier) &&
            node.moduleSpecifier.text.endsWith(".css"),
        )
      )
        errors.push(
          `${policy.cssOrderOwner}: stylesheet order owner must contain only CSS imports`,
        );
    }
  }
  // A leaf's implementation companions cannot become dependencies of other product owners.
  for (const file of sources) {
    if (
      !isSupportDirectory(path.dirname(file)) ||
      /\.(?:test|spec|stories|fixture|test-entry)\.|Fixture/.test(file)
    )
      continue;
    const source = ts.createSourceFile(
      file,
      readFileSync(file, "utf8"),
      ts.ScriptTarget.Latest,
      true,
    );
    const typeOnly = source.statements.every(
      (node) =>
        ts.isImportDeclaration(node) ||
        ts.isTypeAliasDeclaration(node) ||
        ts.isInterfaceDeclaration(node) ||
        (ts.isExportDeclaration(node) && node.isTypeOnly),
    );
    if (
      typeOnly &&
      source.statements.some(
        (node) =>
          ts.isImportDeclaration(node) && !node.importClause?.isTypeOnly,
      )
    )
      errors.push(
        `${relative(file)}: type-only support must use declaration-level type imports and cannot load styles`,
      );
  }
  for (const file of sources) {
    const testSupport =
      /\.(?:test|spec|stories|fixture|test-entry)\.|Fixture|\/generated\//.test(
        file,
      );
    const source = ts.createSourceFile(
      file,
      readFileSync(file, "utf8"),
      ts.ScriptTarget.Latest,
      true,
    );
    const inspect = (node: ts.Node) => {
      if (
        ts.isStringLiteralLike(node) &&
        (node.text.startsWith(".") || node.text.startsWith("/src/"))
      ) {
        const unresolved = node.text.startsWith("/src/")
          ? path.join(src, node.text.slice(5))
          : path.resolve(path.dirname(file), node.text);
        const target = [
          unresolved,
          unresolved + ".ts",
          unresolved + ".tsx",
          unresolved + ".css",
        ].find((candidate) => existsSync(candidate));
        if (
          target &&
          !testSupport &&
          isSupportDirectory(path.dirname(target))
        ) {
          const group = path.dirname(path.dirname(target));
          const local = file.startsWith(group + path.sep);
          const shared = Object.keys(policy.sharedSupport).some(
            (api) =>
              relative(target) === api ||
              relative(path.dirname(target)) === api,
          );
          const styleOwner =
            target.endsWith(".css") && relative(file) === policy.cssOrderOwner;
          if (!local && !shared && !styleOwner)
            errors.push(
              `${relative(file)}: import from outside support group ${relative(group)} (${relative(target)})`,
            );
        }
        if (
          target &&
          relative(target) === policy.cssOrderOwner &&
          relative(file) !== "mount.tsx" &&
          !/(?:[.-]fixture|\.test-entry)\.tsx?$/.test(file)
        )
          errors.push(
            `${relative(file)}: stylesheet owner belongs at a renderer or fixture entry point`,
          );
        if (target && relative(target).startsWith("components/")) {
          const directory = path.dirname(target);
          const main = path.join(directory, path.basename(directory) + ".tsx");
          if (
            !testSupport &&
            !(
              target.endsWith(".css") && relative(file) === policy.cssOrderOwner
            ) &&
            target !== main &&
            /\.(?:tsx?|css)$/.test(target) &&
            existsSync(main) &&
            path.dirname(file) !== directory
          )
            errors.push(
              `${relative(file)}: private companion ${relative(target)} is shared; use an allowlisted feature support folder`,
            );
        }
      }
      ts.forEachChild(node, inspect);
    };
    inspect(source);
  }
  function visit(directory: string) {
    const entries = readdirSync(directory, { withFileTypes: true });
    const files = entries.filter((e) => e.isFile());
    const directories = entries.filter((e) => e.isDirectory());
    const rel = relative(directory);
    if (files.length && directories.length)
      errors.push(`${rel}: contains files and subdirectories`);
    if (!files.length) {
      for (const entry of directories) visit(path.join(directory, entry.name));
      return;
    }
    const folder = path.basename(directory);
    const main = `${folder}.tsx`;
    const supported = isSupportDirectory(directory);
    if (!files.some((f) => f.name === main)) {
      if (!supported)
        errors.push(
          `${rel}: unlisted support directory; expected ${main} or the support folder naming convention`,
        );
      if (supported)
        for (const file of files) {
          if (
            !/\.(?:ts|tsx)$/.test(file.name) ||
            /\.(?:test|spec|stories|fixture|test-entry)\.|Fixture/.test(
              file.name,
            )
          )
            continue;
          if (
            componentNames(
              file.name,
              readFileSync(path.join(directory, file.name), "utf8"),
              false,
            ).length
          )
            errors.push(
              `${rel}/${file.name}: support exports a product component`,
            );
        }
      return;
    }
    const mainFile = path.join(directory, main);
    const components = exportedComponents(main, readFileSync(mainFile, "utf8"));
    if (!components.includes(folder))
      errors.push(
        `${rel}/${main}: exported component name must match ${folder}`,
      );
    if (
      components.length !== 1 &&
      ![
        "exports share private leaves",
        "not split: network boundary exception pending owner migration",
      ].includes(policy.multiple[relative(mainFile)] ?? "")
    )
      errors.push(
        `${rel}/${main}: expected one of the exported components, found ${components.join(", ")}`,
      );
    for (const file of files) {
      if (file.name === main || file.name === "README.md") continue;
      if (
        /\.(?:test|spec)\.[cm]?[jt]sx?$/.test(file.name) &&
        !file.name.startsWith(folder)
      )
        errors.push(`${rel}/${file.name}: test name must start with ${folder}`);
      if (!/\.(?:[cm]?[jt]sx?|css)$/.test(file.name))
        errors.push(`${rel}/${file.name}: unsupported component companion`);
      if (
        /\.(?:ts|tsx)$/.test(file.name) &&
        !/\.(?:test|spec|stories|fixture|test-entry)\.|Fixture/.test(
          file.name,
        ) &&
        componentNames(
          file.name,
          readFileSync(path.join(directory, file.name), "utf8"),
          false,
        ).length
      )
        errors.push(
          `${rel}/${file.name}: component declaration must stay in the primary file`,
        );
    }
  }
  const root = path.join(src, "components");
  if (existsSync(root)) visit(root);
  return errors;
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href
) {
  const root = fileURLToPath(
    new URL("../../../client/apps/web/src/", import.meta.url),
  );
  const errors = checkComponentFolders(root);
  if (errors.length) {
    console.error(errors.join("\n"));
    process.exitCode = 1;
  } else console.log("Component folders follow the renderer layout contract.");
}
