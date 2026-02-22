import { useCallback, useEffect, useMemo, useReducer } from "react";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";

import { readSettings, addOrUpdateModel } from "../../../../core/storage/repo";
import { setModelSetupCompleted } from "../../../../core/storage/appState";
import type { ProviderCredential, Model } from "../../../../core/storage/schemas";

import {
  getDefaultModelName,
  initialModelState,
  modelReducer,
  type ModelState,
} from "./modelReducer";

type ControllerReturn = {
  state: ModelState;
  canSave: boolean;
  handleProviderSelect: (provider: ProviderCredential) => void;
  handleModelNameChange: (value: string) => void;
  handleDisplayNameChange: (value: string) => void;
  handleSaveModel: () => Promise<void>;
  handleSkip: () => Promise<void>;
  goToProviderSetup: () => void;
};

export function useModelController(): ControllerReturn {
  const navigate = useNavigate();
  const [state, dispatch] = useReducer(modelReducer, initialModelState);

  const { selectedProvider, modelName, displayName } = state;

  useEffect(() => {
    let isMounted = true;

    (async () => {
      try {
        const settings = await readSettings();
        if (!isMounted) {
          return;
        }

        if (settings.providerCredentials.length > 0) {
          const firstProvider = settings.providerCredentials[0];
          const defaultModel = getDefaultModelName(firstProvider.providerId);

          dispatch({
            type: "LOAD_SUCCESS",
            payload: {
              providers: settings.providerCredentials,
              selectedProvider: firstProvider,
              modelName: defaultModel,
              displayName: defaultModel,
            },
          });
          return;
        }

        dispatch({
          type: "LOAD_SUCCESS",
          payload: {
            providers: settings.providerCredentials,
            selectedProvider: null,
            modelName: "",
            displayName: "",
          },
        });
      } catch (error) {
        console.error("Failed to load providers:", error);
        if (!isMounted) {
          return;
        }
        dispatch({ type: "LOAD_ERROR" });
      }
    })();

    return () => {
      isMounted = false;
    };
  }, []);

  const handleProviderSelect = useCallback((provider: ProviderCredential) => {
    dispatch({
      type: "SELECT_PROVIDER",
      payload: {
        provider,
        defaultModel: getDefaultModelName(provider.providerId),
      },
    });
  }, []);

  const handleModelNameChange = useCallback((value: string) => {
    dispatch({ type: "SET_MODEL_NAME", payload: value });
  }, []);

  const handleDisplayNameChange = useCallback((value: string) => {
    dispatch({ type: "SET_DISPLAY_NAME", payload: value });
  }, []);

  const handleSaveModel = useCallback(async () => {
    if (!selectedProvider || !modelName.trim() || !displayName.trim()) {
      return;
    }

    dispatch({ type: "SET_SAVING", payload: true });
    dispatch({ type: "SET_VERIFICATION_ERROR", payload: null });

    try {
      const shouldVerify = ["openai", "anthropic"].includes(selectedProvider.providerId);

      if (shouldVerify) {
        try {
          const verification = await invoke<{ exists: boolean; error?: string }>(
            "verify_model_exists",
            {
              providerId: selectedProvider.providerId,
              credentialId: selectedProvider.id,
              model: modelName.trim(),
            },
          );

          if (!verification.exists) {
            dispatch({
              type: "SET_VERIFICATION_ERROR",
              payload: verification.error || "Model not found on provider",
            });
            return;
          }
        } catch (error: any) {
          dispatch({
            type: "SET_VERIFICATION_ERROR",
            payload: error.message || "Model verification failed",
          });
          return;
        }
      }

      const model: Omit<Model, "id"> = {
        name: modelName.trim(),
        providerId: selectedProvider.providerId,
        providerCredentialId: selectedProvider.id,
        providerLabel: selectedProvider.label,
        displayName: displayName.trim(),
        createdAt: Date.now(),
        inputScopes: ["text"],
        outputScopes: ["text"],
      };

      await addOrUpdateModel(model);

      await setModelSetupCompleted(true);

      navigate("/onboarding/memory");
    } catch (error: any) {
      console.error("Failed to save model:", error);
      dispatch({
        type: "SET_VERIFICATION_ERROR",
        payload: error.message || "Failed to save model",
      });
    } finally {
      dispatch({ type: "SET_SAVING", payload: false });
    }
  }, [navigate, selectedProvider, modelName, displayName]);

  const handleSkip = useCallback(async () => {
    navigate("/onboarding/memory");
  }, [navigate]);

  const goToProviderSetup = useCallback(() => {
    navigate("/onboarding/provider");
  }, [navigate]);

  const canSave = useMemo(() => {
    return Boolean(
      selectedProvider && modelName.trim().length > 0 && displayName.trim().length > 0,
    );
  }, [selectedProvider, modelName, displayName]);

  return {
    state,
    canSave,
    handleProviderSelect,
    handleModelNameChange,
    handleDisplayNameChange,
    handleSaveModel,
    handleSkip,
    goToProviderSetup,
  };
}
