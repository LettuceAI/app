const open = "<img>";
const closers = ["</img>", "[continue]", "[/continue]"];

/** Where a streamed reply stands relative to a scene image directive (`<img>prompt</img>`). */
export interface SceneDirectiveState {
  carry: string;
  inside: boolean;
  seen: boolean;
}

export function sceneDirectiveState(): SceneDirectiveState {
  return { carry: "", inside: false, seen: false };
}

function partialTail(value: string, tokens: readonly string[]): number {
  const lower = value.toLowerCase();
  let best = 0;
  for (const token of tokens) {
    for (let length = Math.min(value.length, token.length - 1); length > best; length--) {
      if (lower.endsWith(token.slice(0, length))) {
        best = length;
        break;
      }
    }
  }
  return best;
}

function earliest(value: string, tokens: readonly string[]): { index: number; length: number } | null {
  const lower = value.toLowerCase();
  let found: { index: number; length: number } | null = null;
  for (const token of tokens) {
    const index = lower.indexOf(token);
    if (index !== -1 && (found === null || index < found.index)) found = { index, length: token.length };
  }
  return found;
}

/** The part of a delta the reader sees: the directive and its prompt are held back, a split tag is carried to the next delta. */
export function visibleDelta(state: SceneDirectiveState, text: string): string {
  let remaining = state.carry + text;
  let visible = "";
  state.carry = "";
  while (remaining.length > 0) {
    if (!state.inside) {
      const match = earliest(remaining, [open]);
      if (match) {
        visible += remaining.slice(0, match.index);
        remaining = remaining.slice(match.index + match.length);
        state.inside = true;
        state.seen = true;
        continue;
      }
      const keep = remaining.length - partialTail(remaining, [open]);
      visible += remaining.slice(0, keep);
      state.carry = remaining.slice(keep);
      break;
    }
    const match = earliest(remaining, closers);
    if (match) {
      remaining = remaining.slice(match.index + match.length);
      state.inside = false;
      continue;
    }
    state.carry = remaining.slice(remaining.length - partialTail(remaining, closers));
    break;
  }
  return visible;
}
