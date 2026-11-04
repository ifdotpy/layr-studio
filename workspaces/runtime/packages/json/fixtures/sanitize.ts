import { createHash } from "node:crypto";

// Only these schema keys survive. Other keys can be user-controlled data.
const keys = new Set(
  `id rootId epoch status parentId created updated agent name model source type kind
value collection seq deleted payload activityAt activityGeneration bytes command cwd requestId turnId
interactive livenessCommand log stallTimeoutSeconds outputMatchExcerpts excerpt logOffset outputPatterns
outputMatchesTotal outputLineTail stdout stderr exitCode finished error itemId durationMs startedAtMs
completedAtMs outputTruncated tail arguments processId tool server outputFile ownerEpoch
events lastEvent activity activeTools contextUsage tokensUsed tokenUsageAccounting
items role content text timestamp message data method params result jsonrpc cursor records messages
page nextCursor hasMore root agentId revision rows`.split(/\s+/),
);

function maskString(raw: string, isKey: boolean, attempt = 0): string {
  let masked = '"';
  const digest = createHash("sha256")
    .update(raw)
    .update(String(attempt))
    .digest();
  let offset = 0;
  for (let i = 1; i < raw.length - 1; i++) {
    const char = raw[i];
    if (char === "\\") {
      const next = raw[++i];
      if (next === "u") {
        const code = Number.parseInt(raw.slice(i + 1, i + 5), 16);
        masked +=
          code >= 0xd800 && code <= 0xdbff
            ? "\\ud83d"
            : code >= 0xdc00 && code <= 0xdfff
              ? "\\ude00"
              : code > 0x7ff
                ? "\\u6f22"
                : code > 0x7f
                  ? "\\u00e9"
                  : "\\u0061";
        i += 4;
      } else {
        masked += `\\${next}`;
      }
    } else if (char !== undefined) {
      const code = raw.codePointAt(i);
      if (code === undefined) throw new Error("invalid string");
      if (code > 0xffff) {
        masked += "😀";
        i++;
      } else if (code > 0x7ff) masked += "漢";
      else if (code > 0x7f) masked += "é";
      else
        masked += /\s/.test(char)
          ? char
          : isKey
            ? String.fromCharCode(
                97 + ((digest[offset++ % digest.length] ?? 0) % 26),
              )
            : "x";
    }
  }
  return `${masked}"`;
}

// Scan JSON tokens rather than parsing and re-encoding: escapes, string sizes,
// Unicode widths, number syntax, arrays, and object nesting are retained.
export function sanitize(input: string): string {
  let output = "";
  const objects: { mapped: Map<string, string>; used: Set<string> }[] = [];
  for (let i = 0; i < input.length;) {
    if (input[i] === '"') {
      const start = i++;
      while (i < input.length) {
        if (input[i] === "\\") {
          i += 2;
          continue;
        }
        if (input[i++] === '"') break;
      }
      const raw = input.slice(start, i);
      let next = i;
      while (/\s/.test(input[next] ?? "") && next < input.length) next++;
      const isKey = input[next] === ":";
      const key: unknown = isKey ? JSON.parse(raw) : null;
      if (typeof key === "string") {
        const object = objects.at(-1);
        if (!object) throw new Error("key outside object");
        let replacement = object.mapped.get(key);
        if (replacement === undefined) {
          if (keys.has(key)) replacement = raw;
          else {
            for (let attempt = 0; attempt < 1024; attempt++) {
              const candidate = maskString(raw, true, attempt);
              const decoded: unknown = JSON.parse(candidate);
              if (typeof decoded === "string" && !object.used.has(decoded)) {
                replacement = candidate;
                break;
              }
            }
          }
          if (replacement === undefined)
            throw new Error("cannot mask object key without a collision");
          const decoded: unknown = JSON.parse(replacement);
          if (typeof decoded !== "string")
            throw new Error("invalid masked key");
          object.used.add(decoded);
          object.mapped.set(key, replacement);
        }
        output += replacement;
      } else output += maskString(raw, false);
    } else if (/[0-9]/.test(input[i] ?? "")) {
      output += input[i] === "0" ? "0" : "1";
      i++;
    } else {
      if (input[i] === "{")
        objects.push({ mapped: new Map(), used: new Set(keys) });
      else if (input[i] === "}") objects.pop();
      output += input[i++];
    }
  }
  JSON.parse(output);
  if (Buffer.byteLength(output) !== Buffer.byteLength(input))
    throw new Error("sanitization changed byte length");
  return output;
}
