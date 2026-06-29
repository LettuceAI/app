import {
  BrowserRouter,
  Route,
  Routes,
  useLocation,
  useNavigate,
  Navigate,
  useParams,
} from "react-router-dom";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Toaster } from "sonner";

import { OnboardingPage } from "./ui/pages/onboarding";
import { WhereToFindPage } from "./ui/pages/onboarding/WhereToFind";
import { SettingsPage } from "./ui/pages/settings/Settings";
import { SettingsLayout } from "./ui/pages/settings/SettingsLayout";
import { ProvidersPage } from "./ui/pages/settings/ProvidersPage";
import { ModelsPage } from "./ui/pages/settings/ModelsPage";
import { EditModelPage } from "./ui/pages/settings/EditModelPage";
import { HuggingFaceBrowserPage } from "./ui/pages/settings/HuggingFaceBrowserPage";
import { InstalledModelsPage } from "./ui/pages/settings/InstalledModelsPage";
import PerformancePage from "./ui/pages/settings/PerformancePage";
import { LocalRuntimeDefaultsPage } from "./ui/pages/settings/LocalRuntimeDefaultsPage";
import { ImageGenerationPage } from "./ui/pages/settings/ImageGenerationPage";
import { SystemPromptsPage } from "./ui/pages/settings/SystemPromptsPage";
import { EditPromptTemplate } from "./ui/pages/settings/EditPromptTemplate";
import { SecurityPage } from "./ui/pages/settings/SecurityPage";
import { ResetPage } from "./ui/pages/settings/ResetPage";
import { BackupRestorePage } from "./ui/pages/settings/BackupRestorePage";
import { UsagePage } from "./ui/pages/settings/UsagePage";
import { UsageActivityPage } from "./ui/pages/settings/UsageActivityPage";
import { CustomizationPage } from "./ui/pages/settings/CustomizationPage";
import { SpeechRecognitionPage } from "./ui/pages/settings/SpeechRecognitionPage";
import { ColorCustomizationPage } from "./ui/pages/settings/ColorCustomizationPage";
import { ChatAppearancePage } from "./ui/pages/settings/ChatAppearancePage";
import { LogsPage } from "./ui/pages/settings/LogsPage";
import { AboutPage } from "./ui/pages/settings/AboutPage";
import { CharactersPage } from "./ui/pages/settings/CharactersPage";
import { DeveloperPage } from "./ui/pages/settings/DeveloperPage";
import { ChangelogPage } from "./ui/pages/settings/ChangelogPage";
import { HelpPage } from "./ui/pages/settings/HelpPage";
import { AdvancedPage } from "./ui/pages/settings/AdvancedPage";
import { CreationHelperPage as AICreationHelperPage } from "./ui/pages/settings/CreationHelperPage";
import { HelpMeReplyPage } from "./ui/pages/settings/HelpMeReplyPage";
import { LorebooksPage } from "./ui/pages/settings/LorebooksPage";
import { LorebookGeneratorFlowPage } from "./ui/pages/library/LorebookGeneratorFlowPage";
import { CompanionsHubPage } from "./ui/pages/settings/CompanionsHubPage";
import { CompanionDownloadQueuePage } from "./ui/pages/settings/CompanionDownloadQueuePage";
import { HostApiPage } from "./ui/pages/settings/HostApiPage";
import { VoicesPage } from "./ui/pages/settings/VoicesPage";
import { DynamicMemoryPage } from "./ui/pages/settings/DynamicMemoryPage";
import { EmbeddingDownloadPage } from "./ui/pages/settings/EmbeddingDownloadPage";
import { CompanionDownloadPage } from "./ui/pages/settings/CompanionDownloadPage";
import { EmbeddingTestPage } from "./ui/pages/settings/EmbeddingTestPage";
import { KokoroTestPage } from "./ui/pages/settings/KokoroTestPage";
import { KokoroStudioPage } from "./ui/pages/settings/KokoroStudioPage";
import { KokoroBlendEditorPage } from "./ui/pages/settings/KokoroBlendEditorPage";
import {
  ChatPage,
  ChatConversationPage,
  ChatSettingsPage,
  ChatHistoryPage,
  ChatTreePage,
  ChatMemoriesPage,
  CompanionMemoryPage,
  CompanionRelationshipPage,
  CompanionSoulPage,
  MessageDebugPage,
  SearchMessagesPage,
  ChatLayout,
} from "./ui/pages/chats";
import { ThemeProvider } from "./core/theme/ThemeContext";
import { toast } from "./ui/components/toast";
import { DownloadQueueProvider } from "./core/downloads/DownloadQueueContext";
import {
  CreateCharacterPage,
  EditCharacterPage,
  LorebookEditor,
  CreationHelperPage,
} from "./ui/pages/characters";
import { CreatePersonaPage, EditPersonaPage } from "./ui/pages/personas";
import ChatTemplateListPage from "./ui/pages/characters/ChatTemplateListPage";
import ChatTemplateEditorPage from "./ui/pages/characters/ChatTemplateEditorPage";
import { SearchPage } from "./ui/pages/search";
import { LibraryPage } from "./ui/pages/library/LibraryPage";
import { AvatarLibraryPickerPage } from "./ui/pages/library/ImageLibraryPage";
import { StandaloneLorebookEditor } from "./ui/pages/library/StandaloneLorebookEditor";
import { LorebookEntryGeneratorFlowPage } from "./ui/pages/LorebookEntryGeneratorFlowPage";
import { LorebookTriggerPreviewPage } from "./ui/pages/LorebookTriggerPreviewPage";
import { SyncPage } from "./ui/pages/sync/SyncPage";
import {
  DiscoveryPage,
  DiscoverySearchPage,
  DiscoveryCardDetailPage,
  DiscoveryBrowsePage,
} from "./ui/pages/discovery";
import {
  GroupChatsListPage,
  GroupChatCreatePage,
  GroupChatLayout,
  GroupChatPage,
  GroupSettingsPage,
  GroupChatSettingsPage,
  GroupChatHistoryPage,
  GroupChatMemoriesPage,
  GroupChatAppearancePage,
  GroupChatSearchPage,
} from "./ui/pages/group-chats";
import {
  EngineHomePage,
  EngineSetupWizard,
  EngineCharacterCreate,
  EngineChatPage,
  EngineProvidersConfigPage,
  EngineSettingsConfigPage,
} from "./ui/pages/engine";

