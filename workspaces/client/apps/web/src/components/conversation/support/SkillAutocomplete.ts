import { makeAutoObservable, observableRef } from "mobx";
import type { GetResult } from "../../../api.ts";
import type { useServerNet } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { resourceData } from "../../../store/runtime.ts";

export type SkillItem = GetResult<"/api/skills">["skills"][number];

const MAX_VISIBLE_SKILLS = 8;

type TokenRange = NonNullable<ReturnType<typeof tokenAtCaret>>;

function tokenAtCaret(value: string, caret: number) {
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
    query,
    signature: `${start}:${end}:${caret}:${query}`,
  };
}

type SkillRead = ReturnType<ReturnType<typeof useServerNet>["reads"]["skills"]>;

/** Selection and dismissal belong to the selected conversation's composer. */
export class SkillAutocomplete {
  selection: { owner: SkillRead; range: TokenRange } | null = null;
  dismissed = "";
  dismissedCommand: string | null = null;
  constructor() {
    makeAutoObservable(this, { selection: observableRef }, { autoBind: true });
  }
  rangeFor(read: SkillRead) {
    return this.selection?.owner === read ? this.selection.range : null;
  }
  get catalog() {
    return this.selection && resourceData(this.selection.owner.current);
  }
  get matches() {
    const query = this.selection?.range.query.toLocaleLowerCase();
    return query === undefined
      ? []
      : (this.catalog?.skills ?? [])
          .filter((skill) => skill.name.toLocaleLowerCase().includes(query))
          .slice(0, MAX_VISIBLE_SKILLS);
  }
  updateRange(
    enabled: boolean,
    read: SkillRead,
    element: HTMLTextAreaElement | null,
  ) {
    const next =
      enabled && element && element.selectionStart === element.selectionEnd
        ? tokenAtCaret(element.value, element.selectionStart)
        : null;
    if (!next) this.dismissed = "";
    this.selection =
      next && this.dismissed !== next.signature
        ? { owner: read, range: next }
        : null;
    if (this.selection) {
      this.dismissed = "";
      if (read.current.status !== "live") void read.read().catch(() => {});
    }
  }
  choose(skill: SkillItem, value: string) {
    const range = this.selection?.range;
    if (!range) return;
    this.dismiss();
    const suffix = value.slice(range.end, range.end + 1);
    return { text: `$${skill.name}${/\s/.test(suffix) ? "" : " "}`, ...range };
  }
  dismiss() {
    this.dismissed = this.selection?.range.signature || "";
    this.selection = null;
  }
  blur() {
    this.dismissed = "";
    this.selection = null;
  }
  dismissCommand(key: string) {
    this.dismissedCommand = key;
  }
}
