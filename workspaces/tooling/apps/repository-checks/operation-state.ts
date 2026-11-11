// Known limitation: alias collection follows file-wide spelling, not lexical scopes.
// Known limitation: parameter destructuring and receiver aliases can bypass transport checks.
// Known limitation: computed or derived observable state is not fully recognized.
// Known limitation: read loading named pending is rejected; name it loading instead.
// Known limitation: HTTP-to-entity assignment is unenforced and requires review.
// Known limitation: outbox keys built by concatenation or templates are not recognized.
// Known limitation: IIFE-wrapped identity generation may hide the enclosing mutation.
import ts from "typescript";
import { execFileSync } from "node:child_process";
import { isGlueSource, WEB_SOURCE } from "./react-glue.ts";
import { fingerprintMatches } from "./ratchet.ts";

export const OPERATION_STATE_RULE = "web-no-ad-hoc-operation-state";
// Owner: immutable migration seed used only before this rule's baseline exists.
const OPERATION_STATE_SEED = "4f44f7fcb8ce40447fb79489a4451768a47c644d";
// Migration coverage is explicit so removing an import cannot disable the rule.
export const OPERATION_OWNERS = [
  "renameRequest.ts",
  "app/conversation/RenameOwner.ts",
  "servers/projectLocations.ts",
  "components/settings/ProjectLocationsSettings/ProjectLocationsStore.ts",
  "components/settings/ProjectLocationsSettings/ProjectLocationsSettings.tsx",
  "app/shell/AppUi.tsx",
  "servers/requestOperations.ts",
  "servers/serverServices.ts",
  "components/servers/ServerServicesProvider/ServerServicesProvider.tsx",
].map((file) => WEB_SOURCE + file);

export function isOperationOwner(file: string, text: string): boolean {
  return (
    isGlueSource(file) &&
    (OPERATION_OWNERS.includes(file) ||
      (/from\s*["'][^"']*\/(?:requestOperations|RequestRegistry)\.ts["']/.test(
        text,
      ) &&
        file !== WEB_SOURCE + "servers/RequestRegistry.ts"))
  );
}

const stateNames =
  /^(?:pending|busy|saving|sending|uncertain|requestId|request_id|retryBody|networkError)$/;
const hooks = new Set(["useState", "useRef"]);
const transport = new Set(["post", "syncPost"]);
const identities = new Set(["randomUUID", "stableRequestId", "uuid", "v4"]);
const forbidden = new Set(["runInAction", "reload", "refetch"]);
const spelling = (node: ts.Node | undefined): string =>
  node && (ts.isIdentifier(node) || ts.isStringLiteral(node)) ? node.text : "";

const rootedAtZ = (node: ts.Node): boolean =>
  ts.isCallExpression(node) &&
  ts.isPropertyAccessExpression(node.expression) &&
  ["parse", "safeParse"].includes(node.expression.name.text)
    ? false
    : ts.isIdentifier(node)
      ? node.text === "z"
      : ts.isCallExpression(node) ||
          ts.isPropertyAccessExpression(node) ||
          ts.isElementAccessExpression(node) ||
          ts.isParenthesizedExpression(node)
        ? rootedAtZ(node.expression)
        : false;

export function initialOperationBaseline(root: string) {
  return fingerprintMatches(
    OPERATION_OWNERS.flatMap((file) =>
      operationStateMatches(
        file,
        execFileSync("git", ["show", `${OPERATION_STATE_SEED}:${file}`], {
          cwd: root,
          encoding: "utf8",
        }),
      ),
    ),
  );
}

