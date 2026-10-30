function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

interface MentionMatch {
  index: number;
  length: number;
}

export function findMention(draft: string, name: string): MentionMatch | null {
  const escaped = escapeRegExp(name);
  const quoted = new RegExp(`@"${escaped}"`, "i").exec(draft);
  if (quoted) return { index: quoted.index, length: quoted[0].length };
  const unquoted = new RegExp(`(^|\\s)@${escaped}(?=$|\\s|[.,!?;:])`, "i").exec(draft);
  if (!unquoted) return null;
  return { index: unquoted.index + (unquoted[1]?.length ?? 0), length: name.length + 1 };
}

function mentionToken(name: string): string {
  return name.includes(" ") ? `@"${name}"` : `@${name}`;
}

export function toggleMention(draft: string, name: string): string {
  const match = findMention(draft, name);
  if (match) {
    const before = draft.slice(0, match.index);
    const after = draft.slice(match.index + match.length);
    return `${before}${after}`.replace(/\s{2,}/g, " ").replace(/^\s+/, "");
  }
  const trimmed = draft.replace(/\s+$/, "");
  return trimmed.length ? `${trimmed} ${mentionToken(name)} ` : `${mentionToken(name)} `;
}

export interface ActiveMention {
  start: number;
  query: string;
}

export function activeMention(value: string, cursor: number): ActiveMention | null {
  const before = value.slice(0, cursor);
  const at = before.lastIndexOf("@");
  if (at === -1) return null;
  const previous = at > 0 ? before[at - 1] : " ";
  if (previous !== " " && previous !== "\n") return null;
  const after = before.slice(at + 1);
  if (after.includes("\n")) return null;
  if (after.startsWith('"')) return after.indexOf('"', 1) === -1 ? { start: at, query: after.slice(1) } : null;
  return after.includes(" ") ? null : { start: at, query: after };
}

export function insertMention(value: string, mention: ActiveMention, cursor: number, name: string): { text: string; cursor: number } {
  const token = mentionToken(name);
  const head = `${value.slice(0, mention.start)}${token} `;
  return { text: `${head}${value.slice(cursor)}`, cursor: head.length };
}
