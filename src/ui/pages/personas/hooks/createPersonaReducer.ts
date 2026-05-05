import { useMemo, useReducer } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { savePersona } from "../../../../core/storage/repo";
import { saveAvatar } from "../../../../core/storage/avatars";
import { convertToImageRef } from "../../../../core/storage/images";
import { invalidateAvatarCache } from "../../../hooks/useAvatar";
import { importPersona, readFileAsText } from "../../../../core/storage/personaTransfer";
import { toast } from "../../../components/toast";
import { useI18n } from "../../../../core/i18n/context";
import type { AvatarCrop } from "../../../../core/storage/schemas";

export interface PersonaFormState {
  title: string;
  description: string;
  nickname: string;
  avatarPath: string | null;
  avatarCrop: AvatarCrop | null;
  avatarRoundPath: string | null;
  designDescription: string;
  designReferenceImageIds: string[];
  activeLorebookIds: string[];
  isDefault: boolean;
  saving: boolean;
  importing: boolean;
  error: string | null;
}

export type PersonaFormAction =
  | { type: "set_title"; value: string }
  | { type: "set_description"; value: string }
  | { type: "set_nickname"; value: string }
  | { type: "set_avatar_path"; value: string | null }
  | { type: "set_avatar_crop"; value: AvatarCrop | null }
  | { type: "set_avatar_round_path"; value: string | null }
  | { type: "set_design_description"; value: string }
  | { type: "set_design_reference_image_ids"; value: string[] }
  | { type: "set_active_lorebook_ids"; value: string[] }
  | { type: "set_default"; value: boolean }
  | { type: "set_saving"; value: boolean }
  | { type: "set_importing"; value: boolean }
  | { type: "set_error"; value: string | null }
  | {
      type: "hydrate_from_import";
      payload: {
        title: string;
        description: string;
        nickname?: string;
        avatarPath: string | null;
        avatarCrop?: AvatarCrop | null;
        avatarRoundPath?: string | null;
        designDescription?: string;
        designReferenceImageIds?: string[];
        activeLorebookIds?: string[];
      };
    };

export const initialCreatePersonaState: PersonaFormState = {
  title: "",
  description: "",
  nickname: "",
  avatarPath: null,
  avatarCrop: null,
  avatarRoundPath: null,
  designDescription: "",
  designReferenceImageIds: [],
  activeLorebookIds: [],
  isDefault: false,
  saving: false,
  importing: false,
  error: null,
};

export function createPersonaReducer(
  state: PersonaFormState,
  action: PersonaFormAction,
): PersonaFormState {
  switch (action.type) {
    case "set_title":
      return { ...state, title: action.value };
    case "set_description":
      return { ...state, description: action.value };
    case "set_nickname":
      return { ...state, nickname: action.value };
    case "set_avatar_path":
      return { ...state, avatarPath: action.value };
    case "set_avatar_crop":
      return { ...state, avatarCrop: action.value };
    case "set_avatar_round_path":
      return { ...state, avatarRoundPath: action.value };
    case "set_design_description":
      return { ...state, designDescription: action.value };
    case "set_design_reference_image_ids":
      return { ...state, designReferenceImageIds: action.value };
    case "set_active_lorebook_ids":
      return { ...state, activeLorebookIds: action.value };
    case "set_default":
      return { ...state, isDefault: action.value };
    case "set_saving":
      return { ...state, saving: action.value };
    case "set_importing":
      return { ...state, importing: action.value };
    case "set_error":
      return { ...state, error: action.value };
    case "hydrate_from_import":
      return {
        ...state,
        title: action.payload.title,
        description: action.payload.description,
        nickname: action.payload.nickname ?? "",
        avatarPath: action.payload.avatarPath,
        avatarCrop: action.payload.avatarCrop ?? null,
        avatarRoundPath: action.payload.avatarRoundPath ?? null,
        designDescription: action.payload.designDescription ?? "",
        designReferenceImageIds: action.payload.designReferenceImageIds ?? [],
        activeLorebookIds: action.payload.activeLorebookIds ?? [],
        isDefault: false,
      };
    default:
      return state;
  }
}

