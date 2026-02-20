import { BrowserRouter, Route, Routes, useLocation, Navigate } from "react-router-dom";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { motion } from "framer-motion";
import { Toaster } from "sonner";

import { WelcomePage, OnboardingPage } from "./ui/pages/onboarding";
import { WhereToFindPage } from "./ui/pages/onboarding/WhereToFind";
import { SettingsPage } from "./ui/pages/settings/Settings";
import { ProvidersPage } from "./ui/pages/settings/ProvidersPage";
import { ModelsPage } from "./ui/pages/settings/ModelsPage";
import { EditModelPage } from "./ui/pages/settings/EditModelPage";
import { ImageGenerationPage } from "./ui/pages/settings/ImageGenerationPage";
import { SystemPromptsPage } from "./ui/pages/settings/SystemPromptsPage";
import { EditPromptTemplate } from "./ui/pages/settings/EditPromptTemplate";
import { SecurityPage } from "./ui/pages/settings/SecurityPage";
import { ResetPage } from "./ui/pages/settings/ResetPage";
import { BackupRestorePage } from "./ui/pages/settings/BackupRestorePage";
import { ConvertPage } from "./ui/pages/settings/ConvertPage";
import { UsagePage } from "./ui/pages/settings/UsagePage";
import { AccessibilityPage } from "./ui/pages/settings/AccessibilityPage";
import { ColorCustomizationPage } from "./ui/pages/settings/ColorCustomizationPage";
import { ChatAppearancePage } from "./ui/pages/settings/ChatAppearancePage";
import { LogsPage } from "./ui/pages/settings/LogsPage";
import { CharactersPage } from "./ui/pages/settings/CharactersPage";
import { DeveloperPage } from "./ui/pages/settings/DeveloperPage";
import { ChangelogPage } from "./ui/pages/settings/ChangelogPage";
import { AdvancedPage } from "./ui/pages/settings/AdvancedPage";
import { CreationHelperPage as AICreationHelperPage } from "./ui/pages/settings/CreationHelperPage";
import { HelpMeReplyPage } from "./ui/pages/settings/HelpMeReplyPage";
import { VoicesPage } from "./ui/pages/settings/VoicesPage";
import { DynamicMemoryPage } from "./ui/pages/settings/DynamicMemoryPage";
import { EmbeddingDownloadPage } from "./ui/pages/settings/EmbeddingDownloadPage";
import { EmbeddingTestPage } from "./ui/pages/settings/EmbeddingTestPage";
import {
  ChatPage,
  ChatConversationPage,
  ChatSettingsPage,
  ChatHistoryPage,
  ChatMemoriesPage,
  SearchMessagesPage,
  ChatLayout,
} from "./ui/pages/chats";
import { ThemeProvider } from "./core/theme/ThemeContext";
import { toast } from "./ui/components/toast";
import {
  CreateCharacterPage,
  EditCharacterPage,
  LorebookEditor,
  CreationHelperPage,
} from "./ui/pages/characters";
import { CreatePersonaPage, PersonasPage, EditPersonaPage } from "./ui/pages/personas";
import { SearchPage } from "./ui/pages/search";
import { LibraryPage } from "./ui/pages/library/LibraryPage";
import { StandaloneLorebookEditor } from "./ui/pages/library/StandaloneLorebookEditor";
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
  GroupChatSettingsPage,
  GroupChatHistoryPage,
  GroupChatMemoriesPage,
} from "./ui/pages/group-chats";
import {
  EngineHomePage,
  EngineSetupWizard,
  EngineCharacterCreate,
  EngineChatPage,
  EngineProvidersConfigPage,
  EngineSettingsConfigPage,
} from "./ui/pages/engine";

import { CreateMenu, Tooltip, useFirstTimeTooltip } from "./ui/components";
import { V1UpgradeToast } from "./ui/components/V1UpgradeToast";
import { V2UpgradeToast } from "./ui/components/V2UpgradeToast";
import { ConfirmBottomMenuHost } from "./ui/components/ConfirmBottomMenu";
import { isOnboardingCompleted } from "./core/storage/appState";
import { TopNav, BottomNav } from "./ui/components/App";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { useAndroidBackHandler } from "./ui/hooks/useAndroidBackHandler";
import { logManager, isLoggingEnabled } from "./core/utils/logger";
import { storageBridge } from "./core/storage/files";
import { getPlatform } from "./core/utils/platform";