import { CreateMenu, GuidedTour, useGuidedTour } from "./ui/components";
import { V1UpgradeToast } from "./ui/components/V1UpgradeToast";
import { V2UpgradeToast } from "./ui/components/V2UpgradeToast";
import { V3UpgradeToast } from "./ui/components/V3UpgradeToast";
import { ConfirmBottomMenuHost } from "./ui/components/ConfirmBottomMenu";
import {
  getLastSeenAppVersion,
  isOnboardingCompleted,
} from "./core/storage/appState";
import {
  WhatsNewDrawer,
  WHATS_NEW_OPEN_EVENT,
} from "./ui/pages/whats-new/WhatsNewPage";
import { TopNav, BottomNav, TitleBar, WindowResizeHandles } from "./ui/components/App";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen, UnlistenFn } from "@tauri-apps/api/event";
import { useAndroidBackHandler } from "./ui/hooks/useAndroidBackHandler";
import { logManager, isLoggingEnabled } from "./core/utils/logger";
import { getPlatform } from "./core/utils/platform";
import { I18nProvider, useI18n } from "./core/i18n/context";
import { hasSeenTooltip, setTooltipSeen } from "./core/storage/appState";
import { checkForAppUpdate } from "./core/app-updates/checkForAppUpdate";
import { detectUpdateChannel } from "./core/app-updates/checkForAppUpdate";
import { presentAppUpdateToast } from "./core/app-updates/presentAppUpdateToast";
import {
  readSettings,
  refreshSettingsFromStorage,
  SETTINGS_UPDATED_EVENT,
} from "./core/storage/repo";
import { recordChatDebugEvent } from "./core/debug/chatDebugStore";

const chatLog = logManager({ component: "Chat" });
const FIRST_RUN_TOUR_STORAGE_KEY = "app_tour_v1";

function getPayloadObject(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object") return null;
  return value as Record<string, unknown>;
}

function getString(value: unknown): string | undefined {
  return typeof value === "string" && value.trim().length > 0 ? value : undefined;
}

function getBoolean(value: unknown): boolean | undefined {
  return typeof value === "boolean" ? value : undefined;
}

