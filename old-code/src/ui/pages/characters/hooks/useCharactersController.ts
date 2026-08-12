import { useCallback, useEffect, useReducer } from "react";

import { listCharacters, deleteCharacter } from "../../../../core/storage/repo";
import type { Character } from "../../../../core/storage/schemas";

type CharactersState = {
  characters: Character[];
  loading: boolean;
  selectedCharacter: Character | null;
  showDeleteConfirm: boolean;
  deleting: boolean;
};

type Action =
  | { type: "set_loading"; payload: boolean }
  | { type: "set_characters"; payload: Character[] }
  | { type: "set_selected_character"; payload: Character | null }
  | { type: "set_show_delete_confirm"; payload: boolean }
  | { type: "set_deleting"; payload: boolean };

const initialState: CharactersState = {
  characters: [],
  loading: true,
  selectedCharacter: null,
  showDeleteConfirm: false,
  deleting: false,
};

function reducer(state: CharactersState, action: Action): CharactersState {
  switch (action.type) {
    case "set_loading":
      return { ...state, loading: action.payload };
    case "set_characters":
      return { ...state, characters: action.payload, loading: false };
    case "set_selected_character":
      return { ...state, selectedCharacter: action.payload };
    case "set_show_delete_confirm":
      return { ...state, showDeleteConfirm: action.payload };
    case "set_deleting":
      return { ...state, deleting: action.payload };
    default:
      return state;
  }
}

export function useCharactersController() {
  const [state, dispatch] = useReducer(reducer, initialState);

  const loadCharacters = useCallback(async () => {
    dispatch({ type: "set_loading", payload: true });
    try {
      const list = await listCharacters();
      dispatch({ type: "set_characters", payload: list });
    } catch (error) {
      console.error("Failed to load characters:", error);
      dispatch({ type: "set_loading", payload: false });
    }
  }, []);

  useEffect(() => {
    void loadCharacters();
  }, [loadCharacters]);

  const setSelectedCharacter = useCallback((character: Character | null) => {
    dispatch({ type: "set_selected_character", payload: character });
  }, []);

  const setShowDeleteConfirm = useCallback((value: boolean) => {
    dispatch({ type: "set_show_delete_confirm", payload: value });
  }, []);

  const handleDelete = useCallback(
    async () => {
      if (!state.selectedCharacter) return;
      dispatch({ type: "set_deleting", payload: true });
      try {
        await deleteCharacter(state.selectedCharacter.id);
        await loadCharacters();
        dispatch({ type: "set_show_delete_confirm", payload: false });
        dispatch({ type: "set_selected_character", payload: null });
      } catch (error) {
        console.error("Failed to delete character:", error);
      } finally {
        dispatch({ type: "set_deleting", payload: false });
      }
    },
    [state.selectedCharacter, loadCharacters],
  );

  return {
    state,
    setSelectedCharacter,
    setShowDeleteConfirm,
    handleDelete,
    reload: loadCharacters,
  };
}
