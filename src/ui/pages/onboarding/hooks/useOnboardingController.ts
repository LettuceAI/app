import { useCallback, useEffect, useMemo, useReducer } from "react";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";

import { getProviderCapabilities, toCamel } from "../../../../core/providers/capabilities";
import {
  addOrUpdateProviderCredential,
  readSettings,
  addOrUpdateModel,
} from "../../../../core/storage/repo";
import {
  setProviderSetupCompleted,
  setModelSetupCompleted,
  setOnboardingCompleted,
} from "../../../../core/storage/appState";
import { storageBridge } from "../../../../core/storage/files";
import { checkEmbeddingModel } from "../../../../core/storage/repo";
import type { ProviderCredential, Model, Settings } from "../../../../core/storage/schemas";
import { isImageOnlyProvider } from "../../../../core/storage/schemas";

import {
  OnboardingStep,
  initialOnboardingState,
  onboardingReducer,
  getDefaultBaseUrl,
  getDefaultModelName,
  type OnboardingState,
  type MemoryType,
} from "./onboardingReducer";

export { OnboardingStep };

export interface OnboardingController {
  state: OnboardingState;

  // Navigation
  setStep: (step: OnboardingStep) => void;
  goBack: () => void;

  // Provider
  canTestProvider: boolean;
  canSaveProvider: boolean;
  handleSelectProvider: (provider: { id: string; name: string; defaultBaseUrl?: string }) => void;
  handleProviderLabelChange: (value: string) => void;
  handleApiKeyChange: (value: string) => void;
  handleBaseUrlChange: (value: string) => void;
  handleConfigChange: (config: Record<string, any> | undefined) => void;
  handleTestConnection: () => Promise<void>;
  handleSaveProvider: () => Promise<void>;

  // Model
  canSaveModel: boolean;
  handleSelectCredential: (credential: ProviderCredential) => void;
  handleModelNameChange: (value: string) => void;
  handleDisplayNameChange: (value: string) => void;
  handleSaveModel: () => Promise<void>;
  handleSkipModel: () => void;

  // Memory
  handleSelectMemoryType: (type: MemoryType) => void;
  handleFinish: () => Promise<void>;
}

