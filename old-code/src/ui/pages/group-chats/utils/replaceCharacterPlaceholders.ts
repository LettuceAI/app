import type { Character } from "../../../../core/storage/schemas";

/**
 * Replace character name placeholders in scene content with actual character names.
 * Supports {{@"Character Name"}} syntax.
 *
 * If a character name in the placeholder doesn't exist in the provided characters array,
 * the placeholder is left as-is (for display purposes).
 *
 * @param content - The content containing placeholders
 * @param characters - Array of characters in the group chat
 * @returns Content with placeholders replaced
 */
export function replaceCharacterPlaceholders(
  content: string,
  characters: Character[]
): string {
  if (!content) return content;

  let result = content;
  const regex = /\{\{@"([^"]+)"\}\}/g;

  result = result.replace(regex, (match, characterName) => {
    // Check if this character exists in the group
    const characterExists = characters.some(c => c.name === characterName);

    if (characterExists) {
      // Replace with just the character name
      return characterName;
    } else {
      // Keep the original placeholder if character not found
      return match;
    }
  });

  return result;
}
