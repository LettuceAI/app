/** Splits every comma-separated entry into its own tag, so a pasted "a, b, c" saves as three. */
export function splitTags(tags: readonly string[]): string[] {
  const out: string[] = [];
  for (const part of tags.flatMap((tag) => tag.split(","))) {
    const tag = part.trim();
    if (tag && !out.includes(tag)) out.push(tag);
  }
  return out;
}
