export interface JinjaToken { text: string; className: string }

const keywords = new Set(["if", "else", "elif", "endif", "for", "endfor", "in", "is", "not", "and", "or", "set", "macro", "endmacro", "call", "endcall", "filter", "endfilter", "block", "endblock", "extends", "include", "import", "from", "as", "with", "endwith", "raw", "endraw", "do", "true", "false", "none", "True", "False", "None", "recursive"]);

export function jinjaTokens(source: string): JinjaToken[] {
  const tokens: JinjaToken[] = [];
  const push = (text: string, className = "text-fg-2") => { if (text) tokens.push({ text, className }); };
  const regions = /({#[\s\S]*?(?:#}|$)|{{-?[\s\S]*?(?:-?}}|$)|{%-?[\s\S]*?(?:-?%}|$)|<\|[^\n]*?\|>)/g;
  let position = 0;
  for (const region of source.matchAll(regions)) {
    push(source.slice(position, region.index));
    const text = region[0];
    position = region.index + text.length;
    if (text.startsWith("{#")) { push(text, "text-fg-3 italic"); continue; }
    if (text.startsWith("<|")) { push(text, "text-warning"); continue; }
    const parts = /("(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|\b\d+(?:\.\d+)?\b|[a-zA-Z_]\w*|{{-?|-?}}|{%-?|-?%}|[|=+*/%<>!~:.,()[\]{}-])/g;
    let offset = 0;
    for (const part of text.matchAll(parts)) {
      push(text.slice(offset, part.index));
      const word = part[0];
      push(word, word.startsWith('"') || word.startsWith("'") ? "text-accent" : keywords.has(word) ? "text-secondary" : /^\d/.test(word) ? "text-warning" : /^[a-zA-Z_]\w*$/.test(word) ? "text-fg" : "text-info");
      offset = part.index + word.length;
    }
    push(text.slice(offset));
  }
  push(source.slice(position));
  return tokens;
}
