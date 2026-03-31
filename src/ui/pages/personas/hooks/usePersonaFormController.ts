import { useCallback, useEffect, useReducer, useRef } from "react";
import { useNavigate } from "react-router-dom";

import { getPersona, savePersona } from "../../../../core/storage/repo";
import { loadAvatar, saveAvatar } from "../../../../core/storage/avatars";
import { convertToImageRef, deleteImageRef } from "../../../../core/storage/images";
import { invalidateAvatarCache } from "../../../hooks/useAvatar";
import type { AvatarCrop } from "../../../../core/storage/schemas";

type PersonaFormState = {
  loading: boolean;
  saving: boolean;
  error: string | null;
  title: string;
  description: string;
  nickname: string;
  isDefault: boolean;
  avatarPath: string | null;
  avatarCrop: AvatarCrop | null;
  avatarRoundPath: string | null;
  designDescription: string;
  designReferenceImageIds: string[];
};

type Action =
  | { type: "set_loading"; payload: boolean }
  | { type: "set_saving"; payload: boolean }
  | { type: "set_error"; payload: string | null }
  | {
      type: "set_fields";
      payload: Partial<Omit<PersonaFormState, "loading" | "saving" | "error">>;
    };

const initialState: PersonaFormState = {
  loading: true,
  saving: false,
  error: null,
  title: "",
  description: "",
  nickname: "",
  isDefault: false,
  avatarPath: null,
  avatarCrop: null,
  avatarRoundPath: null,
  designDescription: "",
  designReferenceImageIds: [],
};

function reducer(state: PersonaFormState, action: Action): PersonaFormState {
  switch (action.type) {
    case "set_loading":
      return { ...state, loading: action.payload };
    case "set_saving":
      return { ...state, saving: action.payload };
    case "set_error":
      return { ...state, error: action.payload };
    case "set_fields":
      return { ...state, ...action.payload };
    default:
      return state;
  }
}

