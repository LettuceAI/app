/** Split a comma-separated string into trimmed, non-empty tokens. */
export function parseCommaSeparated(raw: string): string[] {
  return raw
    .split(",")
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}