function getNumber(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function summarizeChatDebugEvent(
  state: string,
  payload: unknown,
  level?: string,
): { level: "info" | "warn" | "error"; message: string } | null {
  const obj = getPayloadObject(payload);
  if (!obj) return null;

  const operation = getString(obj.operation);
  const providerId = getString(obj.providerId);
  const model = getString(obj.model);
  const requestId = getString(obj.requestId);
  const status = getNumber(obj.status);
  const elapsedMs = getNumber(obj.elapsedMs);
  const stream = getBoolean(obj.stream);
  const fallbackAttempt = getBoolean(obj.fallbackAttempt);
  const message = getString(obj.message);
  const hasReasoning = getBoolean(obj.hasReasoning);
  const length = getNumber(obj.length);

  switch (state) {
    case "continue_start":
      return {
        level: "info",
        message: `session=${getString(obj.sessionId) ?? "unknown"} character=${getString(obj.characterId) ?? "unknown"} messages=${getNumber(obj.messageCount) ?? 0}`,
      };
    case "continue_model_selected":
      return {
        level: "info",
        message: `provider=${providerId ?? "unknown"} model=${model ?? "unknown"} credential=${getString(obj.credentialId) ?? "unknown"}`,
      };
    case "system_prompt_built": {
      const debug = getPayloadObject(obj.system_prompt_debug);
      return {
        level: "info",
        message: `session=${getString(debug?.session_id) ?? "unknown"} base_source=${getString(debug?.base_template_source) ?? "unknown"} entries=${getNumber(debug?.entry_count) ?? 0} total_chars=${getNumber(debug?.total_chars) ?? 0}`,
      };
    }
    case "sending_request":
    case "continue_request":
    case "regenerate_request":
      return {
        level: "info",
        message:
          `operation=${operation ?? state} provider=${providerId ?? "unknown"} model=${model ?? "unknown"}` +
          ` stream=${stream ?? false} request_id=${requestId ?? "missing"}` +
          (fallbackAttempt ? " fallback_attempt=true" : ""),
      };
    case "response":
    case "continue_response":
    case "regenerate_response":
      return {
        level: "info",
        message:
          `operation=${operation ?? state} model=${model ?? "unknown"} status=${status ?? "unknown"}` +
          (elapsedMs != null ? ` elapsed_ms=${elapsedMs}` : "") +
          (requestId ? ` request_id=${requestId}` : ""),
      };
    case "provider_error":
    case "continue_provider_error":
    case "regenerate_provider_error":
      return {
        level: "error",
        message:
          `operation=${operation ?? state} model=${model ?? "unknown"} status=${status ?? "unknown"}` +
          (requestId ? ` request_id=${requestId}` : "") +
          (message ? ` message=${message}` : ""),
      };
    case "assistant_reply":
    case "continue_assistant_reply":
      return {
        level: "info",
        message:
          `operation=${operation ?? state} reply_length=${length ?? 0}` +
          (requestId ? ` request_id=${requestId}` : ""),
      };
    case "continue_empty_response":
    case "regenerate_empty_response":
      return {
        level: "warn",
        message:
          `operation=${operation ?? state} empty_response=true has_reasoning=${hasReasoning ?? false}` +
          (requestId ? ` request_id=${requestId}` : ""),
      };
    case "transport_retry":
      return {
        level: "warn",
        message:
          `scope=${getString(obj.scope) ?? "unknown"} attempt=${getNumber(obj.attempt) ?? 0}/${getNumber(obj.maxRetries) ?? 0}` +
          ` reason=${getString(obj.reason) ?? "unknown"}` +
          (status != null ? ` status=${status}` : "") +
          (getNumber(obj.delayMs) != null ? ` delay_ms=${getNumber(obj.delayMs)}` : "") +
          (requestId ? ` request_id=${requestId}` : ""),
      };
    default:
      if (level?.toUpperCase() === "ERROR" && message) {
        return { level: "error", message };
      }
      if (level?.toUpperCase() === "WARN" && message) {
        return { level: "warn", message };
      }
      return null;
  }
}

type LlamaModelLoadProgressEvent = {
  requestId?: string | null;
  modelPath?: string | null;
  modelName?: string | null;
  stage?: number | null;
  status?: number | null;
  progress?: number | null;
};

const LLAMA_MODEL_LOAD_STATUS_LOADING = 0;
const LLAMA_MODEL_LOAD_STATUS_RETRYING = 1;
const LLAMA_MODEL_LOAD_STATUS_LOADED = 2;
const LLAMA_MODEL_LOAD_STATUS_FAILED = 3;
const PERSONA_LIBRARY_ROUTE = "/library?view=personas";

function shouldKeepLlamaLoaded(pathname: string) {
  if (pathname.startsWith("/chat/")) {
    return true;
  }

  if (pathname.startsWith("/engine-chat/")) {
    return true;
  }

  if (pathname.startsWith("/group-chats/groups/")) {
    return false;
  }

  if (
    pathname === "/group-chats" ||
    pathname === "/group-chats/history" ||
    pathname === "/group-chats/new"
  ) {
    return false;
  }

  return /^\/group-chats\/[^/]+(?:\/.*)?$/.test(pathname);
}

function resolveLlamaModelLoadCopy(stage?: number | null) {
  switch (stage) {
    case 0:
      return {
        title: "Local model startup",
        subtitle: "Preparing GPU offload",
      };
    case 1:
      return {
        title: "Local model startup",
        subtitle: "Preparing CPU runtime",
      };
    case 2:
      return {
        title: "Local model startup",
        subtitle: "Switching to CPU fallback",
      };
    case 3:
      return {
        title: "Local model startup",
        subtitle: "Finalizing runtime",
      };
    default:
      return {
        title: "Local model startup",
        subtitle: "Preparing model runtime",
      };
  }
}

function LegacyPersonaEditRedirect() {
  const { personaId } = useParams();

  return (
    <Navigate to={personaId ? `/personas/${personaId}/edit` : PERSONA_LIBRARY_ROUTE} replace />
  );
}

function App() {
  const platform = useMemo(() => getPlatform(), []);
  const [whatsNewOpen, setWhatsNewOpen] = useState(false);

  useEffect(() => {
    const onOpen = () => setWhatsNewOpen(true);
    window.addEventListener(WHATS_NEW_OPEN_EVENT, onOpen);
    return () => window.removeEventListener(WHATS_NEW_OPEN_EVENT, onOpen);
  }, []);

  useEffect(() => {
    if (typeof document === "undefined" || platform.os !== "linux") return;

    const styleId = "linux-color-scheme-dark";
    let style = document.getElementById(styleId) as HTMLStyleElement | null;

    if (!style) {
      style = document.createElement("style");
      style.id = styleId;
      style.textContent = ":root { color-scheme: dark; }";
      document.head.appendChild(style);
    }

    return () => {
      style?.remove();
    };
  }, [platform.os]);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    (async () => {
      try {
        unlisten = await listen("chat://debug", (event) => {
          if (
            typeof event.payload === "object" &&
            event.payload !== null &&
            "state" in event.payload
          ) {
            const { state, level, payload, message } = event.payload as {
              state: string;
              level?: string;
              payload?: unknown;
              message?: string;
            };

            // Backend logs come pre-formatted with timestamp
            if (message !== undefined) {
              if (isLoggingEnabled()) {
                const method = level?.toLowerCase() || "log";
                if (method in console) {
                  (console as any)[method](message);
                } else {
                  console.log(message);
                }
              }
            } else if (payload !== undefined) {
              recordChatDebugEvent({ state, payload, level });
              const summary = summarizeChatDebugEvent(state, payload, level);
              if (summary) {
                chatLog.with({ fn: state })[summary.level](summary.message);
              }
            }
          } else {
            chatLog.warn("unknown event payload", event.payload);
          }
        });
      } catch (err) {
        console.error("Failed to attach debug listener:", err);
      }
    })();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    (async () => {
      try {
        unlisten = await listen("app://gpu-fallback-prompt", () => {
          toast.warning(
            "GPU memory insufficient",
            "This model doesn't fit in GPU memory. Switch to CPU (slower) or abort?",
            {
              actionLabel: "Switch to CPU",
              onAction: () => emit("app://gpu-fallback-response", "switch"),
              secondaryLabel: "Abort",
              onSecondary: () => emit("app://gpu-fallback-response", "abort"),
              id: "gpu-fallback",
              duration: Infinity,
            },
          );
        });
      } catch (err) {
        console.error("Failed to attach gpu-fallback listener:", err);
      }
    })();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    (async () => {
      try {
        unlisten = await listen("app://toast", (event) => {
          const payload = event.payload as Record<string, unknown> | null;
          if (!payload || typeof payload !== "object") {
            return;
          }
          const variant = payload.variant;
          const title = payload.title;
          const description = payload.description;
          const id = payload.id;
          const dismiss = payload.dismiss;
          const kind = payload.kind;
          const subtitle = payload.subtitle;
          const modelName = payload.modelName;
          const progress = payload.progress;
          if (dismiss === true && (typeof id === "string" || typeof id === "number")) {
            toast.dismiss(id);
            return;
          }
          if (
            kind === "modelLoad"
            && typeof title === "string"
            && typeof subtitle === "string"
            && typeof modelName === "string"
            && typeof progress === "number"
          ) {
            toast.modelLoad({
              id: typeof id === "string" || typeof id === "number" ? id : undefined,
              title,
              subtitle,
              modelName,
              progress,
              duration: Infinity,
            });
            return;
          }
          if (typeof title !== "string") {
            return;
          }
          const detail = typeof description === "string" ? description : undefined;
          const toastOptions =
            typeof id === "string" || typeof id === "number" ? { id } : undefined;
          switch (variant) {
            case "success":
              toast.success(title, detail, toastOptions);
              break;
            case "warning":
              toast.warning(title, detail, toastOptions);
              break;
            case "error":
              toast.error(title, detail, toastOptions);
              break;
            default:
              toast.info(title, detail, toastOptions);
          }
        });
      } catch (err) {
        console.error("Failed to attach toast listener:", err);
      }
    })();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    (async () => {
      try {
        unlisten = await listen<LlamaModelLoadProgressEvent>(
          "llama-model-load-progress",
          (event) => {
            const payload = event.payload;
            if (!payload || typeof payload !== "object") {
              return;
            }

            const requestId =
              typeof payload.requestId === "string" && payload.requestId.trim()
                ? payload.requestId
                : null;
            const modelPath =
              typeof payload.modelPath === "string" && payload.modelPath.trim()
                ? payload.modelPath
                : null;
            const requestToastId = requestId;
            const pathToastId = modelPath ? `llama-model-load:${modelPath}` : null;
            const toastId = pathToastId ?? requestToastId;
            if (!toastId) {
              return;
            }

            const status =
              typeof payload.status === "number" && Number.isFinite(payload.status)
                ? payload.status
                : LLAMA_MODEL_LOAD_STATUS_LOADING;
            if (
              status === LLAMA_MODEL_LOAD_STATUS_LOADED ||
              status === LLAMA_MODEL_LOAD_STATUS_FAILED
            ) {
              toast.dismiss(toastId);
              if (requestToastId && requestToastId !== toastId) {
                toast.dismiss(requestToastId);
              }
              if (pathToastId && pathToastId !== toastId) {
                toast.dismiss(pathToastId);
              }
              return;
            }

            const modelName =
              typeof payload.modelName === "string" && payload.modelName.trim()
                ? payload.modelName
                : "Local model";
            const progress =
              typeof payload.progress === "number" && Number.isFinite(payload.progress)
                ? payload.progress
                : 0;
            const stage =
              typeof payload.stage === "number" && Number.isFinite(payload.stage)
                ? payload.stage
                : undefined;
            const copy = resolveLlamaModelLoadCopy(stage);

            toast.modelLoad({
              id: toastId,
              title: copy.title,
              subtitle:
                status === LLAMA_MODEL_LOAD_STATUS_RETRYING
                  ? "Switching to CPU fallback"
                  : copy.subtitle,
              modelName,
              progress,
            });
          },
        );
      } catch (err) {
        console.error("Failed to attach llama model load progress listener:", err);
      }
    })();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  return (
    <I18nProvider>
      <ThemeProvider>
        <BrowserRouter>
          <div id="app-root" className="min-h-screen bg-surface text-fg antialiased">
            <Toaster
              position={"top-center"}
              expand={true}
              offset={{ top: "calc(var(--titlebar-h, 0px) + 16px)" }}
              mobileOffset={{
                top: "calc(env(safe-area-inset-top) + 80px)",
                left: 8,
                right: 8,
              }}
              toastOptions={{
                unstyled: true,
                className: "pointer-events-auto w-full max-w-md",
                descriptionClassName: "text-xs text-fg/70",
              }}
            />
            <ConfirmBottomMenuHost />
            <WhatsNewDrawer
              isOpen={whatsNewOpen}
              onClose={() => setWhatsNewOpen(false)}
            />
            <DownloadQueueProvider>
              <AppUpdateNotifier />
              <AppContent />
            </DownloadQueueProvider>
          </div>
        </BrowserRouter>
      </ThemeProvider>
    </I18nProvider>
  );
}

function AppUpdateNotifier() {
  const { t } = useI18n();
  const platform = useMemo(() => getPlatform(), []);
  const [autoChecksEnabled, setAutoChecksEnabled] = useState(true);
  const [settingsReady, setSettingsReady] = useState(false);
  const showUpdateToast = useCallback(
    (update: {
      currentVersion: string;
      latestVersion: string;
      releaseUrl: string;
      downloadUrl: string;
      releaseTag: string;
      channel: "dev" | "release";
    }) => {
      presentAppUpdateToast(update, platform.os, {
        title: t("updates.available.title"),
        description: t("updates.available.description", {
          currentVersion: update.currentVersion,
          latestVersion: update.latestVersion,
        }),
        viewLabel: t("updates.available.actions.view"),
        laterLabel: t("common.buttons.later"),
      });
    },
    [platform.os, t],
  );

  useEffect(() => {
    let cancelled = false;

    const syncSettings = async () => {
      try {
        const settings = await readSettings();
        if (cancelled) return;
        setAutoChecksEnabled(settings.advancedSettings?.appUpdateChecksEnabled ?? true);
      } catch {
        if (!cancelled) {
          setAutoChecksEnabled(true);
        }
      } finally {
        if (!cancelled) {
          setSettingsReady(true);
        }
      }
    };

    const handleSettingsUpdated = () => {
      void syncSettings();
    };

    void syncSettings();
    window.addEventListener(SETTINGS_UPDATED_EVENT, handleSettingsUpdated);

    return () => {
      cancelled = true;
      window.removeEventListener(SETTINGS_UPDATED_EVENT, handleSettingsUpdated);
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlisten: UnlistenFn | null = null;

    const reloadSettings = async () => {
      try {
        await refreshSettingsFromStorage();
      } catch (error) {
        if (!cancelled) {
          console.error("Failed to refresh settings after database reload:", error);
        }
      }
    };

    void listen("database-reloaded", () => {
      if (cancelled) return;
      void reloadSettings();
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch((error) => {
        if (!cancelled) {
          console.error("Failed to attach database reload listener:", error);
        }
      });

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  useEffect(() => {
    const handleForceUpdateNotification = async (event: Event) => {
      const detail = (
        event as CustomEvent<
          | {
              currentVersion?: string;
              latestVersion?: string;
              releaseUrl?: string;
              downloadUrl?: string;
              releaseTag?: string;
              channel?: "dev" | "release";
            }
          | undefined
        >
      ).detail;

      const currentVersion =
        detail?.currentVersion ?? (await invoke<string>("get_app_version").catch(() => "1.0.0"));
      const channel = detail?.channel ?? detectUpdateChannel(currentVersion);
      const latestVersion = detail?.latestVersion ?? "999.0.0";
      const releaseUrl = detail?.releaseUrl ?? "https://github.com/LettuceAI/app/releases";
      const downloadUrl =
        detail?.downloadUrl ??
        `https://www.lettuceai.app/download?platform=${encodeURIComponent(platform.os)}&source=in-app-update-test`;
      const releaseTag =
        detail?.releaseTag ??
        `forced-update-${platform.os}-${latestVersion.replace(/[^\w.-]+/g, "-")}`;

      await setTooltipSeen(`app-update-dismissed:${platform.os}:${releaseTag}`, false);
      showUpdateToast({
        currentVersion,
        latestVersion,
        releaseUrl,
        downloadUrl,
        releaseTag,
        channel,
      });
    };

    window.addEventListener("lettuce:update-check:force", handleForceUpdateNotification);
    return () => {
      window.removeEventListener("lettuce:update-check:force", handleForceUpdateNotification);
    };
  }, [platform.os, showUpdateToast]);

  useEffect(() => {
    if (import.meta.env.DEV || !settingsReady || !autoChecksEnabled) return;

    let cancelled = false;

    const run = async () => {
      try {
        const update = await checkForAppUpdate(platform);
        if (!update || cancelled) return;

        const dismissKey = `app-update-dismissed:${platform.os}:${update.releaseTag}`;
        if (await hasSeenTooltip(dismissKey)) return;
        if (cancelled) return;

        showUpdateToast(update);
      } catch {}
    };

    void run();

    return () => {
      cancelled = true;
    };
  }, [autoChecksEnabled, platform, settingsReady, showUpdateToast]);

  return null;
}

function AppContent() {
  const location = useLocation();
  const navigate = useNavigate();
  const { t } = useI18n();
  console.log("AppContent render:", location.pathname, location.key);
  const mainRef = useRef<HTMLDivElement | null>(null);
  const previousLlamaKeepAliveRouteRef = useRef(shouldKeepLlamaLoaded(location.pathname));
  const platform = useMemo(() => getPlatform(), []);
  const isChatRoute = location.pathname === "/chat" || location.pathname === "/";
  // Group chat detail: /group-chats/:id, /group-chats/:id/settings, /group-chats/new (NOT /group-chats list)
  const isGroupChatDetailRoute = location.pathname.startsWith("/group-chats/");
  const isEngineChatRoute = location.pathname.startsWith("/engine-chat/");
  const isChatDetailRoute =
    location.pathname.startsWith("/chat/") || isGroupChatDetailRoute || isEngineChatRoute;
  const isSearchRoute = location.pathname === "/search";
  const isAvatarLibraryPickerRoute = location.pathname === "/library/images/pick";
  const isOnboardingRoute = useMemo(
    () =>
      location.pathname.startsWith("/welcome") ||
      location.pathname.startsWith("/onboarding") ||
      location.pathname.startsWith("/wheretofind"),
    [location.pathname],
  );
  const isDiscoveryRoute = useMemo(
    () => location.pathname.startsWith("/discover"),
    [location.pathname],
  );
  const isDiscoverySubRoute = useMemo(
    () => location.pathname.startsWith("/discover/"),
    [location.pathname],
  );
  const isCreateRoute = useMemo(
    () => location.pathname.startsWith("/create/"),
    [location.pathname],
  );
  const isPersonaEditRoute = useMemo(
    () => /^\/personas\/[^/]+\/edit$/.test(location.pathname),
    [location.pathname],
  );
  const isCharacterEditRoute = useMemo(
    () => /^\/settings\/characters\/[^/]+\/edit$/.test(location.pathname),
    [location.pathname],
  );

  const isSettingRoute = useMemo(
    () => location.pathname.startsWith("/settings"),
    [location.pathname],
  );

  const preSettingsPathRef = useRef<string>("/");
  useEffect(() => {
    if (!isSettingRoute) {
      preSettingsPathRef.current = location.pathname + location.search;
    }
  }, [isSettingRoute, location.pathname, location.search]);

  const [isLgViewport, setIsLgViewport] = useState(() =>
    typeof window === "undefined" ? false : window.matchMedia("(min-width: 1024px)").matches,
  );
  useEffect(() => {
    const mq = window.matchMedia("(min-width: 1024px)");
    const handler = (e: MediaQueryListEvent) => setIsLgViewport(e.matches);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, []);

  const isLogsRoute = location.pathname === "/settings/logs";

  const isLorebookEditorRoute = useMemo(
    () =>
      location.pathname.startsWith("/library/lorebooks/") ||
      /^\/settings\/characters\/[^/]+\/lorebook(\/preview|\/generate)?$/.test(location.pathname) ||
      /^\/group-chats\/groups\/[^/]+\/lorebook(\/preview|\/generate)?$/.test(location.pathname) ||
      /^\/group-chats\/[^/]+\/lorebook(\/preview|\/generate)?$/.test(location.pathname),
    [location.pathname],
  );
  const isLorebookGeneratorRoute = location.pathname === "/library/lorebook/generate";
  const isTemplateEditorRoute = useMemo(
    () => /^\/settings\/characters\/[^/]+\/templates\/[^/]+$/.test(location.pathname),
    [location.pathname],
  );

  const showTopNav =
    !isOnboardingRoute &&
    !isChatDetailRoute &&
    !isCreateRoute &&
    !isSearchRoute &&
    !isLorebookEditorRoute;
  const showBottomNav =
    !isSettingRoute &&
    !isOnboardingRoute &&
    !isChatDetailRoute &&
    !isCreateRoute &&
    !isPersonaEditRoute &&
    !isSearchRoute &&
    !isAvatarLibraryPickerRoute &&
    !isLorebookEditorRoute &&
    !isLorebookGeneratorRoute &&
    !isDiscoverySubRoute;

  const [showCreateMenu, setShowCreateMenu] = useState(false);
  const { shouldShow: showGuidedTour, dismiss: dismissGuidedTour } =
    useGuidedTour("appShell");

  useEffect(() => {
    const globalWindow = window as Window & {
      __debug?: Record<string, unknown> & {
        resetFirstRunTour?: () => Promise<void>;
      };
    };

    const resetFirstRunTour = async () => {
      await setTooltipSeen(FIRST_RUN_TOUR_STORAGE_KEY, false);
      console.info('[debug] First-run tour reset. Open "/chat" to trigger it again.');
    };

    globalWindow.__debug = {
      ...(globalWindow.__debug ?? {}),
      resetFirstRunTour,
    };

    return () => {
      if (globalWindow.__debug?.resetFirstRunTour !== resetFirstRunTour) {
        return;
      }

      const { resetFirstRunTour: _resetFirstRunTour, ...rest } = globalWindow.__debug;
      globalWindow.__debug = Object.keys(rest).length > 0 ? rest : undefined;
    };
  }, []);

  const handleAndroidBack = useCallback(() => {
    const globalWindow = window as any;
    if (globalWindow.__unsavedChanges) {
      toast.warningSticky(
        "Unsaved changes",
        "Save or discard your changes before leaving.",
        "Discard",
        () => {
          window.dispatchEvent(new CustomEvent("unsaved:discard"));
        },
        "unsaved-changes",
        {
          label: "Save",
          onAction: () => {
            window.dispatchEvent(new CustomEvent("unsaved:save"));
          },
        },
      );
      return false;
    }
    return true;
  }, []);

  useAndroidBackHandler({ canLeave: handleAndroidBack });

  useEffect(() => {
    if (isOnboardingRoute || isCreateRoute) {
      setShowCreateMenu(false);
    }
  }, [isOnboardingRoute, isCreateRoute]);

  useEffect(() => {
    if (platform.os !== "android") return;
    invoke("android_monitor_set_route", {
      route: location.pathname + location.search,
    }).catch(() => {
      // Ignore monitor update failures; Android monitor is best-effort metadata.
    });
  }, [location.pathname, location.search, platform.os]);

  useEffect(() => {
    const previousShouldKeepLoaded = previousLlamaKeepAliveRouteRef.current;
    const nextShouldKeepLoaded = shouldKeepLlamaLoaded(location.pathname);
    previousLlamaKeepAliveRouteRef.current = nextShouldKeepLoaded;

    if (!previousShouldKeepLoaded || nextShouldKeepLoaded) {
      return;
    }

    invoke("llamacpp_unload").catch((error) => {
      console.error("Failed to unload llama.cpp after leaving chat routes:", error);
    });
  }, [location.pathname]);

  useEffect(() => {
    const urlParams = new URLSearchParams(location.search);
    if (urlParams.get("firstTime") === "true" && isChatRoute) {
      window.history.replaceState({}, document.title, location.pathname);
    }
  }, [location.search, location.pathname, isChatRoute]);

  useEffect(() => {
    if (!location.pathname.startsWith("/settings")) return;

    const id = window.setTimeout(() => {
      const main = mainRef.current;
      if (main) {
        main.scrollTop = 0;

        const inner = main.querySelector(
          "[data-settings-scroll], .settings-scroll",
        ) as HTMLElement | null;
        if (inner) {
          inner.scrollTop = 0;
        }
      }

      window.scrollTo(0, 0);
    }, 0);

    return () => window.clearTimeout(id);
  }, [location.pathname]);

  const isDesktop = useMemo(() => {
    const platform = getPlatform();
    return platform.type === "desktop";
  }, []);

  return (
    <div className="relative min-h-screen overflow-hidden pt-[var(--titlebar-h,0px)]">
      <TitleBar />
      <WindowResizeHandles />
      <div
        className={`relative z-10 mx-auto flex w-full ${
          isChatDetailRoute
            ? "max-w-full h-[calc(100dvh-var(--titlebar-h,0px))]"
            : isSettingRoute
              ? "max-w-md min-h-[calc(100dvh-var(--titlebar-h,0px))] lg:max-w-none lg:h-[calc(100dvh-var(--titlebar-h,0px))] lg:min-h-0"
              : "max-w-md lg:max-w-none min-h-[calc(100dvh-var(--titlebar-h,0px))]"
        } flex-col ${showBottomNav ? "pb-[calc(72px+env(safe-area-inset-bottom))]" : "pb-0"}`}
      >
        {showTopNav && (
          <TopNav
            currentPath={location.pathname + location.search}
            onBackOverride={
              isPersonaEditRoute
                ? () => navigate(PERSONA_LIBRARY_ROUTE, { replace: true })
                : isCharacterEditRoute
                  ? () => navigate("/", { replace: true })
                  : isSettingRoute &&
                      isLgViewport &&
                      (location.pathname === "/settings" ||
                        location.pathname === "/settings/about")
                    ? () => {
                        const target = preSettingsPathRef.current || "/";
                        navigate(target.startsWith("/settings") ? "/" : target);
                      }
                    : undefined
            }
            titleOverride={
              isAvatarLibraryPickerRoute
                ? t("common.nav.library")
                : isLorebookGeneratorRoute
                  ? "Generate Lorebook"
                  : location.pathname === "/settings/models/installed"
                    ? t("installedModels.title")
                    : /^\/settings\/voices\/kokoro\/[^/]+\/blend$/.test(location.pathname)
                      ? t("voices.extra.kokoro.newBlend")
                      : /^\/settings\/voices\/kokoro\/[^/]+\/blend\/.+$/.test(location.pathname)
                        ? t("voices.extra.kokoro.editBlend")
                        : /^\/settings\/voices\/kokoro\/[^/]+$/.test(location.pathname)
                          ? t("voices.extra.kokoro.title")
                          : undefined
            }
          />
        )}

        <main
          ref={mainRef}
          className={`flex-1 ${showTopNav ? "pt-[var(--topnav-h,72px)]" : ""} ${
            location.pathname === "/welcome"
              ? "overflow-hidden px-0 pt-0 pb-0"
              : isOnboardingRoute
              ? `overflow-y-auto ${isDesktop ? "" : "px-0 pt-5 pb-5"}`
              : isChatDetailRoute
                ? "overflow-hidden px-0 pt-0 pb-0"
                : isCreateRoute
                  ? "overflow-hidden px-0 pt-0 pb-0"
                  : isSearchRoute
                    ? "overflow-hidden px-0 pt-0 pb-0"
                    : isLogsRoute
                      ? "overflow-hidden px-0 pt-0 pb-0"
                      : isLorebookEditorRoute
                        ? "overflow-hidden px-0 pt-0 pb-0"
                        : isPersonaEditRoute
                        ? "overflow-hidden px-0 pt-0 pb-0"
                        : isTemplateEditorRoute
                          ? "overflow-hidden px-0 pt-0 pb-0"
                          : isDiscoveryRoute
                            ? "overflow-hidden px-0 pt-0 pb-0"
                            : isSettingRoute
                              ? "overflow-y-auto px-4 pt-4 pb-6 lg:overflow-hidden lg:p-0 lg:mt-(--topnav-h,72px)"
                              : `overflow-y-auto px-4 pt-4 ${showBottomNav ? "pb-[calc(96px+env(safe-area-inset-bottom))]" : "pb-6"}`
          }`}
        >
          <div
            key={(() => {
              if (location.pathname.startsWith("/settings")) return "/settings";
              if (location.pathname.startsWith("/library")) return location.pathname;
              if (location.pathname === "/chat") return "/chat";
              const chatMatch = location.pathname.match(/^\/chat\/([^/]+)/);
              if (chatMatch) return `/chat/${chatMatch[1]}`;
              const groupMatch = location.pathname.match(/^\/group-chats\/([^/]+)/);
              if (groupMatch) return `/group-chats/${groupMatch[1]}`;
              return location.key;
            })()}
            className={
              location.pathname.startsWith("/settings")
                ? "h-full app-text-scope settings-theme-scope"
                : "h-full app-text-scope"
            }
          >
            <Routes>
              <Route path="/" element={<OnboardingCheck />} />
              <Route path="/welcome" element={<OnboardingPage />} />
              <Route path="/onboarding/start" element={<OnboardingPage />} />
              <Route path="/onboarding/learn" element={<OnboardingPage />} />
              <Route path="/onboarding/path" element={<OnboardingPage />} />
              <Route path="/onboarding/gemini" element={<OnboardingPage />} />
              <Route path="/onboarding/openrouter" element={<OnboardingPage />} />
              <Route path="/onboarding/finish" element={<OnboardingPage />} />
              <Route path="/onboarding/provider" element={<OnboardingPage />} />
              <Route path="/onboarding/models" element={<OnboardingPage />} />
              <Route path="/onboarding/memory" element={<OnboardingPage />} />
              <Route path="/onboarding/sync" element={<OnboardingPage />} />
              <Route path="/wheretofind" element={<WhereToFindPage />} />
              <Route path="/search" element={<SearchPage />} />
              <Route path="/discover" element={<DiscoveryPage />} />
              <Route path="/discover/search" element={<DiscoverySearchPage />} />
              <Route path="/discover/browse" element={<DiscoveryBrowsePage />} />
              <Route path="/discover/card/:path" element={<DiscoveryCardDetailPage />} />
              <Route path="/library" element={<LibraryPage />} />
              <Route path="/library/images/pick" element={<AvatarLibraryPickerPage />} />
              <Route
                path="/library/images"
                element={<Navigate to="/library?view=images" replace />}
              />
              <Route path="/library/lorebooks/:lorebookId" element={<StandaloneLorebookEditor />} />
              <Route
                path="/library/lorebooks/:lorebookId/generate"
                element={<LorebookEntryGeneratorFlowPage />}
              />
              <Route
                path="/library/lorebooks/:lorebookId/preview"
                element={<LorebookTriggerPreviewPage />}
              />
              <Route
                path="/library/lorebook/generate"
                element={<LorebookGeneratorFlowPage />}
              />
              <Route element={<SettingsLayout />}>
              <Route path="/settings" element={<SettingsPage />} />
              <Route path="/settings/providers" element={<ProvidersPage />} />
              <Route path="/settings/models" element={<ModelsPage />} />
              <Route path="/settings/models/new" element={<EditModelPage />} />
              <Route path="/settings/models/browse" element={<HuggingFaceBrowserPage />} />
              <Route path="/settings/models/installed" element={<InstalledModelsPage />} />
              <Route
                path="/settings/models/runtime-defaults"
                element={<LocalRuntimeDefaultsPage />}
              />
              <Route path="/settings/models/:modelId" element={<EditModelPage />} />
              <Route path="/settings/voices" element={<VoicesPage />} />
              <Route
                path="/settings/voices/kokoro/:providerId"
                element={<KokoroStudioPage />}
              />
              <Route
                path="/settings/voices/kokoro/:providerId/blend"
                element={<KokoroBlendEditorPage />}
              />
              <Route
                path="/settings/voices/kokoro/:providerId/blend/:blendId"
                element={<KokoroBlendEditorPage />}
              />
              <Route path="/settings/image-generation" element={<ImageGenerationPage />} />
              <Route path="/settings/prompts" element={<SystemPromptsPage />} />
              <Route path="/settings/prompts/new" element={<EditPromptTemplate />} />
              <Route path="/settings/prompts/:id" element={<EditPromptTemplate />} />
              <Route path="/settings/security" element={<SecurityPage />} />
              <Route path="/settings/usage" element={<UsagePage />} />
              <Route path="/settings/usage/activity" element={<UsageActivityPage />} />
              <Route path="/settings/performance" element={<PerformancePage />} />
              <Route path="/settings/customization" element={<CustomizationPage />} />
              <Route path="/settings/speech-recognition" element={<SpeechRecognitionPage />} />
              <Route path="/settings/customization/colors" element={<ColorCustomizationPage />} />
              <Route path="/settings/customization/chat" element={<ChatAppearancePage />} />
              <Route path="/settings/logs" element={<LogsPage />} />
              <Route path="/settings/about" element={<AboutPage />} />
              <Route path="/settings/advanced" element={<AdvancedPage />} />
              <Route path="/settings/advanced/memory" element={<DynamicMemoryPage />} />
              <Route path="/settings/advanced/companions" element={<CompanionsHubPage />} />
              <Route path="/settings/advanced/creation-helper" element={<AICreationHelperPage />} />
              <Route path="/settings/advanced/help-me-reply" element={<HelpMeReplyPage />} />
              <Route path="/settings/advanced/lorebooks" element={<LorebooksPage />} />
              <Route
                path="/settings/advanced/companion-soul-writer"
                element={<CompanionsHubPage />}
              />
              <Route path="/settings/advanced/host-api" element={<HostApiPage />} />
              <Route path="/settings/embedding-download" element={<EmbeddingDownloadPage />} />
              <Route path="/settings/companion-download" element={<CompanionDownloadPage />} />
              <Route
                path="/settings/companion-download-queue"
                element={<CompanionDownloadQueuePage />}
              />
              <Route path="/settings/embedding-test" element={<EmbeddingTestPage />} />
              <Route path="/settings/developer/kokoro-test" element={<KokoroTestPage />} />
              <Route path="/settings/changelog" element={<ChangelogPage />} />
              <Route path="/settings/help" element={<HelpPage />} />
              <Route path="/settings/developer" element={<DeveloperPage />} />
              <Route path="/settings/reset" element={<ResetPage />} />
              <Route path="/settings/backup" element={<BackupRestorePage />} />
              <Route path="/settings/sync" element={<SyncPage />} />
              <Route path="/settings/engine/:credentialId" element={<EngineHomePage />} />
              <Route path="/settings/engine/:credentialId/setup" element={<EngineSetupWizard />} />
              <Route
                path="/settings/engine/:credentialId/providers"
                element={<EngineProvidersConfigPage />}
              />
              <Route
                path="/settings/engine/:credentialId/settings"
                element={<EngineSettingsConfigPage />}
              />
              <Route
                path="/settings/engine/:credentialId/character/new"
                element={<EngineCharacterCreate />}
              />
              </Route>
              <Route path="/engine-chat/:credentialId/:slug" element={<EngineChatPage />} />
              <Route path="/chat" element={<ChatPage />} />
              <Route path="/chat/:characterId" element={<ChatLayout />}>
                <Route index element={<ChatConversationPage />} />
                <Route path="settings" element={<ChatSettingsPage />} />
                <Route path="companion/soul" element={<CompanionSoulPage />} />
                <Route path="memories" element={<ChatMemoriesPage />} />
              </Route>
              <Route path="/chat/:characterId/search" element={<SearchMessagesPage />} />
              <Route path="/chat/:characterId/history" element={<ChatHistoryPage />} />
              <Route path="/chat/:characterId/tree" element={<ChatTreePage />} />
              <Route
                path="/chat/:characterId/companion/memories"
                element={<CompanionMemoryPage />}
              />
              <Route
                path="/chat/:characterId/companion/relationship"
                element={<CompanionRelationshipPage />}
              />
              <Route
                path="/chat/:characterId/debug/:sessionId/:messageId"
                element={<MessageDebugPage />}
              />
              <Route path="/create/character" element={<CreateCharacterPage />} />
              <Route path="/create/character/helper" element={<CreationHelperPage />} />
              <Route path="/settings/characters" element={<CharactersPage />} />
              <Route
                path="/settings/characters/:characterId/edit"
                element={<EditCharacterPage />}
              />
              <Route
                path="/settings/characters/:characterId/lorebook"
                element={<LorebookEditor />}
              />
              <Route
                path="/settings/characters/:characterId/lorebook/generate"
                element={<LorebookEntryGeneratorFlowPage />}
              />
              <Route
                path="/settings/characters/:characterId/lorebook/preview"
                element={<LorebookTriggerPreviewPage />}
              />
              <Route path="/group-chats/groups/:groupId/lorebook" element={<LorebookEditor />} />
              <Route
                path="/group-chats/groups/:groupId/lorebook/preview"
                element={<LorebookTriggerPreviewPage />}
              />
              <Route
                path="/settings/characters/:characterId/templates"
                element={<ChatTemplateListPage />}
              />
              <Route
                path="/settings/characters/:characterId/templates/:templateId"
                element={<ChatTemplateEditorPage />}
              />
              <Route path="/create/persona" element={<CreatePersonaPage />} />
              <Route path="/personas" element={<Navigate to={PERSONA_LIBRARY_ROUTE} replace />} />
              <Route path="/personas/:personaId/edit" element={<EditPersonaPage />} />
              <Route
                path="/settings/personas"
                element={<Navigate to={PERSONA_LIBRARY_ROUTE} replace />}
              />
              <Route
                path="/settings/personas/:personaId/edit"
                element={<LegacyPersonaEditRedirect />}
              />
              <Route path="/group-chats" element={<GroupChatsListPage />} />
              <Route path="/group-chats/history" element={<GroupChatHistoryPage />} />
              <Route path="/group-chats/new" element={<GroupChatCreatePage />} />
              <Route path="/group-chats/groups/:groupId/settings" element={<GroupSettingsPage />} />
              <Route path="/group-chats/:groupSessionId" element={<GroupChatLayout />}>
                <Route index element={<GroupChatPage />} />
                <Route path="settings" element={<GroupChatSettingsPage />} />
                <Route path="lorebook" element={<LorebookEditor />} />
                <Route path="lorebook/preview" element={<LorebookTriggerPreviewPage />} />
                <Route path="memories" element={<GroupChatMemoriesPage />} />
                <Route path="appearance" element={<GroupChatAppearancePage />} />
                <Route path="search" element={<GroupChatSearchPage />} />
              </Route>
            </Routes>
          </div>
        </main>

        {showBottomNav && <BottomNav onCreateClick={() => setShowCreateMenu(true)} />}
      </div>

      {showBottomNav && (
        <CreateMenu isOpen={showCreateMenu} onClose={() => setShowCreateMenu(false)} />
      )}

      {isChatRoute && showBottomNav && showGuidedTour && (
        <GuidedTour tour="appShell" onDismiss={dismissGuidedTour} />
      )}

      {/* V1 Embedding Model Upgrade Toast */}
      <V1UpgradeToast />
      {/* V2 Embedding Model Upgrade Toast */}
      <V2UpgradeToast />
      {/* V3 -> V4 Embedding Model Upgrade Toast (persistent dismissal) */}
      <V3UpgradeToast />
    </div>
  );
}

function OnboardingCheck() {
  const [isChecking, setIsChecking] = useState(true);
  const [shouldShowOnboarding, setShouldShowOnboarding] = useState(false);

  useEffect(() => {
    let cancelled = false;

    const check = async () => {
      const onboardingCompleted = await isOnboardingCompleted();
      if (cancelled) return;
      if (!onboardingCompleted) {
        setShouldShowOnboarding(true);
        setIsChecking(false);
        return;
      }

      try {
        const currentVersion = await invoke<string>("get_app_version");
        const lastSeen = await getLastSeenAppVersion();
        if (cancelled) return;
        if (lastSeen !== currentVersion) {
          window.dispatchEvent(new Event(WHATS_NEW_OPEN_EVENT));
        }
      } catch { }

      if (!cancelled) setIsChecking(false);
    };

    void check();

    return () => {
      cancelled = true;
    };
  }, []);

  if (isChecking) {
    return (
      <div className="flex h-full items-center justify-center rounded-3xl border border-fg/5 bg-fg/5 backdrop-blur-sm">
        <div className="h-10 w-10 animate-spin rounded-full border-4 border-fg/10 border-t-fg/60" />
      </div>
    );
  }

  if (shouldShowOnboarding) {
    return <Navigate to="/welcome" replace />;
  }

  return <ChatPage />;
}

export default App;
