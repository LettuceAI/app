export function formatSequences(values: string[]): string {
  return values.map((value) => value.replace(/\\/g, "\\\\").replace(/,/g, "\\,").replace(/\n/g, "\\n").replace(/\r/g, "\\r").replace(/\t/g, "\\t").replace(/^ | $/g, "\\s")).join(", ");
}

export function parseSequences(value: string): string[] {
  const result: string[] = [];
  let current = "";
  let escaped = false;
  for (const char of value.trim()) {
    if (escaped) {
      current += char === "n" ? "\n" : char === "r" ? "\r" : char === "t" ? "\t" : char === "s" ? " " : char;
      escaped = false;
    } else if (char === "\\") escaped = true;
    else if (char === ",") { result.push(current); current = ""; }
    else if (char !== " " || current !== "") current += char;
  }
  if (escaped) throw new Error("Incomplete escape");
  result.push(current);
  return result;
}
