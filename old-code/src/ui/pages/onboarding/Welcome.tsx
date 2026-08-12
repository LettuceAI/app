import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import {
  AlertTriangle,
  ArrowRight,
  Upload,
  Smartphone,
  FileArchive,
  Lock,
  Loader2,
  Eye,
  EyeOff,
  CheckCircle,
  HelpCircle,
  ChevronDown,
} from "lucide-react";
import { motion } from "framer-motion";

import { setOnboardingCompleted, setOnboardingSkipped } from "../../../core/storage/appState";
import { hasConfiguredModel } from "../../../core/storage/repo";
import { storageBridge } from "../../../core/storage/files";
import logoSvg from "../../../assets/logo.svg";
import { typography, radius, spacing, interactive, shadows, cn } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import { LocaleSelector } from "../../components/LocaleSelector";
import { BottomMenu, MenuButton, MenuDivider, MenuSection } from "../../components/BottomMenu";
import { DynamicMemoryEmbeddingPrompt } from "./components/DynamicMemoryEmbeddingPrompt";

interface WelcomePageProps {
  onContinue?: () => void;
  onGoToSync?: () => void;
}

export function WelcomePage({ onContinue, onGoToSync }: WelcomePageProps = {}) {
  const { locale, setLocale, t } = useI18n();
  const navigate = useNavigate();
  const [showSkipWarning, setShowSkipWarning] = useState(false);
  const [showRestoreBackup, setShowRestoreBackup] = useState(false);
  const [showMoreOptions, setShowMoreOptions] = useState(false);

  // Ctrl+Shift+L to open logs page during onboarding
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && e.key === "L") {
        e.preventDefault();
        navigate("/settings/logs");
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [navigate]);

  // Load brand fonts (Fraunces for display, Noto Sans for body — same as lettuceai.app)
  useEffect(() => {
    const id = "lai-brand-fonts";
    if (document.getElementById(id)) return;
    const preconnect = document.createElement("link");
    preconnect.rel = "preconnect";
    preconnect.href = "https://fonts.gstatic.com";
    preconnect.crossOrigin = "anonymous";
    document.head.appendChild(preconnect);
    const link = document.createElement("link");
    link.id = id;
    link.rel = "stylesheet";
    link.href =
      "https://fonts.googleapis.com/css2?family=Fraunces:opsz,wght@9..144,300..900&family=Noto+Sans:wght@400;500;600;700&display=swap";
    document.head.appendChild(link);
  }, []);

  const handleAddProvider = () => {
    if (onContinue) onContinue();
    else navigate("/onboarding/provider");
  };

  const handleConfirmSkip = async () => {
    await setOnboardingCompleted(true);
    await setOnboardingSkipped(true);
    // Small delay to ensure state is persisted before navigation
    await new Promise((resolve) => setTimeout(resolve, 100));
    navigate("/");
  };

  const handleSkipRequest = async () => {
    if (await hasConfiguredModel()) {
      await handleConfirmSkip();
      return;
    }
    setShowSkipWarning(true);
  };

  const handleRestoreComplete = async () => {
    await setOnboardingCompleted(true);
    navigate("/chat");
  };

  return (
    <div className="relative flex h-[calc(100dvh-var(--titlebar-h,0px))] flex-col overflow-hidden text-white antialiased pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)] font-['Noto_Sans',ui-sans-serif,system-ui,sans-serif]">
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-x-0 bottom-0 z-0 h-3/4 bg-[linear-gradient(180deg,transparent_0%,rgba(5,5,5,0.12)_40%,rgba(5,5,5,0.68)_100%)] lg:hidden"
      />

      {/* Top bar */}
      <motion.div
        className="relative z-10 flex items-center justify-between px-6 pt-6 lg:px-12 lg:pt-8"
        initial={{ opacity: 0, y: -6 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.4 }}
      >
        <div className="flex items-center gap-2.5">
          <img src={logoSvg} alt="" className="h-7 w-7" />
          <span className="text-[18px] font-bold tracking-[-0.02em] text-[#f4f4f5]">LettuceAI</span>
        </div>

        <div className="[&_.tabular-nums]:hidden lg:hidden">
          <LocaleSelector
            value={locale}
            onChange={setLocale}
            label=""
            description=""
            title={t("components.localeSelector.title")}
            labelClassName="hidden"
            descriptionClassName="hidden"
            triggerClassName="bg-transparent! border-0! p-0! py-1! text-[14px]! text-white/[0.62]! shadow-none! [backdrop-filter:none]! hover:bg-transparent! hover:text-[#f4f4f5]!"
            menuClassName=""
          />
        </div>
      </motion.div>

      <div className="relative z-10 flex flex-1 flex-col items-center justify-end px-6 pb-10 pt-6 lg:justify-center lg:px-16 lg:py-12">
        <div className="w-full max-w-2xl flex flex-col items-center text-center">
          <motion.div
            className="relative hidden overflow-visible lg:block"
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5, ease: "easeOut" }}
          >
            <div className="absolute inset-[-30px] rounded-full blur-[22px] bg-[radial-gradient(closest-side,rgba(0,210,148,0.20),transparent_70%)]" aria-hidden="true" />
            <img
              src={logoSvg}
              alt={t("onboarding.welcome.appName")}
              className="relative h-14 w-14 lg:h-20 lg:w-20"
            />
          </motion.div>

          {/* Headline */}
          <motion.h1
            className="mt-5 lg:mt-9 font-bold tracking-[-0.03em] text-[#f4f4f5] text-[clamp(29px,8vw,37px)] leading-[1.05] max-w-[14ch] [text-shadow:0_2px_18px_rgba(0,0,0,0.45)] lg:text-[clamp(37px,5vw,57px)] lg:leading-[1.08] lg:max-w-[18ch]"
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5, delay: 0.05 }}
          >
            {t("onboarding.welcome.headline.lead")}{" "}
            <span className="font-['Fraunces','Iowan_Old_Style',Georgia,serif] italic font-medium text-[#00d294] tracking-[-0.02em] [font-variation-settings:'opsz'_96]">
              {t("onboarding.welcome.headline.accent")}
            </span>
          </motion.h1>

          {/* Tagline */}
          <motion.p
            className="mt-3 lg:mt-5 max-w-sm lg:max-w-lg font-normal text-white/[0.72] text-[14px] leading-[1.45] [text-shadow:0_1px_10px_rgba(0,0,0,0.6)] lg:text-[16.5px] lg:leading-[1.55] lg:[text-shadow:none]"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.5, delay: 0.15 }}
          >
            {t("onboarding.welcome.tagline")}
          </motion.p>

          {/* Primary CTA */}
          <motion.button
            className="group mt-6 lg:mt-10 inline-flex items-center gap-2.5 w-full justify-between cursor-pointer rounded-xl border border-[rgba(0,210,148,0.45)] bg-[linear-gradient(180deg,rgba(0,210,148,0.28),rgba(0,210,148,0.16))] backdrop-blur-[10px] transition-colors py-3 px-4 shadow-[inset_0_1px_0_rgba(255,255,255,0.10),0_10px_28px_-8px_rgba(0,210,148,0.40),0_18px_44px_-12px_rgba(0,0,0,0.6)] hover:border-[rgba(0,210,148,0.65)] hover:bg-[linear-gradient(180deg,rgba(0,210,148,0.36),rgba(0,210,148,0.22))] lg:w-auto lg:justify-start lg:py-3 lg:pr-[18px] lg:pl-[22px]"
            onClick={handleAddProvider}
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            whileTap={{ scale: 0.985 }}
            transition={{ duration: 0.5, delay: 0.22, scale: { duration: 0.12, delay: 0 } }}
          >
            <span className="text-[16px] font-semibold tracking-[-0.005em] text-white">{t("onboarding.welcome.getStarted")}</span>
            <span className="inline-flex h-[22px] w-[22px] items-center justify-center rounded-full bg-white/10 text-white transition group-hover:translate-x-0.5 group-hover:bg-white/[0.18]">
              <ArrowRight size={16} strokeWidth={2.25} />
            </span>
          </motion.button>

          <motion.div
            className="mt-3 w-full lg:mt-5 lg:w-auto"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.5, delay: 0.28 }}
          >
            <button
              onClick={() => setShowMoreOptions(true)}
              className="group flex w-full items-center justify-center gap-1.5 lg:hidden cursor-pointer rounded-xl border border-white/[0.18] bg-white/[0.07] backdrop-blur-[10px] py-[13px] px-4 text-[15px] font-semibold text-[#f4f4f5] transition hover:border-white/[0.28] hover:bg-white/[0.11] active:scale-[0.985]"
            >
              <span>{t("onboarding.welcome.moreOptions")}</span>
              <ChevronDown size={15} strokeWidth={2} />
            </button>
            <button
              onClick={() => setShowMoreOptions(true)}
              className="group hidden items-center gap-1.5 py-1 lg:inline-flex cursor-pointer bg-transparent text-[13.5px] text-white/[0.62] transition-colors hover:text-[#f4f4f5]"
            >
              <span className="underline decoration-white/[0.18] underline-offset-4 transition group-hover:decoration-[rgba(0,210,148,0.55)]">{t("onboarding.welcome.moreOptions")}</span>
              <ChevronDown size={14} strokeWidth={2} />
            </button>
          </motion.div>
        </div>
      </div>

      {/* Bottom footer strip — desktop only */}
      <motion.div
        className="relative z-10 hidden lg:block px-6 py-4 lg:px-12 lg:py-5 border-t border-white/[0.08]"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ duration: 0.4, delay: 0.3 }}
      >
        <div className="flex flex-col-reverse gap-3 lg:flex-row lg:items-center lg:justify-between">
          <div className="[&_.tabular-nums]:hidden flex items-center w-full lg:w-auto">
            <LocaleSelector
              value={locale}
              onChange={setLocale}
              label=""
              description=""
              title={t("components.localeSelector.title")}
              labelClassName="hidden"
              descriptionClassName="hidden"
              triggerClassName="bg-transparent! border-0! p-0! py-1! text-[14px]! text-white/[0.62]! shadow-none! [backdrop-filter:none]! hover:bg-transparent! hover:text-[#f4f4f5]!"
              menuClassName=""
            />
          </div>
          <p className="text-[12px] font-normal text-white/40 text-center lg:text-right">
            {t("onboarding.welcome.languageSelector.description")}
          </p>
        </div>
      </motion.div>

      <BottomMenu
        isOpen={showMoreOptions}
        onClose={() => setShowMoreOptions(false)}
        title={t("onboarding.welcome.moreOptions")}
        location="bottom"
      >
        <MenuSection>
          <MenuButton
            icon={Upload}
            title={t("onboarding.welcome.restoreFromBackup")}
            description={t("onboarding.welcome.moreMenu.restoreDesc")}
            color="from-blue-500 to-blue-600"
            onClick={() => {
              setShowMoreOptions(false);
              setShowRestoreBackup(true);
            }}
          />
          <MenuButton
            icon={Smartphone}
            title={t("onboarding.welcome.syncFromDevice")}
            description={t("onboarding.welcome.moreMenu.syncDesc")}
            color="from-purple-500 to-purple-600"
            onClick={() => {
              setShowMoreOptions(false);
              if (onGoToSync) onGoToSync();
              else navigate("/onboarding/sync");
            }}
          />
          <MenuButton
            icon={HelpCircle}
            title={t("onboarding.welcome.readFaq")}
            description={t("onboarding.welcome.moreMenu.faqDesc")}
            color="from-emerald-500 to-emerald-600"
            onClick={() => {
              setShowMoreOptions(false);
              navigate("/settings/help", { state: { fromWelcome: true } });
            }}
          />
          <MenuDivider />
          <MenuButton
            icon={ArrowRight}
            title={t("onboarding.welcome.skipForNow")}
            description={t("onboarding.welcome.moreMenu.skipDesc")}
            color="from-white/20 to-white/10"
            onClick={() => {
              setShowMoreOptions(false);
              void handleSkipRequest();
            }}
          />
        </MenuSection>
      </BottomMenu>

      {showSkipWarning && (
        <SkipWarning
          onClose={() => setShowSkipWarning(false)}
          onConfirm={handleConfirmSkip}
          onAddProvider={handleAddProvider}
        />
      )}

      {showRestoreBackup && (
        <RestoreBackupModal
          onClose={() => setShowRestoreBackup(false)}
          onComplete={handleRestoreComplete}
        />
      )}
    </div>
  );
}

