export interface AvatarLibrarySelectionPayload {
  filePath: string;
}

export interface BackgroundLibrarySelectionPayload {
  filePath: string;
}

const AVATAR_LIBRARY_SELECTION_PREFIX = "avatar-library-selection:";
const BACKGROUND_LIBRARY_SELECTION_PREFIX = "background-library-selection:";
const CHARACTER_CREATE_LIBRARY_RETURN_PREFIX = "character-create-library-return:";

export function buildAvatarLibrarySelectionKey(returnPath: string): string {
  return `${AVATAR_LIBRARY_SELECTION_PREFIX}${returnPath}`;
}

export function buildBackgroundLibrarySelectionKey(returnPath: string): string {
  return `${BACKGROUND_LIBRARY_SELECTION_PREFIX}${returnPath}`;
}

export function buildCharacterCreateLibraryReturnKey(returnPath: string): string {
  return `${CHARACTER_CREATE_LIBRARY_RETURN_PREFIX}${returnPath}`;
}
