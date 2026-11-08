import { useRef, useState } from "react";
import type { GetResult } from "../api";
import { useServerApi, useServerStore } from "../servers/ServerServicesContext";
import { resourceData } from "../store/runtime.ts";

type SkillCatalog = GetResult<"/api/skills">;
export type SkillItem = SkillCatalog["skills"][number];

const MAX_VISIBLE_SKILLS = 8;

interface TokenRange {
  start: number;
  end: number;
  caret: number;
  query: string;
  signature: string;
}

function tokenAtCaret(value: string, caret: number): TokenRange | null {
  const before = value.slice(0, caret);
  const match = /(^|[^\p{L}\p{N}_])\$([\p{L}\p{N}_:-]*)$/u.exec(before);
  if (!match || match[2] === undefined) return null;
  const start = before.length - match[2].length - 1;
  const query = match[2];
  if (/^\d/.test(query)) return null;
  let end = caret;
  while (end < value.length && /[\p{L}\p{N}_:-]/u.test(value.charAt(end)))
    end++;
  if (/^\d/.test(value.slice(start + 1, end))) return null;
  return {
    start,
    end,
    caret,
    query,
    signature: `${start}:${end}:${caret}:${query}`,
  };
}

export function useSkillAutocomplete({
  enabled,
  agentId,
  workspace,
  account,
  cwd,
  provider,
  input,
  insert,
}: {
  enabled: boolean;
  agentId: string;
  workspace: string;
  account: string;
  cwd: string;
  provider: string;
  draft: string;
  input: React.RefObject<HTMLTextAreaElement | null>;
  insert: (text: string, start: number, end: number) => void;
}) {
  const api = useServerApi(),
    store = useServerStore();
  const query = { agent: agentId };
  const read = store.reads.skills(api, query, {
    workspace,
    account,
    cwd,
    provider,
  });
  const catalog = resourceData(read.current);
  const loading = read.current.status === "loading";
  const loadError = read.current.status === "error";
  const [selection, setSelection] = useState<{
    owner: typeof read;
    range: TokenRange;
  } | null>(null);
  const range = selection?.owner === read ? selection.range : null;
  const setRange = (range: TokenRange | null) =>
    setSelection(range ? { owner: read, range } : null);
  const dismissed = useRef("");
  const updateRange = () => {
    if (!enabled) {
      setRange(null);
      return;
    }
    const element = input.current;
    if (!element || element.selectionStart !== element.selectionEnd) {
      dismissed.current = "";
      setRange(null);
      return;
    }
    const next = tokenAtCaret(element.value, element.selectionStart);
    if (!next) {
      dismissed.current = "";
      setRange(null);
      return;
    }
    if (dismissed.current === next.signature) {
      setRange(null);
      return;
    }
    dismissed.current = "";
    setRange(next);
    if (read.current.status !== "live") void read.read().catch(() => {});
  };

  const matches =
    catalog && range
      ? catalog.skills
          .filter((skill) =>
            skill.name
              .toLocaleLowerCase()
              .includes(range.query.toLocaleLowerCase()),
          )
          .slice(0, MAX_VISIBLE_SKILLS)
      : [];
  const choose = (skill: SkillItem) => {
    if (!range) return;
    dismissed.current = range.signature;
    setRange(null);
    const suffix = input.current?.value.slice(range.end, range.end + 1) || "";
    insert(
      `$${skill.name}${/\s/.test(suffix) ? "" : " "}`,
      range.start,
      range.end,
    );
  };

  const dismiss = () => {
    dismissed.current = range?.signature || "";
    setRange(null);
  };

  const blur = () => {
    dismissed.current = "";
    setRange(null);
  };

  return {
    range: range,
    matches: matches,
    loading: loading,
    loadError: loadError,
    hasErrors: !!catalog?.errors.length,
    updateRange,
    dismiss,
    blur,
    choose,
  };
}