function SkipWarning({
  onClose,
  onConfirm,
  onAddProvider,
}: {
  onClose: () => void;
  onConfirm: () => void | Promise<void>;
  onAddProvider: () => void;
}) {
  const { t } = useI18n();
  const [isExiting, setIsExiting] = useState(false);

  const handleClose = () => {
    setIsExiting(true);
    setTimeout(onClose, 200);
  };

  const handleConfirm = () => {
    setIsExiting(true);
    setTimeout(() => void onConfirm(), 200);
  };

  const handleAddProvider = () => {
    setIsExiting(true);
    setTimeout(onAddProvider, 200);
  };

  return (
    <motion.div
      className="fixed inset-0 z-50 flex items-end justify-center bg-black/60 backdrop-blur-sm"
      initial={{ opacity: 0 }}
      animate={{ opacity: isExiting ? 0 : 1 }}
      transition={{ duration: 0.2 }}
      onClick={handleClose}
    >
      <motion.div
        className={cn(
          "w-full max-w-lg border border-white/10 bg-[#0b0b0d] p-6",
          "rounded-t-3xl sm:rounded-3xl sm:mb-8",
          shadows.xl,
        )}
        initial={{ y: "100%", opacity: 0 }}
        animate={{
          y: isExiting ? "100%" : 0,
          opacity: isExiting ? 0 : 1,
        }}
        transition={{
          type: "spring",
          damping: 30,
          stiffness: 350,
          duration: 0.2,
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <h3 className={cn(typography.h2.size, typography.h2.weight, "text-white")}>
            {t("onboarding.welcome.skipWarning.title")}
          </h3>
        </div>

        {/* Warning content */}
        <div
          className={cn(
            "flex items-start gap-3 border border-red-500/40 bg-red-500/10 p-4 mb-6",
            radius.md,
          )}
        >
          <div
            className={cn(
              "flex h-10 w-10 shrink-0 items-center justify-center bg-red-500/25 text-red-300",
              radius.md,
            )}
          >
            <AlertTriangle size={20} strokeWidth={2.5} />
          </div>
          <div className={spacing.tight}>
            <h4 className={cn(typography.body.size, typography.h3.weight, "text-red-200")}>
              {t("onboarding.welcome.skipWarning.warningTitle")}
            </h4>
            <p
              className={cn(
                typography.bodySmall.size,
                typography.bodySmall.lineHeight,
                "text-red-100/80",
              )}
            >
              {t("onboarding.welcome.skipWarning.warningMessage")}
            </p>
          </div>
        </div>

        {/* Actions */}
        <div className={cn("flex flex-col", spacing.field)}>
          <button
            className={cn(
              "inline-flex items-center justify-center gap-2 px-6 py-3",
              radius.md,
              "border border-emerald-400/40 bg-emerald-400/20 text-emerald-100",
              typography.body.size,
              typography.h3.weight,
              interactive.transition.fast,
              interactive.active.scale,
              "hover:border-emerald-400/60 hover:bg-emerald-400/30",
            )}
            onClick={handleAddProvider}
          >
            <span>{t("onboarding.welcome.skipWarning.addProvider")}</span>
            <ArrowRight size={16} strokeWidth={2.5} />
          </button>
          <button
            className={cn(
              "px-6 py-3",
              radius.md,
              "border border-white/10 bg-white/5 text-white/60",
              typography.body.size,
              interactive.transition.fast,
              interactive.active.scale,
              "hover:border-white/20 hover:bg-white/10 hover:text-white",
            )}
            onClick={handleConfirm}
          >
            {t("onboarding.welcome.skipWarning.skipAnyway")}
          </button>
        </div>
      </motion.div>
    </motion.div>
  );
}

interface BackupInfo {
  version: number;
  createdAt: number;
  appVersion: string;
  encrypted: boolean;
  totalFiles: number;
  path: string;
  filename: string;
}

function RestoreBackupModal({
  onClose,
  onComplete: _onComplete,
}: {
  onClose: () => void;
  onComplete: () => void | Promise<void>;
}) {
  const { t } = useI18n();
  const [isExiting, setIsExiting] = useState(false);
  const [backups, setBackups] = useState<BackupInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedBackup, setSelectedBackup] = useState<BackupInfo | null>(null);
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showEmbeddingPrompt, setShowEmbeddingPrompt] = useState(false);
  const navigate = useNavigate();

  useEffect(() => {
    loadBackups();
  }, []);

  const loadBackups = async () => {
    try {
      setLoading(true);
      const list = await storageBridge.backupList();
      setBackups(list);
    } catch (e) {
      console.error("Failed to load backups:", e);
    } finally {
      setLoading(false);
    }
  };

  const handleBrowseForBackup = async () => {
    try {
      setError(null);
      setLoading(true);
      const result = await storageBridge.backupPickFile();
      if (!result) return;

      const { path, filename } = result;

      const info = await storageBridge.backupGetInfo(path);

      const backupInfo: BackupInfo = {
        ...info,
        path,
        filename,
      };

      setSelectedBackup(backupInfo);
      setPassword("");
    } catch (e) {
      console.error("Failed to browse for backup:", e);
      setError(e instanceof Error ? e.message : t("onboarding.welcome.restoreBackup.errors.failedToOpenFile"));
    } finally {
      setLoading(false);
    }
  };

  const handleClose = () => {
    if (restoring) return;
    setIsExiting(true);
    setTimeout(onClose, 200);
  };

  const handleRestore = async () => {
    if (!selectedBackup) return;

    if (selectedBackup.encrypted && password.length < 1) {
      setError(t("onboarding.welcome.restoreBackup.errors.passwordRequired"));
      return;
    }

    try {
      setError(null);

      if (selectedBackup.encrypted) {
        const valid = await storageBridge.backupVerifyPassword(selectedBackup.path, password);
        if (!valid) {
          setError(t("onboarding.welcome.restoreBackup.errors.incorrectPassword"));
          return;
        }
      }

      setRestoring(true);

      const hasDynamicMemory = await storageBridge.backupCheckDynamicMemory(
        selectedBackup.path,
        selectedBackup.encrypted ? password : undefined,
      );

      await storageBridge.backupImport(
        selectedBackup.path,
        selectedBackup.encrypted ? password : undefined,
      );

      await setOnboardingCompleted(true);

      if (hasDynamicMemory) {
        const hasEmbeddingModel = await storageBridge.checkEmbeddingModel();
        if (!hasEmbeddingModel) {
          setRestoring(false);
          setShowEmbeddingPrompt(true);
          return;
        }
      }

      setIsExiting(true);
      setTimeout(() => {
        navigate("/");
      }, 200);
    } catch (e) {
      console.log(e);
      setError(e instanceof Error ? e.message : t("onboarding.welcome.restoreBackup.errors.failedToRestore"));
      setRestoring(false);
    }
  };

  const handleDownloadModel = () => {
    setShowEmbeddingPrompt(false);
    handleClose();
    navigate("/settings/embedding-download?returnTo=/");
  };

  const handleDisableAndContinue = async () => {
    setShowEmbeddingPrompt(false);
    setRestoring(true);

    try {
      await storageBridge.backupDisableDynamicMemory();

      navigate("/");
    } catch (error) {
      console.error("Failed to disable dynamic memory:", error);
      setError(error instanceof Error ? error.message : t("onboarding.welcome.restoreBackup.errors.failedToUpdateSettings"));
    } finally {
      setRestoring(false);
    }
  };

  const formatDate = (timestamp: number) => {
    const date = new Date(timestamp);
    return `${date.toLocaleDateString(undefined, { month: "short", day: "numeric" })}, ${date.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" })}`;
  };

  return (
    <motion.div
      className="fixed inset-0 z-50 flex items-end justify-center bg-black/60 backdrop-blur-sm"
      initial={{ opacity: 0 }}
      animate={{ opacity: isExiting ? 0 : 1 }}
      transition={{ duration: 0.2 }}
      onClick={handleClose}
    >
      <motion.div
        className={cn(
          "w-full max-w-lg border border-white/10 bg-[#0b0b0d] p-6",
          "rounded-t-3xl sm:rounded-3xl sm:mb-8",
          "max-h-[80vh] overflow-hidden flex flex-col",
          shadows.xl,
        )}
        initial={{ y: "100%", opacity: 0 }}
        animate={{
          y: isExiting ? "100%" : 0,
          opacity: isExiting ? 0 : 1,
        }}
        transition={{
          type: "spring",
          damping: 30,
          stiffness: 350,
          duration: 0.2,
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between mb-4">
          <h3 className={cn(typography.h2.size, typography.h2.weight, "text-white")}>
            {t("onboarding.welcome.restoreBackup.title")}
          </h3>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto space-y-4">
          {!selectedBackup ? (
            <>
              <div className="flex items-center justify-between">
                <p className={cn(typography.bodySmall.size, "text-white/50")}>
                  {t("onboarding.welcome.restoreBackup.selectMessage")}
                </p>
                <button
                  onClick={handleBrowseForBackup}
                  className="text-[13px] font-medium text-blue-400 hover:text-blue-300"
                >
                  {t("onboarding.welcome.restoreBackup.browse")}
                </button>
              </div>

              {/* Error display for list view */}
              {error && (
                <div
                  className={cn(
                    "flex items-start gap-2 border border-red-400/30 bg-red-400/10 px-3 py-2 text-[15px] text-red-200 mb-4",
                    radius.md,
                  )}
                >
                  <AlertTriangle className="h-4 w-4 shrink-0 mt-0.5" />
                  <span>{error}</span>
                </div>
              )}

              {loading ? (
                <div className="flex flex-col items-center justify-center py-8">
                  <Loader2 className="h-6 w-6 animate-spin text-white/30" />
                  <p className="mt-2 text-[15px] text-white/40">{t("onboarding.welcome.restoreBackup.processing")}</p>
                  <p className="text-[13px] text-white/20 mt-1">{t("onboarding.welcome.restoreBackup.processingNote")}</p>
                  <button
                    onClick={() => setLoading(false)}
                    className="mt-6 text-[13px] text-red-400/60 hover:text-red-300 transition-colors"
                  >
                    {t("onboarding.welcome.restoreBackup.cancel")}
                  </button>
                </div>
              ) : backups.length === 0 ? (
                <div className={cn("border border-white/10 bg-white/5 p-6 text-center", radius.md)}>
                  <FileArchive className="mx-auto h-8 w-8 text-white/20" />
                  <p className="mt-3 text-[15px] text-white/40">{t("onboarding.welcome.restoreBackup.noBackups")}</p>
                  <p className="mt-1 text-[13px] text-white/30">{t("onboarding.welcome.restoreBackup.noBackupsHint")}</p>
                  <button
                    onClick={handleBrowseForBackup}
                    className={cn(
                      "mt-4 inline-flex items-center gap-2 px-4 py-2 rounded-lg",
                      "border border-blue-400/30 bg-blue-400/10",
                      "text-[15px] text-blue-300 font-medium",
                      "hover:bg-blue-400/20 active:scale-[0.98]",
                      interactive.transition.default,
                    )}
                  >
                    <Upload className="h-4 w-4" />
                    {t("onboarding.welcome.restoreBackup.browseLettuce")}
                  </button>
                </div>
              ) : (
                <div className="space-y-2">
                  {backups.map((backup) => (
                    <button
                      key={backup.path}
                      onClick={() => {
                        setSelectedBackup(backup);
                        setPassword("");
                        setError(null);
                      }}
                      className={cn(
                        "w-full rounded-xl border border-white/10 bg-white/5 p-3 text-left",
                        interactive.transition.default,
                        "hover:border-white/20 hover:bg-white/8",
                        "active:scale-[0.99]",
                      )}
                    >
                      <div className="flex items-center gap-3">
                        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full border border-white/10 bg-white/10">
                          <FileArchive className="h-4 w-4 text-white/60" />
                        </div>
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-2">
                            <p className="truncate text-[15px] font-medium text-white">
                              {backup.filename}
                            </p>
                            {backup.encrypted && (
                              <Lock className="h-3 w-3 shrink-0 text-amber-400/70" />
                            )}
                          </div>
                          <p className="mt-0.5 text-[12px] text-white/40">
                            {formatDate(backup.createdAt)} · v{backup.appVersion}
                          </p>
                        </div>
                      </div>
                    </button>
                  ))}
                </div>
              )}
            </>
          ) : (
            <>
              {/* Selected backup info */}
              <div className={cn("border border-white/10 bg-white/5 p-3", radius.md)}>
                <div className="flex items-center gap-3">
                  <FileArchive className="h-6 w-6 text-white/40" />
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-[15px] font-medium text-white">
                      {selectedBackup.filename}
                    </p>
                    <p className="text-[13px] text-white/40">
                      {formatDate(selectedBackup.createdAt)} · v{selectedBackup.appVersion}
                    </p>
                  </div>
                </div>
              </div>

              {/* Info notice */}
              <div
                className={cn(
                  "flex items-start gap-2 border border-blue-400/30 bg-blue-400/10 px-3 py-2 text-[13px] text-blue-200",
                  radius.md,
                )}
              >
                <CheckCircle className="h-4 w-4 shrink-0 mt-0.5" />
                <span>{t("onboarding.welcome.restoreBackup.infoMessage")}</span>
              </div>

              {error && (
                <div
                  className={cn(
                    "flex items-center gap-2 border border-red-400/30 bg-red-400/10 px-3 py-2 text-[15px] text-red-200",
                    radius.md,
                  )}
                >
                  <AlertTriangle className="h-4 w-4 shrink-0" />
                  {error}
                </div>
              )}

              {selectedBackup.encrypted && (
                <div>
                  <label className="mb-1.5 block text-[13px] font-medium text-white/50">
                    {t("onboarding.welcome.restoreBackup.passwordLabel")}
                  </label>
                  <div className="relative">
                    <input
                      type={showPassword ? "text" : "password"}
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                      placeholder={t("onboarding.welcome.restoreBackup.passwordPlaceholder")}
                      className={cn(
                        "w-full border border-white/10 bg-white/5 px-4 py-3 pr-12 text-white placeholder-white/30",
                        radius.lg,
                        "focus:border-white/20 focus:outline-none",
                      )}
                    />
                    <button
                      type="button"
                      onClick={() => setShowPassword(!showPassword)}
                      className="absolute right-3 top-1/2 -translate-y-1/2 text-white/40 hover:text-white/60"
                    >
                      {showPassword ? <EyeOff className="h-5 w-5" /> : <Eye className="h-5 w-5" />}
                    </button>
                  </div>
                </div>
              )}
            </>
          )}
        </div>

        {/* Actions */}
        <div className={cn("flex flex-col pt-4", spacing.field)}>
          {selectedBackup ? (
            <>
              <button
                onClick={() => void handleRestore()}
                disabled={restoring || (selectedBackup.encrypted && password.length < 1)}
                className={cn(
                  "flex items-center justify-center gap-2 px-6 py-3",
                  radius.md,
                  "border border-emerald-400/40 bg-emerald-400/20 text-emerald-100",
                  typography.body.size,
                  typography.h3.weight,
                  interactive.transition.fast,
                  interactive.active.scale,
                  "hover:border-emerald-400/60 hover:bg-emerald-400/30",
                  "disabled:opacity-50",
                )}
              >
                {restoring ? (
                  <>
                    <Loader2 className="h-4 w-4 animate-spin" />
                    {t("onboarding.welcome.restoreBackup.restoring")}
                  </>
                ) : (
                  <>
                    <Upload size={16} />
                    {t("onboarding.welcome.restoreBackup.restoreButton")}
                  </>
                )}
              </button>
              <button
                onClick={() => setSelectedBackup(null)}
                disabled={restoring}
                className={cn(
                  "px-6 py-3",
                  radius.md,
                  "border border-white/10 bg-white/5 text-white/60",
                  typography.body.size,
                  interactive.transition.fast,
                  interactive.active.scale,
                  "hover:border-white/20 hover:bg-white/10 hover:text-white",
                  "disabled:opacity-50",
                )}
              >
                {t("onboarding.welcome.restoreBackup.back")}
              </button>
            </>
          ) : (
            <button
              onClick={handleClose}
              className={cn(
                "px-6 py-3",
                radius.md,
                "border border-white/10 bg-white/5 text-white/60",
                typography.body.size,
                interactive.transition.fast,
                interactive.active.scale,
                "hover:border-white/20 hover:bg-white/10 hover:text-white",
              )}
            >
              {t("onboarding.welcome.restoreBackup.cancel")}
            </button>
          )}
        </div>
      </motion.div>

      {showEmbeddingPrompt && (
        <DynamicMemoryEmbeddingPrompt
          onDownload={handleDownloadModel}
          onContinueWithout={handleDisableAndContinue}
        />
      )}
    </motion.div>
  );
}