const chatLog = logManager({ component: "Chat" });

function App() {
  const [isMobile, setIsMobile] = useState(false);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const media = window.matchMedia("(max-width: 768px)");
    const update = () => setIsMobile(media.matches);
    update();
    if (typeof media.addEventListener === "function") {
      media.addEventListener("change", update);
      return () => media.removeEventListener("change", update);
    }
    media.addListener(update);
    return () => media.removeListener(update);
  }, []);

  useEffect(() => {
    const handleVisibilityChange = () => {
      if (document.visibilityState === "hidden") {
        storageBridge.dbCheckpoint().catch((err) => {
          console.warn("Failed to checkpoint database on background:", err);
        });
      }
    };

    const handleBeforeUnload = () => {
      storageBridge.dbCheckpoint().catch(() => {});
    };

    document.addEventListener("visibilitychange", handleVisibilityChange);
    window.addEventListener("beforeunload", handleBeforeUnload);

    return () => {
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      window.removeEventListener("beforeunload", handleBeforeUnload);
    };
  }, []);

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
              chatLog.with({ fn: state }).log(payload);
            } else {
              chatLog.with({ fn: state }).log(event.payload);
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
        unlisten = await listen("app://toast", (event) => {
          const payload = event.payload as Record<string, unknown> | null;
          if (!payload || typeof payload !== "object") {
            return;
          }
          const variant = payload.variant;
          const title = payload.title;
          const description = payload.description;
          if (typeof title !== "string") {
            return;
          }
          const detail = typeof description === "string" ? description : undefined;
          switch (variant) {
            case "success":
              toast.success(title, detail);
              break;
            case "warning":
              toast.warning(title, detail);
              break;
            case "error":
              toast.error(title, detail);
              break;
            default:
              toast.info(title, detail);
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

  return (
    <ThemeProvider>
      <BrowserRouter
        future={{
          v7_startTransition: true,
          v7_relativeSplatPath: true,
        }}
      >
        <div id="app-root" className="min-h-screen bg-surface text-fg antialiased">
          <Toaster
            position={isMobile ? "bottom-center" : "top-center"}
            offset={isMobile ? { bottom: 24 } : { top: 16 }}
            toastOptions={{
              unstyled: true,
              className: "pointer-events-auto w-full max-w-md",
              descriptionClassName: "text-xs text-fg/70",
            }}
          />
          <ConfirmBottomMenuHost />
          <AppContent />
        </div>
      </BrowserRouter>
    </ThemeProvider>
  );
}

function AppContent() {
  const location = useLocation();
  console.log("AppContent render:", location.pathname, location.key);
  const mainRef = useRef<HTMLDivElement | null>(null);
  const isChatRoute = location.pathname === "/chat" || location.pathname === "/";
  // Group chat detail: /group-chats/:id, /group-chats/:id/settings, /group-chats/new (NOT /group-chats list)
  const isGroupChatDetailRoute = location.pathname.startsWith("/group-chats/");
  const isEngineChatRoute = location.pathname.startsWith("/engine-chat/");
  const isChatDetailRoute = location.pathname.startsWith("/chat/") || isGroupChatDetailRoute || isEngineChatRoute;
  const isSearchRoute = location.pathname === "/search";
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
  const isCreateRoute = useMemo(
    () => location.pathname.startsWith("/create/"),
    [location.pathname],
  );

  const isSettingRoute = useMemo(
    () => location.pathname.startsWith("/settings"),
    [location.pathname],
  );
  const shouldAnimatePage = !location.pathname.startsWith("/settings/providers");

  const isLorebookEditorRoute = useMemo(
    () => location.pathname.startsWith("/library/lorebooks/"),
    [location.pathname],
  );

  const showTopNav =
    !isOnboardingRoute &&
    !isChatDetailRoute &&
    !isCreateRoute &&
    !isSearchRoute &&
    !isLorebookEditorRoute &&
    !isDiscoveryRoute;
  const showBottomNav =
    !isSettingRoute &&
    !isOnboardingRoute &&
    !isChatDetailRoute &&
    !isCreateRoute &&
    !isSearchRoute &&
    !isLorebookEditorRoute &&
    !isDiscoveryRoute;

  const [showCreateMenu, setShowCreateMenu] = useState(false);
  const { isVisible: showCreateTooltip, dismissTooltip: dismissCreateTooltip } =
    useFirstTimeTooltip("create_button");
  const [showDelayedTooltip, setShowDelayedTooltip] = useState(false);

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
    const urlParams = new URLSearchParams(location.search);
    const firstTime = urlParams.get("firstTime");

    if (firstTime === "true" && isChatRoute) {
      window.history.replaceState({}, document.title, location.pathname);

      const timer = window.setTimeout(() => {
        setShowDelayedTooltip(true);
      }, 2000);

      return () => {
        window.clearTimeout(timer);
        setShowDelayedTooltip(false);
      };
    } else {
      setShowDelayedTooltip(false);
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

  const [glitchStage, setGlitchStage] = useState<0 | 1 | 2 | 3>(0);
  const glitchStageRef = useRef<0 | 1 | 2 | 3>(0);
  const shakeCooldownRef = useRef(0);
  const lastShakeRef = useRef(0);
  const glitchTimeoutRef = useRef<number | null>(null);
  const [glitchEnabled, setGlitchEnabled] = useState(true);
  const [voidActive, setVoidActive] = useState(false);
  const [voidTextIndex, setVoidTextIndex] = useState(0);
  const [showRestore, setShowRestore] = useState(false);
  const [voidStartAt, setVoidStartAt] = useState(0);
  const [voidReady, setVoidReady] = useState(false);
  const voidMessage = "congrats, you destablised the app. enjoy emptiness";

  useEffect(() => {
    glitchStageRef.current = glitchStage;
  }, [glitchStage]);

  useEffect(() => {
    const key = "lettuce.easterEggs.glitch";
    const applyStored = (value: string | null | undefined) => {
      if (value === null || value === undefined) return;
      setGlitchEnabled(value === "true");
    };
    const syncFromStorage = () => {
      try {
        applyStored(localStorage.getItem(key));
      } catch {
        setGlitchEnabled(true);
      }
    };
    syncFromStorage();
    const handleStorage = (event: StorageEvent) => {
      if (event.key === key) {
        applyStored(event.newValue);
      }
    };
    const handleToggleEvent = (event: Event) => {
      const detail = (event as CustomEvent<boolean>).detail;
      if (typeof detail === "boolean") {
        setGlitchEnabled(detail);
      } else {
        syncFromStorage();
      }
    };
    window.addEventListener("storage", handleStorage);
    window.addEventListener("lettuce:easterEggs:glitch", handleToggleEvent);
    return () => {
      window.removeEventListener("storage", handleStorage);
      window.removeEventListener("lettuce:easterEggs:glitch", handleToggleEvent);
    };
  }, []);

  useEffect(() => {
    const platform = getPlatform();
    if (platform.type !== "mobile" || !glitchEnabled) return;

    let mounted = true;
    const threshold = 18;
    const cooldownMs = 4000;
    const stageCooldownMs = 1000;

    const handleMotion = (event: DeviceMotionEvent) => {
      if (!mounted) return;
      const accel = event.accelerationIncludingGravity;
      if (!accel) return;
      const x = accel.x ?? 0;
      const y = accel.y ?? 0;
      const z = accel.z ?? 0;
      const magnitude = Math.sqrt(x * x + y * y + z * z);
      const now = Date.now();

      if (magnitude > threshold) {
        const canTrigger = now - shakeCooldownRef.current > cooldownMs;
        if (!canTrigger && glitchStageRef.current === 3) return;

        if (now - lastShakeRef.current < stageCooldownMs) {
          return;
        }
        lastShakeRef.current = now;

        const currentStage = glitchStageRef.current;
        const nextStage = currentStage === 0 ? 2 : ((currentStage + 1) as 1 | 2 | 3);

        setGlitchStage(nextStage);
        glitchStageRef.current = nextStage;

        if (glitchTimeoutRef.current) {
          window.clearTimeout(glitchTimeoutRef.current);
          glitchTimeoutRef.current = null;
        }

        const durationMs = nextStage === 1 ? 1200 : nextStage === 2 ? 1500 : 1800;
        if (nextStage !== 3) {
          glitchTimeoutRef.current = window.setTimeout(() => {
            setGlitchStage(0);
          }, durationMs);
        }

        if (nextStage === 2) {
          toast.warning("Reality fracture detected.");
        } else if (nextStage === 3) {
          shakeCooldownRef.current = now;
          toast.info("Reality resynced.");
          setVoidActive(true);
          setVoidTextIndex(0);
          setShowRestore(false);
          setVoidStartAt(Date.now());
          setVoidReady(false);
        }
      }
    };

    window.addEventListener("devicemotion", handleMotion);
    return () => {
      mounted = false;
      window.removeEventListener("devicemotion", handleMotion);
      if (glitchTimeoutRef.current) {
        window.clearTimeout(glitchTimeoutRef.current);
        glitchTimeoutRef.current = null;
      }
    };
  }, [glitchEnabled]);

  useEffect(() => {
    if (glitchEnabled) return;
    setGlitchStage(0);
    glitchStageRef.current = 0;
    setVoidActive(false);
    setShowRestore(false);
  }, [glitchEnabled]);

  useEffect(() => {
    if (!voidActive) return;
    const now = Date.now();
    const delayMs = 3000;
    if (now - voidStartAt < delayMs) {
      const timer = window.setTimeout(
        () => {
          setVoidReady(true);
        },
        delayMs - (now - voidStartAt),
      );
      return () => window.clearTimeout(timer);
    }
    if (!voidReady) {
      setVoidReady(true);
      return;
    }
    if (voidTextIndex >= voidMessage.length) {
      const timer = window.setTimeout(() => setShowRestore(true), 1200);
      return () => window.clearTimeout(timer);
    }
    const timer = window.setTimeout(() => {
      setVoidTextIndex((prev) => Math.min(voidMessage.length, prev + 1));
    }, 45);
    return () => window.clearTimeout(timer);
  }, [voidActive, voidStartAt, voidReady, voidTextIndex, voidMessage.length]);

  return (
    <div
      className={`relative min-h-screen overflow-hidden ${
        glitchStage ? `app-glitch app-glitch-${glitchStage}` : ""
      }`}
    >
      <div
        className={`relative z-10 mx-auto flex min-h-screen w-full ${
          isChatDetailRoute ? "max-w-full" : "max-w-md lg:max-w-none"
        } flex-col ${showBottomNav ? "pb-[calc(72px+env(safe-area-inset-bottom))]" : "pb-0"}`}
      >
        {showTopNav && <TopNav currentPath={location.pathname + location.search} />}

        <main
          ref={mainRef}
          className={`app-fall-target flex-1 ${showTopNav ? "pt-[calc(72px+env(safe-area-inset-top))]" : ""} ${
            isOnboardingRoute
              ? `overflow-y-auto ${isDesktop ? "" : "px-0 pt-5 pb-5"}`
              : isChatDetailRoute
                ? "overflow-hidden px-0 pt-0 pb-0"
                : isCreateRoute
                  ? "overflow-hidden px-0 pt-0 pb-0"
                  : isSearchRoute
                    ? "overflow-hidden px-0 pt-0 pb-0"
                    : isLorebookEditorRoute
                      ? "overflow-hidden px-0 pt-0 pb-0"
                      : isDiscoveryRoute
                        ? "overflow-hidden px-0 pt-0 pb-0"
                        : `overflow-y-auto px-4 pt-4 ${showBottomNav ? "pb-[calc(96px+env(safe-area-inset-bottom))]" : "pb-6"}`
          }`}
        >
          {voidActive && (
            <div className="void-overlay pointer-events-none absolute inset-0 z-20 flex items-center justify-center">
              <div className="pointer-events-auto max-w-xs px-6 py-5 text-center">
                <p className="text-sm text-fg/70">
                  {voidMessage.slice(0, voidTextIndex)}
                  <span className="ml-0.5 inline-block h-4 w-2 animate-pulse bg-fg/40 align-middle" />
                </p>
                {showRestore && (
                  <button
                    onClick={() => {
                      setVoidActive(false);
                      setShowRestore(false);
                      setGlitchStage(0);
                      glitchStageRef.current = 0;
                      if (glitchTimeoutRef.current) {
                        window.clearTimeout(glitchTimeoutRef.current);
                        glitchTimeoutRef.current = null;
                      }
                    }}
                    className="mt-4 rounded-full border border-fg/20 bg-fg/10 px-4 py-2 text-xs font-semibold text-fg hover:border-fg/40 hover:bg-fg/15"
                  >
                    Restore
                  </button>
                )}
              </div>
            </div>
          )}
          <motion.div
            key={(() => {
              if (location.pathname.startsWith("/settings")) return location.pathname;
              const chatMatch = location.pathname.match(/^\/chat\/([^/]+)/);
              if (chatMatch) return `/chat/${chatMatch[1]}`;
              const groupMatch = location.pathname.match(/^\/group-chats\/([^/]+)/);
              if (groupMatch) return `/group-chats/${groupMatch[1]}`;
              return location.key;
            })()}
            initial={shouldAnimatePage ? { opacity: 0, y: 16 } : false}
            animate={{ opacity: 1, y: 0 }}
            exit={shouldAnimatePage ? { opacity: 0, y: -16 } : { opacity: 1, y: 0 }}
            transition={shouldAnimatePage ? { duration: 0.2, ease: "easeOut" } : { duration: 0 }}
            className="h-full"
          >
            <Routes>
              <Route path="/" element={<OnboardingCheck />} />
              <Route path="/welcome" element={<WelcomePage />} />
              <Route path="/onboarding/provider" element={<OnboardingPage />} />
              <Route path="/onboarding/models" element={<OnboardingPage />} />
              <Route path="/onboarding/memory" element={<OnboardingPage />} />
              <Route path="/wheretofind" element={<WhereToFindPage />} />
              <Route path="/search" element={<SearchPage />} />
              <Route path="/discover" element={<DiscoveryPage />} />
              <Route path="/discover/search" element={<DiscoverySearchPage />} />
              <Route path="/discover/browse" element={<DiscoveryBrowsePage />} />
              <Route path="/discover/card/:path" element={<DiscoveryCardDetailPage />} />
              <Route path="/library" element={<LibraryPage />} />
              <Route path="/library/lorebooks/:lorebookId" element={<StandaloneLorebookEditor />} />
              <Route path="/settings" element={<SettingsPage />} />
              <Route path="/settings/providers" element={<ProvidersPage />} />
              <Route path="/settings/models" element={<ModelsPage />} />
              <Route path="/settings/models/new" element={<EditModelPage />} />
              <Route path="/settings/models/:modelId" element={<EditModelPage />} />
              <Route path="/settings/voices" element={<VoicesPage />} />
              <Route path="/settings/image-generation" element={<ImageGenerationPage />} />
              <Route path="/settings/prompts" element={<SystemPromptsPage />} />
              <Route path="/settings/prompts/new" element={<EditPromptTemplate />} />
              <Route path="/settings/prompts/:id" element={<EditPromptTemplate />} />
              <Route path="/settings/security" element={<SecurityPage />} />
              <Route path="/settings/usage" element={<UsagePage />} />
              <Route path="/settings/accessibility" element={<AccessibilityPage />} />
              <Route path="/settings/accessibility/colors" element={<ColorCustomizationPage />} />
              <Route path="/settings/accessibility/chat" element={<ChatAppearancePage />} />
              <Route path="/settings/logs" element={<LogsPage />} />
              <Route path="/settings/advanced" element={<AdvancedPage />} />
              <Route path="/settings/advanced/memory" element={<DynamicMemoryPage />} />
              <Route path="/settings/advanced/creation-helper" element={<AICreationHelperPage />} />
              <Route path="/settings/advanced/help-me-reply" element={<HelpMeReplyPage />} />
              <Route path="/settings/embedding-download" element={<EmbeddingDownloadPage />} />
              <Route path="/settings/embedding-test" element={<EmbeddingTestPage />} />
              <Route path="/settings/changelog" element={<ChangelogPage />} />
              <Route path="/settings/developer" element={<DeveloperPage />} />
              <Route path="/settings/reset" element={<ResetPage />} />
              <Route path="/settings/backup" element={<BackupRestorePage />} />
              <Route path="/settings/convert" element={<ConvertPage />} />
              <Route path="/settings/sync" element={<SyncPage />} />
              <Route path="/settings/engine/:credentialId" element={<EngineHomePage />} />
              <Route path="/settings/engine/:credentialId/setup" element={<EngineSetupWizard />} />
              <Route path="/settings/engine/:credentialId/providers" element={<EngineProvidersConfigPage />} />
              <Route path="/settings/engine/:credentialId/settings" element={<EngineSettingsConfigPage />} />
              <Route path="/settings/engine/:credentialId/character/new" element={<EngineCharacterCreate />} />
              <Route path="/engine-chat/:credentialId/:slug" element={<EngineChatPage />} />
              <Route path="/chat" element={<ChatPage />} />
              <Route path="/chat/:characterId" element={<ChatLayout />}>
                <Route index element={<ChatConversationPage />} />
                <Route path="settings" element={<ChatSettingsPage />} />
              </Route>
              <Route path="/chat/:characterId/search" element={<SearchMessagesPage />} />
              <Route path="/chat/:characterId/history" element={<ChatHistoryPage />} />
              <Route path="/chat/:characterId/memories" element={<ChatMemoriesPage />} />
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
              <Route path="/create/persona" element={<CreatePersonaPage />} />
              <Route path="/personas" element={<PersonasPage />} />
              <Route path="/settings/personas" element={<PersonasPage />} />
              <Route path="/settings/personas/:personaId/edit" element={<EditPersonaPage />} />
              <Route path="/group-chats" element={<GroupChatsListPage />} />
              <Route path="/group-chats/history" element={<GroupChatHistoryPage />} />
              <Route path="/group-chats/new" element={<GroupChatCreatePage />} />
              <Route path="/group-chats/:groupSessionId" element={<GroupChatLayout />}>
                <Route index element={<GroupChatPage />} />
                <Route path="settings" element={<GroupChatSettingsPage />} />
                <Route path="memories" element={<GroupChatMemoriesPage />} />
              </Route>
            </Routes>
          </motion.div>
        </main>

        {showBottomNav && <BottomNav onCreateClick={() => setShowCreateMenu(true)} />}
      </div>

      {showBottomNav && (
        <CreateMenu isOpen={showCreateMenu} onClose={() => setShowCreateMenu(false)} />
      )}

      {isChatRoute && showBottomNav && (showDelayedTooltip || showCreateTooltip) && (
        <Tooltip
          isVisible={true}
          message="Create custom AI characters and personas here!"
          onClose={() => {
            dismissCreateTooltip();
            setShowDelayedTooltip(false);
          }}
          position="bottom"
          className="bottom-22 right-4"
        />
      )}

      {/* V1 Embedding Model Upgrade Toast */}
      <V1UpgradeToast />
      {/* V2 Embedding Model Upgrade Toast */}
      <V2UpgradeToast />
    </div>
  );
}

function OnboardingCheck() {
  const [isChecking, setIsChecking] = useState(true);
  const [shouldShowOnboarding, setShouldShowOnboarding] = useState(false);

  useEffect(() => {
    let cancelled = false;

    const checkOnboarding = async () => {
      const onboardingCompleted = await isOnboardingCompleted();
      if (cancelled) return;
      if (!onboardingCompleted) {
        setShouldShowOnboarding(true);
      }
      setIsChecking(false);
    };

    checkOnboarding();

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
