export function replacePlaceholders(text: string, charName: string, personaName: string): string {
  if (!text) return text;
  const safeCharName = charName ?? "";
  const safePersonaName = personaName ?? "";

  return text
    .replace(/\{\{\s*char(?:\.name)?\s*\}\}/g, safeCharName)
    .replace(/\{\{\s*persona(?:\.name)?\s*\}\}/g, safePersonaName)
    .replace(/\{\{\s*user(?:\.name)?\s*\}\}/g, safePersonaName);
}
