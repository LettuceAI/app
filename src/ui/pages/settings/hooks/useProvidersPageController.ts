import { useCallback, useEffect, useReducer } from "react";
import { invoke } from "@tauri-apps/api/core";

import {
  readSettings,
  addOrUpdateProviderCredential,
  removeProviderCredential,
  SETTINGS_UPDATED_EVENT,
} from "../../../../core/storage/repo";
import {
  getProviderCapabilities,
  toCamel,
  type ProviderCapabilitiesCamel,
} from "../../../../core/providers/capabilities";
import type { ProviderCredential } from "../../../../core/storage/schemas";
import {
  initialProvidersPageState,
  providersPageReducer,
  type ProvidersPageState,
} from "./providersPageReducer";
import { getPlatform } from "../../../../core/utils/platform";

type VerifyProviderApiKeyResult = {
  providerId: string;
  valid: boolean;
  status?: number;
  error?: string;
  details?: unknown;
};

type ControllerReturn = {
  state: ProvidersPageState;
  reload: () => Promise<void>;
  openEditor: (provider?: ProviderCredential) => void;
  closeEditor: () => void;
  setSelectedProvider: (provider: ProviderCredential | null) => void;
  setApiKey: (value: string) => void;
  setValidationError: (message: string | null) => void;
  updateEditorProvider: (updates: Partial<ProviderCredential>) => void;
  handleSaveProvider: () => Promise<void>;
  handleDeleteProvider: (id: string) => Promise<void>;
  dismissEngineSetup: () => void;
};

