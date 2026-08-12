const SCENE_TAG_OPEN = "<img>";
const SCENE_CLOSE_TOKENS = ["</img>", "[continue]", "[/continue]"];

export type SceneDirectiveStreamState = {
  carry: string;
  insideTag: boolean;
  promptBuffer: string;
  extractedPrompt: string | null;
};

export function createSceneDirectiveStreamState(): SceneDirectiveStreamState {
  return {
    carry: "",
    insideTag: false,
    promptBuffer: "",
    extractedPrompt: null,
  };
}

function longestSuffixPrefixLength(value: string, token: string): number {
  const valueLower = value.toLowerCase();
  const tokenLower = token.toLowerCase();
  const maxLength = Math.min(value.length, token.length - 1);
  for (let length = maxLength; length > 0; length--) {
    if (valueLower.endsWith(tokenLower.slice(0, length))) {
      return length;
    }
  }
  return 0;
}

function findEarliestTokenIndex(value: string, tokens: string[]): { index: number; token: string } | null {
  const valueLower = value.toLowerCase();
  let bestIndex = -1;
  let bestToken = "";

  for (const token of tokens) {
    const index = valueLower.indexOf(token.toLowerCase());
    if (index === -1) continue;
    if (bestIndex === -1 || index < bestIndex) {
      bestIndex = index;
      bestToken = token;
    }
  }

  return bestIndex >= 0 ? { index: bestIndex, token: bestToken } : null;
}

function longestAnyTokenPrefixSuffixLength(value: string, tokens: string[]): number {
  return tokens.reduce(
    (best, token) => Math.max(best, longestSuffixPrefixLength(value, token)),
    0,
  );
}

export function consumeSceneDirectiveDelta(
  state: SceneDirectiveStreamState,
  text: string,
): { content: string; prompt: string | null } {
  let remaining = state.carry + text;
  let visibleContent = "";
  state.carry = "";

  while (remaining.length > 0) {
    if (!state.insideTag) {
      const openMatch = findEarliestTokenIndex(remaining, [SCENE_TAG_OPEN]);
      if (openMatch) {
        visibleContent += remaining.slice(0, openMatch.index);
        remaining = remaining.slice(openMatch.index + openMatch.token.length);
        state.insideTag = true;
        state.promptBuffer = "";
        continue;
      }

      const partialLength = longestAnyTokenPrefixSuffixLength(remaining, [SCENE_TAG_OPEN]);
      const visibleLength = remaining.length - partialLength;
      if (visibleLength > 0) {
        visibleContent += remaining.slice(0, visibleLength);
      }
      state.carry = remaining.slice(visibleLength);
      break;
    }

    const closeMatch = findEarliestTokenIndex(remaining, SCENE_CLOSE_TOKENS);
    if (closeMatch) {
      state.promptBuffer += remaining.slice(0, closeMatch.index);
      const prompt = state.promptBuffer.trim();
      if (!state.extractedPrompt && prompt) {
        state.extractedPrompt = prompt;
      }
      state.promptBuffer = "";
      remaining = remaining.slice(closeMatch.index + closeMatch.token.length);
      state.insideTag = false;
      continue;
    }

    const partialLength = longestAnyTokenPrefixSuffixLength(remaining, SCENE_CLOSE_TOKENS);
    const promptLength = remaining.length - partialLength;
    if (promptLength > 0) {
      state.promptBuffer += remaining.slice(0, promptLength);
    }
    state.carry = remaining.slice(promptLength);
    break;
  }

  return {
    content: visibleContent,
    prompt: state.extractedPrompt,
  };
}

export function finalizeSceneDirectiveStream(
  state: SceneDirectiveStreamState,
): { content: string; prompt: string | null } {
  if (state.insideTag) {
    state.carry = "";
    state.promptBuffer = "";
    return { content: "", prompt: state.extractedPrompt };
  }

  const tail = state.carry;
  state.carry = "";
  return {
    content: tail,
    prompt: state.extractedPrompt,
  };
}

export function sanitizeAssistantSceneDirective(content: string): {
  cleanContent: string;
  scenePrompt: string | null;
} {
  const streamState = createSceneDirectiveStreamState();
  const firstPass = consumeSceneDirectiveDelta(streamState, content);
  const tail = finalizeSceneDirectiveStream(streamState);

  return {
    cleanContent: `${firstPass.content}${tail.content}`.trim(),
    scenePrompt: tail.prompt?.trim() || firstPass.prompt?.trim() || null,
  };
}