export function usePersonaFormController(personaId: string | undefined) {
  const navigate = useNavigate();
  const [state, dispatch] = useReducer(reducer, initialState);

  // Track initial state for change detection
  const initialStateRef = useRef<{
    title: string;
    description: string;
    nickname: string;
    isDefault: boolean;
    avatarPath: string | null;
    avatarCrop: string;
    avatarRoundPath: string;
    designDescription: string;
    designReferenceImageIds: string;
  } | null>(null);
  const persistedAvatarRef = useRef<{ filename?: string; url?: string }>({});

  const loadPersona = useCallback(async () => {
    if (!personaId) {
      navigate("/library?view=personas", { replace: true });
      return;
    }

    dispatch({ type: "set_loading", payload: true });
    try {
      const persona = await getPersona(personaId);
      if (!persona) {
        navigate("/library?view=personas", { replace: true });
        return;
      }

      // Load avatar if it exists
      let avatarDataUrl: string | null = null;
      let avatarRoundDataUrl: string | null = null;
      if (persona.avatarPath) {
        const loadedAvatar = await loadAvatar("persona", personaId, persona.avatarPath);
        const loadedRound = await loadAvatar("persona", personaId, "avatar_round.webp").catch(
          () => undefined,
        );
        avatarDataUrl = loadedAvatar ?? null;
        avatarRoundDataUrl = loadedRound ?? null;
      }

      persistedAvatarRef.current = {
        filename: persona.avatarPath ?? undefined,
        url: avatarDataUrl ?? undefined,
      };

      dispatch({
        type: "set_fields",
        payload: {
          title: persona.title,
          description: persona.description,
          nickname: persona.nickname ?? "",
          isDefault: persona.isDefault ?? false,
          avatarPath: avatarDataUrl,
          avatarCrop: persona.avatarCrop ?? null,
          avatarRoundPath: avatarRoundDataUrl,
          designDescription: persona.designDescription ?? "",
          designReferenceImageIds: Array.isArray(persona.designReferenceImageIds)
            ? persona.designReferenceImageIds
            : [],
        },
      });

      // Store initial state for change detection
      initialStateRef.current = {
        title: persona.title,
        description: persona.description,
        nickname: persona.nickname ?? "",
        isDefault: persona.isDefault ?? false,
        avatarPath: avatarDataUrl,
        avatarCrop: JSON.stringify(persona.avatarCrop ?? null),
        avatarRoundPath: JSON.stringify(avatarRoundDataUrl ?? null),
        designDescription: persona.designDescription ?? "",
        designReferenceImageIds: JSON.stringify(persona.designReferenceImageIds ?? []),
      };
      dispatch({ type: "set_error", payload: null });
    } catch (error) {
      console.error("Failed to load persona:", error);
      dispatch({
        type: "set_error",
        payload: "Failed to load persona",
      });
    } finally {
      dispatch({ type: "set_loading", payload: false });
    }
  }, [personaId, navigate]);

  useEffect(() => {
    void loadPersona();
  }, [loadPersona]);

  const setTitle = useCallback((value: string) => {
    dispatch({ type: "set_fields", payload: { title: value } });
  }, []);

  const setDescription = useCallback((value: string) => {
    dispatch({ type: "set_fields", payload: { description: value } });
  }, []);

  const setNickname = useCallback((value: string) => {
    dispatch({ type: "set_fields", payload: { nickname: value } });
  }, []);

  const setIsDefault = useCallback((value: boolean) => {
    dispatch({ type: "set_fields", payload: { isDefault: value } });
  }, []);

  const setAvatarPath = useCallback((value: string | null) => {
    dispatch({ type: "set_fields", payload: { avatarPath: value } });
  }, []);

  const setAvatarCrop = useCallback((value: AvatarCrop | null) => {
    dispatch({ type: "set_fields", payload: { avatarCrop: value } });
  }, []);

  const setAvatarRoundPath = useCallback((value: string | null) => {
    dispatch({ type: "set_fields", payload: { avatarRoundPath: value } });
  }, []);

  const setDesignDescription = useCallback((value: string) => {
    dispatch({ type: "set_fields", payload: { designDescription: value } });
  }, []);

  const setDesignReferenceImageIds = useCallback((value: string[]) => {
    dispatch({ type: "set_fields", payload: { designReferenceImageIds: value } });
  }, []);

  const handleSave = useCallback(async () => {
    if (!personaId) {
      return;
    }

    const {
      title,
      description,
      nickname,
      isDefault,
      avatarPath,
      avatarCrop,
      avatarRoundPath,
      designDescription,
      designReferenceImageIds: rawDesignReferenceImageIds,
    } = state;
    if (!title.trim() || !description.trim()) {
      return;
    }

    dispatch({ type: "set_saving", payload: true });
    dispatch({ type: "set_error", payload: null });

    try {
      const previousDesignReferenceImageIds = initialStateRef.current
        ? (JSON.parse(initialStateRef.current.designReferenceImageIds) as string[])
        : [];

      // Save avatar if provided
      let avatarFilename: string | undefined = undefined;
      if (avatarPath) {
        if (avatarPath.startsWith("data:")) {
          avatarFilename = await saveAvatar("persona", personaId, avatarPath, avatarRoundPath);
          if (!avatarFilename) {
            console.error("[EditPersona] Failed to save avatar image");
          } else {
            invalidateAvatarCache("persona", personaId);
          }
        } else {
          avatarFilename =
            persistedAvatarRef.current.url && avatarPath === persistedAvatarRef.current.url
              ? persistedAvatarRef.current.filename
              : avatarPath;
        }
      }

      const designReferenceImageIds = (
        await Promise.all(
          rawDesignReferenceImageIds.map(async (value) => {
            if (!value) return null;
            if (value.startsWith("data:")) {
              const imageId = await convertToImageRef(value);
              return imageId || null;
            }
            return value;
          }),
        )
      ).filter((value): value is string => typeof value === "string" && value.length > 0);

      await savePersona({
        id: personaId,
        title: title.trim(),
        description: description.trim(),
        nickname: nickname.trim() || undefined,
        isDefault,
        avatarPath: avatarFilename,
        avatarCrop: avatarFilename ? (avatarCrop ?? undefined) : undefined,
        designDescription: designDescription.trim() || undefined,
        designReferenceImageIds:
          designReferenceImageIds.length > 0 ? designReferenceImageIds : undefined,
      });

      // Update initial state to match current (for change detection)
      initialStateRef.current = {
        title: title.trim(),
        description: description.trim(),
        nickname: nickname.trim(),
        isDefault,
        avatarPath,
        avatarCrop: JSON.stringify(avatarCrop ?? null),
        avatarRoundPath: JSON.stringify(avatarRoundPath ?? null),
        designDescription: designDescription.trim(),
        designReferenceImageIds: JSON.stringify(designReferenceImageIds),
      };

      // Sync trimmed values
      dispatch({
        type: "set_fields",
        payload: {
          title: title.trim(),
          description: description.trim(),
          nickname: nickname.trim(),
          designDescription: designDescription.trim(),
          designReferenceImageIds,
        },
      });

      const removedDesignReferenceImageIds = previousDesignReferenceImageIds.filter(
        (imageId) => !designReferenceImageIds.includes(imageId),
      );
      if (removedDesignReferenceImageIds.length > 0) {
        await Promise.all(removedDesignReferenceImageIds.map((imageId) => deleteImageRef(imageId)));
      }
    } catch (error: any) {
      console.error("Failed to save persona:", error);
      dispatch({
        type: "set_error",
        payload: error?.message || "Failed to save persona",
      });
    } finally {
      dispatch({ type: "set_saving", payload: false });
    }
  }, [personaId, state]);

  const resetToInitial = useCallback(() => {
    const initial = initialStateRef.current;
    if (!initial) return;
    dispatch({
      type: "set_fields",
      payload: {
        title: initial.title,
        description: initial.description,
        nickname: initial.nickname,
        isDefault: initial.isDefault,
        avatarPath: initial.avatarPath,
        avatarCrop: JSON.parse(initial.avatarCrop) as AvatarCrop | null,
        avatarRoundPath: JSON.parse(initial.avatarRoundPath) as string | null,
        designDescription: initial.designDescription,
        designReferenceImageIds: JSON.parse(initial.designReferenceImageIds) as string[],
      },
    });
    dispatch({ type: "set_error", payload: null });
  }, []);

  // Compute canSave based on changes from initial state
  const canSave = (() => {
    // Must have name and description
    if (!state.title.trim() || !state.description.trim() || state.saving) return false;

    // If initial state not yet loaded, don't allow save
    const initial = initialStateRef.current;
    if (!initial) return false;

    // Check for actual changes
    const hasChanges =
      state.title !== initial.title ||
      state.description !== initial.description ||
      state.nickname !== initial.nickname ||
      state.isDefault !== initial.isDefault ||
      state.avatarPath !== initial.avatarPath ||
      JSON.stringify(state.avatarCrop ?? null) !== initial.avatarCrop ||
      JSON.stringify(state.avatarRoundPath ?? null) !== initial.avatarRoundPath ||
      state.designDescription !== initial.designDescription ||
      JSON.stringify(state.designReferenceImageIds) !== initial.designReferenceImageIds;

    return hasChanges;
  })();

  return {
    state,
    setTitle,
    setDescription,
    setNickname,
    setIsDefault,
    setAvatarPath,
    setAvatarCrop,
    setAvatarRoundPath,
    setDesignDescription,
    setDesignReferenceImageIds,
    handleSave,
    resetToInitial,
    canSave,
  };
}