export function useProvidersPageController(): ControllerReturn {
  const [state, dispatch] = useReducer(providersPageReducer, initialProvidersPageState);
  const isMobile = getPlatform().type === "mobile";

  const reload = useCallback(async () => {
    const settings = await readSettings();
    console.log(
      "[ProvidersPage] Loaded providers from storage:",
      settings.providerCredentials.map((p) => ({
        id: p.id,
        label: p.label,
        baseUrl: p.baseUrl,
        providerId: p.providerId,
      })),
    );
    const visibleProviders = settings.providerCredentials.filter(
      (provider) => provider.providerId !== "llamacpp",
    );
    dispatch({ type: "set_providers", payload: visibleProviders });
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  // Load provider capabilities from backend
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const caps = (await getProviderCapabilities()).map(toCamel);
        if (!cancelled) {
          dispatch({ type: "set_capabilities", payload: caps });
        }
      } catch (e) {
        console.warn("[ProvidersPage] Failed to load provider capabilities", e);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const handler = () => {
      void reload();
    };
    window.addEventListener(SETTINGS_UPDATED_EVENT, handler);
    return () => window.removeEventListener(SETTINGS_UPDATED_EVENT, handler);
  }, [reload]);

  const openEditor = useCallback(
    (provider?: ProviderCredential) => {
      const newId = provider?.id ?? crypto.randomUUID();
      const firstCap: ProviderCapabilitiesCamel | undefined = (
        isMobile
          ? state.capabilities.filter((capability) => capability.id !== "llamacpp")
          : state.capabilities
      )[0];
      const baseProvider: ProviderCredential = provider
        ? { ...provider }
        : ({
            id: newId,
            providerId: firstCap?.id || "openai",
            label: "",
            apiKey: "",
          } as ProviderCredential);

      console.log("[ProvidersPage] Opening editor with provider:", {
        id: baseProvider.id,
        label: baseProvider.label,
        baseUrl: baseProvider.baseUrl,
        providerId: baseProvider.providerId,
      });

      dispatch({ type: "set_selected_provider", payload: null });
      dispatch({
        type: "open_editor",
        payload: { provider: baseProvider, apiKey: "" },
      });

      dispatch({ type: "set_api_key", payload: provider?.apiKey || "" });
    },
    [isMobile, state.capabilities],
  );

  const closeEditor = useCallback(() => {
    dispatch({ type: "close_editor" });
  }, []);

  const setSelectedProvider = useCallback((provider: ProviderCredential | null) => {
    dispatch({ type: "set_selected_provider", payload: provider });
  }, []);

  const setApiKey = useCallback((value: string) => {
    dispatch({ type: "set_api_key", payload: value });
  }, []);

  const setValidationError = useCallback((message: string | null) => {
    dispatch({ type: "set_validation_error", payload: message });
  }, []);

  const updateEditorProvider = useCallback((updates: Partial<ProviderCredential>) => {
    dispatch({ type: "update_editor_provider", payload: updates });
  }, []);

  const handleSaveProvider = useCallback(async () => {
    const { editorProvider, apiKey } = state;
    if (!editorProvider) return;

    dispatch({ type: "set_validation_error", payload: null });
    dispatch({ type: "set_is_saving", payload: true });

    try {
      const isEngineProvider = editorProvider.providerId === "lettuce-engine";
      const isLocalProvider = ["custom", "custom-anthropic", "ollama", "lmstudio"].includes(
        editorProvider.providerId,
      );
      const requiresVerification =
        !isLocalProvider &&
        !isEngineProvider &&
        ["openai", "anthropic", "openrouter", "groq", "mistral"].includes(
          editorProvider.providerId,
        );
      const trimmedKey = apiKey.trim();

      // Engine requires a base URL
      if (isEngineProvider) {
        const engineUrl = editorProvider.baseUrl?.trim().replace(/\/+$/, "");
        if (!engineUrl) {
          dispatch({
            type: "set_validation_error",
            payload: "Engine URL is required (e.g., http://localhost:8000)",
          });
          return;
        }

        // Health check
        try {
          await invoke("engine_health", {
            baseUrl: engineUrl,
            apiKey: trimmedKey || null,
          });
        } catch (error) {
          dispatch({
            type: "set_validation_error",
            payload: `Cannot reach Engine: ${error instanceof Error ? error.message : String(error)}`,
          });
          return;
        }

        // Save the provider with normalized URL
        const providerToSave: ProviderCredential = {
          ...editorProvider,
          baseUrl: engineUrl,
          apiKey: trimmedKey || undefined,
        } as ProviderCredential;
        await addOrUpdateProviderCredential(providerToSave);
        await reload();
        dispatch({ type: "close_editor" });

        // Check setup status
        try {
          const setupStatus = await invoke<{ needs_setup: boolean }>("engine_setup_status", {
            baseUrl: engineUrl,
            apiKey: trimmedKey || null,
          });
          dispatch({
            type: "set_engine_setup_result",
            payload: {
              credentialId: editorProvider.id,
              needsSetup: setupStatus.needs_setup,
            },
          });
        } catch {
          // If setup status fails, still navigate to home (user can retry)
          dispatch({
            type: "set_engine_setup_result",
            payload: { credentialId: editorProvider.id, needsSetup: false },
          });
        }
        return;
      }

      const requiresBaseUrl = ["ollama", "lmstudio"].includes(editorProvider.providerId);
      if (requiresBaseUrl && !editorProvider.baseUrl?.trim()) {
        dispatch({
          type: "set_validation_error",
          payload: "Base URL is required (e.g., http://localhost:11434)",
        });
        return;
      }

      const isCustomProvider =
        editorProvider.providerId === "custom" || editorProvider.providerId === "custom-anthropic";
      if (isCustomProvider) {
        const cfg = (editorProvider.config ?? {}) as Record<string, unknown>;
        const fetchModelsEnabled = cfg.fetchModelsEnabled === true;
        const modelsEndpoint =
          typeof cfg.modelsEndpoint === "string" ? cfg.modelsEndpoint.trim() : "";
        if (fetchModelsEnabled && !modelsEndpoint) {
          dispatch({
            type: "set_validation_error",
            payload: "Models endpoint is required when model fetching is enabled.",
          });
          return;
        }
      }

      if (requiresVerification) {
        if (!trimmedKey) {
          dispatch({
            type: "set_validation_error",
            payload: "API key required",
          });
          return;
        }

        let verification: VerifyProviderApiKeyResult;
        try {
          verification = await invoke<VerifyProviderApiKeyResult>("verify_provider_api_key", {
            providerId: editorProvider.providerId,
            credentialId: editorProvider.id,
            apiKey: trimmedKey,
            baseUrl: editorProvider.baseUrl ?? null,
          });
        } catch (error) {
          dispatch({
            type: "set_validation_error",
            payload: error instanceof Error ? error.message : String(error),
          });
          return;
        }

        if (!verification.valid) {
          dispatch({
            type: "set_validation_error",
            payload: verification.error || "Invalid API key",
          });
          return;
        }
      }

      const providerToSave: ProviderCredential = {
        ...editorProvider,
        apiKey: trimmedKey || undefined,
      } as ProviderCredential;

      await addOrUpdateProviderCredential(providerToSave);

      await reload();
      dispatch({ type: "close_editor" });
    } finally {
      dispatch({ type: "set_is_saving", payload: false });
    }
  }, [state, reload]);

  const handleDeleteProvider = useCallback(
    async (id: string) => {
      dispatch({ type: "set_is_deleting", payload: true });
      try {
        await removeProviderCredential(id);
        await reload();
        dispatch({ type: "set_selected_provider", payload: null });
      } finally {
        dispatch({ type: "set_is_deleting", payload: false });
      }
    },
    [reload],
  );

  const dismissEngineSetup = useCallback(() => {
    dispatch({ type: "set_engine_setup_result", payload: null });
  }, []);

  return {
    state,
    reload,
    openEditor,
    closeEditor,
    setSelectedProvider,
    setApiKey,
    setValidationError,
    updateEditorProvider,
    handleSaveProvider,
    handleDeleteProvider,
    dismissEngineSetup,
  };
}
