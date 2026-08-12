export type ThinkStreamState = {
  inThink: boolean;
  pending: string;
};

const THINK_OPEN = "<think>";
const THINK_CLOSE = "</think>";

function longestSuffixPrefix(text: string, tag: string): number {
  const max = Math.min(text.length, tag.length - 1);
  for (let len = max; len > 0; len -= 1) {
    if (text.endsWith(tag.slice(0, len))) {
      return len;
    }
  }
  return 0;
}

export function createThinkStreamState(): ThinkStreamState {
  return { inThink: false, pending: "" };
}

export function consumeThinkDelta(
  state: ThinkStreamState,
  chunk: string,
): { content: string; reasoning: string } {
  let text = state.pending + chunk;
  state.pending = "";

  let content = "";
  let reasoning = "";

  while (text.length > 0) {
    if (state.inThink) {
      const endIndex = text.indexOf(THINK_CLOSE);
      if (endIndex >= 0) {
        reasoning += text.slice(0, endIndex);
        text = text.slice(endIndex + THINK_CLOSE.length);
        state.inThink = false;
        continue;
      }

      const keep = longestSuffixPrefix(text, THINK_CLOSE);
      if (keep > 0) {
        reasoning += text.slice(0, text.length - keep);
        state.pending = text.slice(text.length - keep);
      } else {
        reasoning += text;
      }
      return { content, reasoning };
    }

    const startIndex = text.indexOf(THINK_OPEN);
    if (startIndex >= 0) {
      content += text.slice(0, startIndex);
      text = text.slice(startIndex + THINK_OPEN.length);
      state.inThink = true;
      continue;
    }

    const keep = longestSuffixPrefix(text, THINK_OPEN);
    if (keep > 0) {
      content += text.slice(0, text.length - keep);
      state.pending = text.slice(text.length - keep);
    } else {
      content += text;
    }
    return { content, reasoning };
  }

  return { content, reasoning };
}

export function finalizeThinkStream(state: ThinkStreamState): { content: string; reasoning: string } {
  if (!state.pending) {
    return { content: "", reasoning: "" };
  }
  const leftover = state.pending;
  state.pending = "";
  if (state.inThink) {
    return { content: "", reasoning: leftover };
  }
  return { content: leftover, reasoning: "" };
}

export function splitThinkTags(text: string): { content: string; reasoning: string } {
  const state = createThinkStreamState();
  const first = consumeThinkDelta(state, text);
  const tail = finalizeThinkStream(state);
  return {
    content: first.content + tail.content,
    reasoning: first.reasoning + tail.reasoning,
  };
}

export function normalizeThinkTags<T extends { content: string; reasoning?: string | null }>(
  message: T,
): T {
  const parsed = splitThinkTags(message.content);
  if (parsed.content === message.content && parsed.reasoning.length === 0) {
    return message;
  }
  const mergedReasoning = [message.reasoning ?? "", parsed.reasoning].filter(Boolean).join("\n");
  return {
    ...message,
    content: parsed.content,
    reasoning: mergedReasoning.length > 0 ? mergedReasoning : message.reasoning ?? null,
  };
}
