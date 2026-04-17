import { useCallback, useEffect, useMemo, useReducer } from "react";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";

import { addOrUpdateProviderCredential } from "../../../../core/storage/repo";
import { setProviderSetupCompleted } from "../../../../core/storage/appState";
import type { ProviderCredential } from "../../../../core/storage/schemas";

import {
  getDefaultBaseUrl,
  initialProviderState,
  providerReducer,
  type ProviderState,
} from "./providerReducer";

type ControllerReturn = {
  state: ProviderState;
  canTest: boolean;
  canSave: boolean;
  handleSelectProvider: (provider: { id: string; name: string; defaultBaseUrl?: string }) => void;
  handleLabelChange: (value: string) => void;
  handleApiKeyChange: (value: string) => void;
  handleBaseUrlChange: (value: string) => void;
  handleTestConnection: () => Promise<void>;
  handleSaveProvider: () => Promise<void>;
  goToWelcome: () => void;
};

export function useProviderController(): ControllerReturn {
  const navigate = useNavigate();
  const [state, dispatch] = useReducer(providerReducer, initialProviderState);
  const { selectedProviderId, apiKey, label, baseUrl, showForm } = state;

  useEffect(() => {
    if (!showForm) return;

    const timeout = window.setTimeout(() => {
      const formElement = document.getElementById("provider-config-form");
      if (formElement) {
        formElement.scrollIntoView({
          behavior: "smooth",
          block: "end",
          inline: "nearest",
        });
      }
    }, 350);

    return () => window.clearTimeout(timeout);
  }, [showForm]);

  const handleSelectProvider = useCallback(
    (provider: { id: string; name: string; defaultBaseUrl?: string }) => {
      dispatch({
        type: "select_provider",
        payload: {
          providerId: provider.id,
          label: `My ${provider.name}`,
          baseUrl: provider.defaultBaseUrl || getDefaultBaseUrl(provider.id),
        },
      });
    },
    [],
  );

  const handleLabelChange = useCallback((value: string) => {
    dispatch({ type: "set_label", payload: value });
  }, []);

  const handleApiKeyChange = useCallback((value: string) => {
    dispatch({ type: "set_api_key", payload: value });
  }, []);

  const handleBaseUrlChange = useCallback((value: string) => {
    dispatch({ type: "set_base_url", payload: value });
  }, []);

  const handleTestConnection = useCallback(async () => {
    const isLocalProvider = ["ollama", "lmstudio", "intenserp"].includes(selectedProviderId || "");
    const requiresBaseUrl = selectedProviderId === "lettuce-host";
    const skipValidationProvider = ["chutes"].includes(selectedProviderId || "");
    if (!selectedProviderId || isLocalProvider || skipValidationProvider || !apiKey.trim()) {
      return;
    }
    if (requiresBaseUrl && !baseUrl.trim()) {
      return;
    }

    dispatch({ type: "set_is_testing", payload: true });
    dispatch({ type: "set_test_result", payload: null });

    try {
      const trimmedKey = apiKey.trim();

      if (selectedProviderId !== "lettuce-host" && trimmedKey.length < 10) {
        throw new Error("API key seems too short");
      }

      const verification = await invoke<{
        providerId: string;
        valid: boolean;
        status?: number;
        error?: string;
      }>("verify_provider_api_key", {
        providerId: selectedProviderId,
        credentialId: crypto.randomUUID(),
        apiKey: trimmedKey,
        baseUrl: baseUrl || null,
      });

      if (!verification.valid) {
        dispatch({
          type: "set_test_result",
          payload: {
            success: false,
            message: verification.error || "Invalid API key",
          },
        });
      } else {
        dispatch({
          type: "set_test_result",
          payload: { success: true, message: "Connection successful!" },
        });
      }
    } catch (error: any) {
      dispatch({
        type: "set_test_result",
        payload: {
          success: false,
          message: error.message || "Connection failed",
        },
      });
    } finally {
      dispatch({ type: "set_is_testing", payload: false });
    }
  }, [apiKey, baseUrl, selectedProviderId]);

  const handleSaveProvider = useCallback(async () => {
    const isLocalProvider = ["ollama", "lmstudio", "intenserp"].includes(selectedProviderId || "");
    const requiresBaseUrl = ["ollama", "lmstudio", "intenserp", "lettuce-host"].includes(
      selectedProviderId || "",
    );
    if (!selectedProviderId || !label.trim() || (!isLocalProvider && !apiKey.trim())) {
      return;
    }

    dispatch({ type: "set_is_submitting", payload: true });
    dispatch({ type: "set_test_result", payload: null });

    try {
      const credentialId = crypto.randomUUID();
      const trimmedKey = apiKey.trim();

      const requiresVerification = [
        "openai",
        "anthropic",
        "openrouter",
        "gemini",
        "lettuce-host",
        "pollinations",
        "pollinations-image",
      ].includes(selectedProviderId);

      if (requiresBaseUrl && !baseUrl?.trim()) {
        dispatch({
          type: "set_test_result",
          payload: {
            success: false,
            message:
              selectedProviderId === "lettuce-host"
                ? "Host URL is required (e.g., http://192.168.1.10:3333)"
                : "Base URL is required (e.g., http://localhost:11434)",
          },
        });
        return;
      }

      if (requiresVerification) {
        try {
          const verification = await invoke<{
            providerId: string;
            valid: boolean;
            status?: number;
            error?: string;
          }>("verify_provider_api_key", {
            providerId: selectedProviderId,
            credentialId: credentialId,
            apiKey: trimmedKey,
            baseUrl: baseUrl || null,
          });

          if (!verification.valid) {
            dispatch({
              type: "set_test_result",
              payload: {
                success: false,
                message: verification.error || "Invalid API key",
              },
            });
            return;
          }
        } catch (error: any) {
          dispatch({
            type: "set_test_result",
            payload: {
              success: false,
              message: error.message || "Verification failed",
            },
          });
          return;
        }
      }

      const credential: Omit<ProviderCredential, "id"> & { id: string } = {
        id: credentialId,
        providerId: selectedProviderId,
        label: label.trim(),
        apiKey: trimmedKey || undefined,
        baseUrl: baseUrl || undefined,
      };

      const result = await addOrUpdateProviderCredential(credential);

      if (!result) {
        throw new Error("Failed to save provider credential");
      }

      // No secrets store: apiKey is saved inline now

      await setProviderSetupCompleted(true);
      navigate("/onboarding/models");
    } catch (error: any) {
      console.log(error);
      dispatch({
        type: "set_test_result",
        payload: {
          success: false,
          message: error.message || "Failed to save provider",
        },
      });
    } finally {
      dispatch({ type: "set_is_submitting", payload: false });
    }
  }, [apiKey, baseUrl, label, navigate, selectedProviderId]);

  const canTest = useMemo(() => {
    const isLocalProvider = ["ollama", "lmstudio", "intenserp"].includes(selectedProviderId || "");
    const requiresBaseUrl = selectedProviderId === "lettuce-host";
    const skipValidationProvider = ["chutes"].includes(selectedProviderId || "");
    return Boolean(
      selectedProviderId &&
      !isLocalProvider &&
      !skipValidationProvider &&
      (!requiresBaseUrl || baseUrl.trim().length > 0) &&
      apiKey.trim().length > 0,
    );
  }, [apiKey, baseUrl, selectedProviderId]);

  const canSave = useMemo(() => {
    const isLocalProvider = ["ollama", "lmstudio", "intenserp"].includes(selectedProviderId || "");
    const requiresBaseUrl = selectedProviderId === "lettuce-host";
    return Boolean(
      selectedProviderId &&
      label.trim().length > 0 &&
      (isLocalProvider || apiKey.trim().length > 0) &&
      (!requiresBaseUrl || baseUrl.trim().length > 0),
    );
  }, [apiKey, baseUrl, label, selectedProviderId]);

  const goToWelcome = useCallback(() => {
    navigate("/welcome");
  }, [navigate]);

  return {
    state,
    canTest,
    canSave,
    handleSelectProvider,
    handleLabelChange,
    handleApiKeyChange,
    handleBaseUrlChange,
    handleTestConnection,
    handleSaveProvider,
    goToWelcome,
  };
}