/** Syntax only, like react-glue: track import/member aliases without type resolution. */
export function operationStateMatches(file: string, text: string) {
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);
  const aliases = new Map<string, string>();
  const literalKeys = new Map<string, string>();
  const canonical = (node: ts.Node): string => {
    if (ts.isIdentifier(node)) return aliases.get(node.text) ?? node.text;
    if (ts.isPropertyAccessExpression(node))
      return node.name.text === "run" &&
        /command/i.test(node.expression.getText(source))
        ? "post"
        : node.name.text;
    if (ts.isElementAccessExpression(node))
      return spelling(node.argumentExpression);
    return "";
  };
  const collect = (node: ts.Node) => {
    if (ts.isImportSpecifier(node) && !node.isTypeOnly) {
      let name = spelling(node.propertyName ?? node.name);
      const declaration = node.parent.parent.parent;
      if (
        ts.isImportDeclaration(declaration) &&
        ts.isStringLiteral(declaration.moduleSpecifier) &&
        declaration.moduleSpecifier.text.endsWith("/request-id.ts") &&
        name === "requestId"
      )
        name = "randomUUID";
      aliases.set(node.name.text, name);
    }
    if (ts.isVariableDeclaration(node) && node.initializer) {
      if (
        ts.isIdentifier(node.name) &&
        ts.isStringLiteral(node.initializer) &&
        ts.isVariableDeclarationList(node.parent) &&
        node.parent.flags & ts.NodeFlags.Const
      )
        literalKeys.set(node.name.text, node.initializer.text);
      if (ts.isIdentifier(node.name))
        aliases.set(node.name.text, canonical(node.initializer));
      if (ts.isObjectBindingPattern(node.name))
        for (const item of node.name.elements) {
          const name = spelling(item.propertyName ?? item.name);
          if (ts.isIdentifier(item.name))
            aliases.set(
              item.name.text,
              name === "run" &&
                /command/i.test(node.initializer.getText(source))
                ? "post"
                : name,
            );
        }
    }
    ts.forEachChild(node, collect);
  };
  collect(source);
  const matches: { file: string; text: string; message: string }[] = [];
  const report = (node: ts.Node, detail: string) =>
    matches.push({
      file,
      text: node.getText(source),
      message: `${detail}. Keep mutation identity/status in RequestRegistry; retry the saved request and read entities from SSE.`,
    });
  const bindingNames = (node: ts.BindingName): string[] =>
    ts.isIdentifier(node)
      ? [node.text]
      : node.elements.flatMap((item) =>
          ts.isBindingElement(item) ? bindingNames(item.name) : [],
        );
  const walk = (node: ts.Node) => {
    if (
      ts.isPropertyDeclaration(node) &&
      stateNames.test(spelling(node.name)) &&
      !node.modifiers?.some(
        (modifier) => modifier.kind === ts.SyntaxKind.ReadonlyKeyword,
      )
    )
      report(node, "Writable operation field");
    if (
      ts.isPropertyAssignment(node) &&
      stateNames.test(spelling(node.name)) &&
      (!/^(?:requestId|request_id)$/.test(spelling(node.name)) ||
        ts.isStringLiteral(node.initializer) ||
        ts.isNewExpression(node.initializer) ||
        node.initializer.kind === ts.SyntaxKind.NullKeyword) &&
      !ts.isPropertyAccessExpression(node.initializer) &&
      !rootedAtZ(node.initializer)
    )
      report(node, "Stored operation state");
    if (
      ts.isVariableDeclaration(node) &&
      node.initializer &&
      ts.isCallExpression(node.initializer) &&
      hooks.has(canonical(node.initializer.expression)) &&
      bindingNames(node.name).some((name) => stateNames.test(name))
    )
      report(node, "Operation hook state");
    if (
      ts.isBinaryExpression(node) &&
      node.operatorToken.kind === ts.SyntaxKind.EqualsToken &&
      (ts.isPropertyAccessExpression(node.left) ||
        ts.isElementAccessExpression(node.left)) &&
      stateNames.test(canonical(node.left))
    )
      report(node, "Operation state assignment");
    if (ts.isCallExpression(node)) {
      const name = canonical(node.expression);
      if (transport.has(name)) report(node, "Direct mutation transport");
      if (forbidden.has(name))
        report(node, "Mutation response overlay or refetch");
      if (name === "set" && stateNames.test(spelling(node.arguments[0])))
        report(node, "Operation map state");
      if (
        ["set", "setItem"].includes(name) &&
        /outbox|storage/i.test(node.expression.getText(source)) &&
        /^(?:studio-server-write:|studio-pending-|studio-sidebar-rename:|studio-project-request:|stableRequest)/.test(
          node.arguments[0] && ts.isStringLiteral(node.arguments[0])
            ? node.arguments[0].text
            : node.arguments[0] && ts.isIdentifier(node.arguments[0])
              ? (literalKeys.get(node.arguments[0].text) ?? "")
              : "",
        )
      )
        report(node, "Direct request-outbox write");
      if (identities.has(name)) {
        // Navigation/device/stream correlation IDs do not own mutations.
        let owner: ts.Node = node;
        while (
          owner.parent &&
          !ts.isFunctionLike(owner) &&
          !ts.isSourceFile(owner)
        )
          owner = owner.parent;
        let mutation = false;
        const findMutation = (child: ts.Node) => {
          if (
            ts.isCallExpression(child) &&
            (transport.has(canonical(child.expression)) ||
              [
                "start",
                "intent",
                "saveProjectLocation",
                "projectLocationRequest",
              ].includes(canonical(child.expression)))
          )
            mutation = true;
          ts.forEachChild(child, findMutation);
        };
        findMutation(owner);
        const parent = node.parent;
        const identity =
          (ts.isVariableDeclaration(parent) ||
            ts.isPropertyAssignment(parent) ||
            ts.isPropertyDeclaration(parent)) &&
          stateNames.test(spelling(parent.name));
        if (mutation || (identity && !ts.isFunctionLike(owner)))
          report(node, "Mutation identity generation");
      }
    }
    ts.forEachChild(node, walk);
  };
  walk(source);
  return matches;
}