export function useOnboardingController(): OnboardingController {
  const navigate = useNavigate();
  const [state, dispatch] = useReducer(onboardingReducer, initialOnboardingState);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const caps = (await getProviderCapabilities()).map(toCamel);
        if (!cancelled) {
          dispatch({ type: "SET_CAPABILITIES", payload: caps });
        }
      } catch (e) {
        console.warn("[Onboarding] Failed to load provider capabilities", e);
        if (!cancelled) {
          dispatch({ type: "SET_CAPABILITIES_LOADING", payload: false });
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (state.step !== OnboardingStep.Model) return;

    let isMounted = true;
    dispatch({ type: "SET_MODEL_LOADING", payload: true });

    (async () => {
      try {
        const settings = await readSettings();
        if (!isMounted) return;

        if (settings.providerCredentials.length > 0) {
          const firstProvider = settings.providerCredentials[0];
          const defaultModel = getDefaultModelName(firstProvider.providerId);

          dispatch({
            type: "LOAD_CREDENTIALS",
            payload: {
              credentials: settings.providerCredentials,
              selected: firstProvider,
              modelName: defaultModel,
              displayName: defaultModel,
            },
          });
        } else {
          dispatch({
            type: "LOAD_CREDENTIALS",
            payload: {
              credentials: [],
              selected: null,
              modelName: "",
              displayName: "",
            },
          });
        }
      } catch (error) {
        console.error("Failed to load providers:", error);
        if (isMounted) {
          dispatch({ type: "SET_MODEL_LOADING", payload: false });
        }
      }
    })();

    return () => {
      isMounted = false;
    };
  }, [state.step]);

  const setStep = useCallback((step: OnboardingStep) => {
    dispatch({ type: "SET_STEP", payload: step });
  }, []);

  const goBack = useCallback(() => {
    if (state.step === OnboardingStep.Memory) {
      dispatch({ type: "SET_STEP", payload: OnboardingStep.Model });
      navigate("/onboarding/models");
    } else if (state.step === OnboardingStep.Model) {
      dispatch({ type: "SET_STEP", payload: OnboardingStep.Provider });
      navigate("/onboarding/provider");
    } else {
      navigate("/welcome");
    }
  }, [state.step, navigate]);

  const handleSelectProvider = useCallback(
    (provider: { id: string; name: string; defaultBaseUrl?: string }) => {
      // Set defaults for custom providers
      let config: Record<string, any> | undefined = undefined;
      if (provider.id === "custom") {
        config = {
          chatEndpoint: "/v1/chat/completions",
          systemRole: "system",
          userRole: "user",
          assistantRole: "assistant",
          toolChoiceMode: "auto",
          supportsStream: true,
          mergeSameRoleMessages: true,
        };
      } else if (provider.id === "custom-anthropic") {
        config = {
          chatEndpoint: "/v1/messages",
          systemRole: "system",
          userRole: "user",
          assistantRole: "assistant",
          supportsStream: true,
          mergeSameRoleMessages: true,
        };
      }

      dispatch({
        type: "SELECT_PROVIDER",
        payload: {
          providerId: provider.id,
          label: `My ${provider.name}`,
          baseUrl: provider.defaultBaseUrl || getDefaultBaseUrl(provider.id),
          config,
        },
      });
    },
    [],
  );

  const handleProviderLabelChange = useCallback((value: string) => {
    dispatch({ type: "SET_PROVIDER_LABEL", payload: value });
  }, []);

  const handleApiKeyChange = useCallback((value: string) => {
    dispatch({ type: "SET_API_KEY", payload: value });
  }, []);

  const handleBaseUrlChange = useCallback((value: string) => {
    dispatch({ type: "SET_BASE_URL", payload: value });
  }, []);

  const handleConfigChange = useCallback((config: Record<string, any> | undefined) => {
    dispatch({ type: "SET_CONFIG", payload: config });
  }, []);

  const handleTestConnection = useCallback(async () => {
    const isLocalProvider = ["ollama", "lmstudio", "intenserp"].includes(
      state.selectedProviderId || "",
    );
    const requiresBaseUrl = state.selectedProviderId === "lettuce-host";
    const skipValidationProvider = ["chutes"].includes(state.selectedProviderId || "");
    if (
      !state.selectedProviderId ||
      isLocalProvider ||
      skipValidationProvider ||
      !state.apiKey.trim()
    ) {
      return;
    }
    if (requiresBaseUrl && !state.baseUrl.trim()) {
      return;
    }

    dispatch({ type: "SET_TESTING", payload: true });
    dispatch({ type: "SET_TEST_RESULT", payload: null });

    try {
      const trimmedKey = state.apiKey.trim();
      if (state.selectedProviderId !== "lettuce-host" && trimmedKey.length < 10) {
        throw new Error("API key seems too short");
      }

      const verification = await invoke<{
        providerId: string;
        valid: boolean;
        status?: number;
        error?: string;
      }>("verify_provider_api_key", {
        providerId: state.selectedProviderId,
        credentialId: crypto.randomUUID(),
        apiKey: trimmedKey,
        baseUrl: state.baseUrl || null,
      });

      dispatch({
        type: "SET_TEST_RESULT",
        payload: verification.valid
          ? { success: true, message: "Connection successful!" }
          : { success: false, message: verification.error || "Invalid API key" },
      });
    } catch (error: any) {
      dispatch({
        type: "SET_TEST_RESULT",
        payload: { success: false, message: error.message || "Connection failed" },
      });
    } finally {
      dispatch({ type: "SET_TESTING", payload: false });
    }
  }, [state.selectedProviderId, state.apiKey, state.baseUrl]);

  const handleSaveProvider = useCallback(async () => {
    const { selectedProviderId, apiKey, providerLabel, baseUrl } = state;
    const isLocalProvider = [
      "custom",
      "custom-anthropic",
      "ollama",
      "lmstudio",
      "intenserp",
    ].includes(selectedProviderId || "");
    if (!selectedProviderId || !providerLabel.trim() || (!isLocalProvider && !apiKey.trim())) {
      return;
    }

    dispatch({ type: "SET_SUBMITTING_PROVIDER", payload: true });
    dispatch({ type: "SET_TEST_RESULT", payload: null });

    try {
      const credentialId = crypto.randomUUID();
      const trimmedKey = apiKey.trim();
      const requiresVerification =
        !isLocalProvider &&
        ["openai", "anthropic", "openrouter", "gemini", "lettuce-host"].includes(
          selectedProviderId,
        );

      // Local providers require base URL
      if (
        ["ollama", "lmstudio", "intenserp", "lettuce-host"].includes(selectedProviderId) &&
        !baseUrl?.trim()
      ) {
        dispatch({
          type: "SET_TEST_RESULT",
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
        const verification = await invoke<{
          providerId: string;
          valid: boolean;
          status?: number;
          error?: string;
        }>("verify_provider_api_key", {
          providerId: selectedProviderId,
          credentialId,
          apiKey: trimmedKey,
          baseUrl: baseUrl || null,
        });

        if (!verification.valid) {
          dispatch({
            type: "SET_TEST_RESULT",
            payload: { success: false, message: verification.error || "Invalid API key" },
          });
          return;
        }
      }

      const credential: ProviderCredential = {
        id: credentialId,
        providerId: selectedProviderId,
        label: providerLabel.trim(),
        apiKey: trimmedKey || undefined,
        baseUrl: baseUrl || undefined,
        config: state.config,
      };

      const result = await addOrUpdateProviderCredential(credential);
      if (!result) throw new Error("Failed to save provider credential");
      const nextCredentials = [
        result,
        ...state.providerCredentials.filter((existing) => existing.id !== result.id),
      ];

      await setProviderSetupCompleted(true);
      dispatch({
        type: "LOAD_CREDENTIALS",
        payload: {
          credentials: nextCredentials,
          selected: result,
          modelName: getDefaultModelName(result.providerId),
          displayName: getDefaultModelName(result.providerId),
        },
      });
      dispatch({ type: "SET_STEP", payload: OnboardingStep.Model });
      navigate("/onboarding/models");
    } catch (error: any) {
      dispatch({
        type: "SET_TEST_RESULT",
        payload: { success: false, message: error.message || "Failed to save provider" },
      });
    } finally {
      dispatch({ type: "SET_SUBMITTING_PROVIDER", payload: false });
    }
  }, [state]);

  const handleSelectCredential = useCallback((credential: ProviderCredential) => {
    dispatch({
      type: "SELECT_CREDENTIAL",
      payload: {
        credential,
        defaultModel: getDefaultModelName(credential.providerId),
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
    const { selectedCredential, modelName, displayName } = state;
    if (!selectedCredential || !modelName.trim() || !displayName.trim()) return;

    dispatch({ type: "SET_SAVING_MODEL", payload: true });
    dispatch({ type: "SET_MODEL_ERROR", payload: null });

    try {
      const shouldVerify = ["openai", "anthropic"].includes(selectedCredential.providerId);

      if (shouldVerify) {
        const verification = await invoke<{ exists: boolean; error?: string }>(
          "verify_model_exists",
          {
            providerId: selectedCredential.providerId,
            credentialId: selectedCredential.id,
            model: modelName.trim(),
          },
        );

        if (!verification.exists) {
          dispatch({
            type: "SET_MODEL_ERROR",
            payload: verification.error || "Model not found on provider",
          });
          return;
        }
      }

      const imageOnly = isImageOnlyProvider(selectedCredential.providerId);
      const model: Omit<Model, "id"> = {
        name: modelName.trim(),
        providerId: selectedCredential.providerId,
        providerCredentialId: selectedCredential.id,
        providerLabel: selectedCredential.label,
        displayName: displayName.trim(),
        createdAt: Date.now(),
        inputScopes: ["text"],
        outputScopes: imageOnly ? ["image"] : ["text"],
      };

      await addOrUpdateModel(model);
      await setModelSetupCompleted(true);
      dispatch({ type: "SET_STEP", payload: OnboardingStep.Memory });
      navigate("/onboarding/memory");
    } catch (error: any) {
      dispatch({
        type: "SET_MODEL_ERROR",
        payload: error.message || "Failed to save model",
      });
    } finally {
      dispatch({ type: "SET_SAVING_MODEL", payload: false });
    }
  }, [state]);

  const handleSkipModel = useCallback(() => {
    dispatch({ type: "SET_STEP", payload: OnboardingStep.Memory });
    navigate("/onboarding/memory");
  }, [navigate]);

  const handleSelectMemoryType = useCallback((type: MemoryType) => {
    dispatch({ type: "SET_MEMORY_TYPE", payload: type });
  }, []);

  const saveMemorySettings = useCallback(async (enableDynamic: boolean) => {
    dispatch({ type: "SET_PROCESSING_MEMORY", payload: true });
    try {
      const currentSettings = await storageBridge.readSettings<Settings | null>(null);
      if (currentSettings) {
        const advancedSettings = (currentSettings.advancedSettings || {}) as any;
        const dynamicMemory = advancedSettings.dynamicMemory || {
          enabled: false,
          summaryMessageInterval: 20,
          maxEntries: 50,
          minSimilarityThreshold: 0.35,
          retrievalLimit: 5,
          retrievalStrategy: "smart",
          hotMemoryTokenBudget: 2000,
          decayRate: 0.08,
          coldThreshold: 0.3,
        };

        dynamicMemory.enabled = enableDynamic;

        await storageBridge.settingsSetAdvanced({
          ...advancedSettings,
          dynamicMemory,
        });
      }
      await setOnboardingCompleted(true);
    } catch (error) {
      console.error("Failed to save memory settings:", error);
    } finally {
      dispatch({ type: "SET_PROCESSING_MEMORY", payload: false });
    }
  }, []);

  const handleFinish = useCallback(async () => {
    if (!state.memoryType) return;

    if (state.memoryType === "dynamic") {
      try {
        const modelExists = await checkEmbeddingModel();
        if (modelExists) {
          await saveMemorySettings(true);
          navigate("/chat?firstTime=true");
        } else {
          navigate("/settings/embedding-download?returnTo=/chat?firstTime=true");
        }
      } catch (error) {
        console.error("Failed to check embedding model:", error);
        navigate("/settings/embedding-download?returnTo=/chat?firstTime=true");
      }
    } else {
      await saveMemorySettings(false);
      navigate("/chat?firstTime=true");
    }
  }, [state.memoryType, navigate, saveMemorySettings]);

  const canTestProvider = useMemo(() => {
    const isLocalProvider = ["ollama", "lmstudio", "intenserp"].includes(
      state.selectedProviderId || "",
    );
    const requiresBaseUrl = state.selectedProviderId === "lettuce-host";
    const skipValidationProvider = ["chutes"].includes(state.selectedProviderId || "");
    return Boolean(
      state.selectedProviderId &&
      !isLocalProvider &&
      !skipValidationProvider &&
      (!requiresBaseUrl || state.baseUrl.trim().length > 0) &&
      state.apiKey.trim().length > 0,
    );
  }, [state.selectedProviderId, state.apiKey, state.baseUrl]);

  const canSaveProvider = useMemo(() => {
    const isLocalProvider = [
      "custom",
      "custom-anthropic",
      "ollama",
      "lmstudio",
      "intenserp",
    ].includes(state.selectedProviderId || "");
    const requiresBaseUrl = state.selectedProviderId === "lettuce-host";
    return Boolean(
      state.selectedProviderId &&
      state.providerLabel.trim().length > 0 &&
      (isLocalProvider || state.apiKey.trim().length > 0) &&
      (!requiresBaseUrl || state.baseUrl.trim().length > 0),
    );
  }, [state.selectedProviderId, state.apiKey, state.baseUrl, state.providerLabel]);

  const canSaveModel = useMemo(() => {
    return Boolean(
      state.selectedCredential &&
      state.modelName.trim().length > 0 &&
      state.displayName.trim().length > 0,
    );
  }, [state.selectedCredential, state.modelName, state.displayName]);

  return {
    state,
    setStep,
    goBack,
    canTestProvider,
    canSaveProvider,
    handleSelectProvider,
    handleProviderLabelChange,
    handleApiKeyChange,
    handleBaseUrlChange,
    handleConfigChange,
    handleTestConnection,
    handleSaveProvider,
    canSaveModel,
    handleSelectCredential,
    handleModelNameChange,
    handleDisplayNameChange,
    handleSaveModel,
    handleSkipModel,
    handleSelectMemoryType,
    handleFinish,
  };
}