export function useCreatePersonaController() {
  const navigate = useNavigate();
  const location = useLocation();
  const { t } = useI18n();
  const [state, dispatch] = useReducer(createPersonaReducer, initialCreatePersonaState);

  const canSave = useMemo(
    () => state.title.trim().length > 0 && state.description.trim().length > 0 && !state.saving,
    [state.title, state.description, state.saving],
  );

  const topNavPath = useMemo(
    () => location.pathname + location.search,
    [location.pathname, location.search],
  );

  const handleAvatarUpload = () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/*";
    input.onchange = (event) => {
      const file = (event.target as HTMLInputElement).files?.[0];
      if (!file) return;

      const reader = new FileReader();
      reader.onload = () => {
        dispatch({ type: "set_avatar_path", value: reader.result as string });
        dispatch({ type: "set_avatar_crop", value: null });
        dispatch({ type: "set_avatar_round_path", value: null });
      };
      reader.readAsDataURL(file);
    };
    input.click();
  };

  const handleImport = async () => {
    const input = document.createElement("input");
    input.type = "file";
    input.onchange = async (event) => {
      const file = (event.target as HTMLInputElement).files?.[0];
      if (!file) return;

      try {
        dispatch({ type: "set_importing", value: true });
        if (file.name.toLowerCase().endsWith(".json")) {
          toast.warning(
            t("personas.importToast.legacyJsonTitle"),
            t("personas.importToast.legacyJsonMessage"),
          );
        }
        const jsonContent = await readFileAsText(file);
        const importedPersona = await importPersona(jsonContent);

        dispatch({
          type: "hydrate_from_import",
          payload: {
            title: importedPersona.title,
            description: importedPersona.description,
            nickname: importedPersona.nickname ?? "",
            avatarPath: importedPersona.avatarPath || null,
            avatarCrop: importedPersona.avatarCrop ?? null,
            designDescription: importedPersona.designDescription ?? "",
            designReferenceImageIds: importedPersona.designReferenceImageIds ?? [],
            activeLorebookIds: importedPersona.activeLorebookIds ?? [],
          },
        });
        dispatch({ type: "set_error", value: null });

        alert(t("personas.importToast.successMessage"));
        navigate(`/personas/${importedPersona.id}/edit`);
      } catch (err: any) {
        console.error("Failed to import persona:", err);
        alert(err?.message || t("personas.errors.importFailed"));
      } finally {
        dispatch({ type: "set_importing", value: false });
      }
    };

    input.click();
  };

  const handleSave = async () => {
    if (!canSave) return;

    try {
      dispatch({ type: "set_saving", value: true });
      dispatch({ type: "set_error", value: null });

      const personaId = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;

      let avatarFilename: string | undefined;
      if (state.avatarPath) {
        avatarFilename = await saveAvatar(
          "persona",
          personaId,
          state.avatarPath,
          state.avatarRoundPath,
        );
        if (!avatarFilename) {
          console.error("[CreatePersona] Failed to save avatar image");
        } else {
          invalidateAvatarCache("persona", personaId);
        }
      }

      const designReferenceImageIds = (
        await Promise.all(
          state.designReferenceImageIds.map(async (value) => {
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
        title: state.title.trim(),
        description: state.description.trim(),
        nickname: state.nickname.trim() || undefined,
        avatarPath: avatarFilename,
        avatarCrop: avatarFilename ? (state.avatarCrop ?? undefined) : undefined,
        designDescription: state.designDescription.trim() || undefined,
        designReferenceImageIds:
          designReferenceImageIds.length > 0 ? designReferenceImageIds : undefined,
        activeLorebookIds: state.activeLorebookIds,
        isDefault: state.isDefault,
      });

      navigate("/library?view=personas", { replace: true });
    } catch (e: any) {
      dispatch({ type: "set_error", value: e?.message || t("personas.errors.saveFailed") });
    } finally {
      dispatch({ type: "set_saving", value: false });
    }
  };

  return {
    state,
    dispatch,
    canSave,
    handleAvatarUpload,
    handleImport,
    handleSave,
    topNavPath,
  };
}
