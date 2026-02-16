export function replacePlaceholders(
  text: string,
  charName: string,
  personaName: string
): string {
  if (!text) return text;
  return text
    .split("{{char}}")
    .join(charName ?? "")
    .split("{{persona}}")
    .join(personaName ?? "")
    .split("{{user}}")
    .join(personaName ?? "");
}

