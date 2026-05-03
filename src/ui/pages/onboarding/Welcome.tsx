import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import {
  AlertTriangle,
  ArrowRight,
  ShieldCheck,
  Sparkles,
  Upload,
  FileArchive,
  Lock,
  Loader2,
  Eye,
  EyeOff,
  CheckCircle,
  HardDrive,
  Download,
} from "lucide-react";
import { motion } from "framer-motion";

import { setOnboardingCompleted, setOnboardingSkipped } from "../../../core/storage/appState";
import { storageBridge } from "../../../core/storage/files";
import logoSvg from "../../../assets/logo.svg";
import { typography, radius, spacing, interactive, shadows, colors, cn } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import { LocaleSelector } from "../../components/LocaleSelector";

export function WelcomePage() {
  const { locale, setLocale, t } = useI18n();
  const navigate = useNavigate();
  const quickFacts = useQuickFacts();
  const [showSkipWarning, setShowSkipWarning] = useState(false);
  const [showRestoreBackup, setShowRestoreBackup] = useState(false);

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

  const handleAddProvider = () => {
    navigate("/onboarding/provider");
  };

  const handleConfirmSkip = async () => {
    await setOnboardingCompleted(true);
    await setOnboardingSkipped(true);
    // Small delay to ensure state is persisted before navigation
    await new Promise((resolve) => setTimeout(resolve, 100));
    navigate("/");
  };

  const handleRestoreComplete = async () => {
    await setOnboardingCompleted(true);
    navigate("/chat");
  };

  return (
    <div
      className={cn("flex min-h-screen flex-col text-gray-200", colors.effects.gradient.surface)}
    >
      {/* Desktop: Split layout | Mobile: Stacked layout */}
      <div className="flex flex-1 flex-col lg:flex-row items-center justify-center px-4 py-12 lg:px-16 lg:gap-16 xl:gap-24">
        {/* Left Side - Branding (desktop) / Top (mobile) */}
        <motion.div
          className="flex flex-col items-center lg:items-start lg:flex-1 lg:max-w-lg"
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.5, ease: "easeOut" }}
        >
          {/* Logo Section */}
          <div className="relative mb-8 overflow-visible">
            {/* Glow effect - larger spread for smoother edges */}
            <div
              className={cn(
                "absolute -inset-8 rounded-full blur-3xl opacity-60 animate-pulse",
                colors.effects.gradient.brand,
              )}
            />

            {/* Logo container */}
            <div
              className={cn(
                "relative flex h-24 w-24 lg:h-32 lg:w-32 items-center justify-center",
                colors.glass.default,
                radius.full,
                shadows.xl,
              )}
            >
              <img
                src={logoSvg}
                alt={t("onboarding.welcome.appName")}
                className="h-14 w-14 lg:h-20 lg:w-20"
              />
            </div>
          </div>

          {/* Brand name */}
          <h1
            className={cn(
              typography.display.size,
              typography.display.weight,
              "mb-3 lg:text-5xl",
              "text-center lg:text-left",
              colors.effects.gradient.text,
            )}
          >
            {t("onboarding.welcome.appName")}
          </h1>

          {/* Tagline */}
          <p
            className={cn(
              typography.body.size,
              typography.body.lineHeight,
              "max-w-70lg:max-w-md lg:text-lg",
              "text-center lg:text-left text-white/60",
            )}
          >
            {t("onboarding.welcome.tagline")}
          </p>

          {/* Feature Pills */}
          <div
            className={cn(
              "mt-6 flex items-center flex-wrap gap-2 lg:gap-3",
              "justify-center lg:justify-start",
            )}
          >
            {quickFacts.map(({ icon: Icon, label }) => (
              <div
                key={label}
                className={cn(
                  "flex items-center gap-1.5 border border-white/10 bg-white/5 px-3 py-1.5 lg:px-4 lg:py-2 backdrop-blur-sm",
                  radius.full,
                )}
              >
                <Icon size={14} className="text-emerald-400 lg:w-4 lg:h-4" strokeWidth={2.5} />
                <span
                  className={cn(
                    typography.bodySmall.size,
                    typography.label.weight,
                    "text-white/70 lg:text-sm",
                  )}
                >
                  {label}
                </span>
              </div>
            ))}
          </div>

          {/* Language Selector */}
          <motion.div
            className="mt-6 w-full max-w-xs lg:max-w-sm"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.4, delay: 0.3 }}
          >
            <LocaleSelector
              value={locale}
              onChange={setLocale}
              label={t("onboarding.welcome.languageSelector.title")}
              description={t("onboarding.welcome.languageSelector.description")}
              title={t("components.localeSelector.title")}
              labelClassName={cn(typography.caption.size, "font-medium text-white/70")}
              descriptionClassName={cn(
                "text-center lg:text-left",
                typography.caption.size,
                "text-white/35",
              )}
              triggerClassName={cn(
                "border-white/10 bg-white/5 text-white backdrop-blur-sm",
                "hover:border-white/20 hover:bg-white/8",
                "focus:border-emerald-400/50 focus:ring-1 focus:ring-emerald-400/30",
              )}
              menuClassName="bg-[#0f1014]"
            />
          </motion.div>

          {/* Desktop-only: Bottom hint on left side */}
          <motion.p
            className={cn("mt-8 hidden lg:block", typography.caption.size, "text-white/40")}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.4, delay: 0.5 }}
          >
            {t("onboarding.welcome.setupTime")}
          </motion.p>
        </motion.div>

        {/* Right Side - Actions (desktop) / Bottom (mobile) */}
        <motion.div
          className="flex flex-col items-center lg:items-stretch w-full max-w-xs lg:max-w-sm lg:flex-1 mt-8 lg:mt-0"
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.4, delay: 0.2 }}
        >
          {/* CTA Buttons */}
          <div className={cn("w-full", spacing.field)}>
            <button
              className={cn(
                "group w-full flex items-center justify-center gap-2 px-6 py-4",
                radius.md,
                "border border-emerald-400/40 bg-emerald-400/20 text-emerald-100",
                typography.body.size,
                typography.h3.weight,
                shadows.glow,
                interactive.transition.default,
                interactive.active.scale,
                "hover:border-emerald-400/60 hover:bg-emerald-400/30",
              )}
              onClick={handleAddProvider}
            >
              <span>{t("onboarding.welcome.getStarted")}</span>
              <ArrowRight
                size={18}
                className="transition-transform group-hover:translate-x-0.5"
                strokeWidth={2.5}
              />
            </button>

            <button
              className={cn(
                "w-full px-6 py-3",
                radius.md,
                "border border-white/10 bg-white/5 text-white/60",
                typography.body.size,
                interactive.transition.default,
                interactive.active.scale,
                "hover:border-white/20 hover:bg-white/8 hover:text-white/80",
              )}
              onClick={() => setShowSkipWarning(true)}
            >
              {t("onboarding.welcome.skipForNow")}
            </button>

            <button
              className={cn(
                "w-full flex items-center justify-center gap-2 px-6 py-3",
                radius.md,
                "border border-white/10 bg-white/5 text-white/60",
                typography.body.size,
                interactive.transition.default,
                interactive.active.scale,
                "hover:border-white/20 hover:bg-white/8 hover:text-white/80",
              )}
              onClick={() => setShowRestoreBackup(true)}
            >
              <Upload size={16} />
              {t("onboarding.welcome.restoreFromBackup")}
            </button>
          </div>

          {/* Mobile-only: Bottom hint */}
          <motion.p
            className={cn("mt-8 text-center lg:hidden", typography.caption.size, "text-white/40")}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.4, delay: 0.5 }}
          >
            {t("onboarding.welcome.setupTime")}
          </motion.p>
        </motion.div>
      </div>

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

const useQuickFacts = () => {
  const { t } = useI18n();
  return [
    { icon: ShieldCheck, label: t("onboarding.welcome.features.onDevice") },
    { icon: Sparkles, label: t("onboarding.welcome.features.characterReady") },
  ];
};

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
            "flex items-start gap-3 border border-amber-400/20 bg-amber-400/5 p-4 mb-6",
            radius.md,
          )}
        >
          <div
            className={cn(
              "flex h-10 w-10 shrink-0 items-center justify-center bg-amber-400/20 text-amber-300",
              radius.md,
            )}
          >
            <AlertTriangle size={20} strokeWidth={2.5} />
          </div>
          <div className={spacing.tight}>
            <h4 className={cn(typography.body.size, typography.h3.weight, "text-white")}>
              {t("onboarding.welcome.skipWarning.warningTitle")}
            </h4>
            <p
              className={cn(
                typography.bodySmall.size,
                typography.bodySmall.lineHeight,
                "text-white/60",
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

  // Load backups on mount
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
                  className="text-xs font-medium text-blue-400 hover:text-blue-300"
                >
                  {t("onboarding.welcome.restoreBackup.browse")}
                </button>
              </div>

              {/* Error display for list view */}
              {error && (
                <div
                  className={cn(
                    "flex items-start gap-2 border border-red-400/30 bg-red-400/10 px-3 py-2 text-sm text-red-200 mb-4",
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
                  <p className="mt-2 text-sm text-white/40">{t("onboarding.welcome.restoreBackup.processing")}</p>
                  <p className="text-xs text-white/20 mt-1">{t("onboarding.welcome.restoreBackup.processingNote")}</p>
                  <button
                    onClick={() => setLoading(false)}
                    className="mt-6 text-xs text-red-400/60 hover:text-red-300 transition-colors"
                  >
                    {t("onboarding.welcome.restoreBackup.cancel")}
                  </button>
                </div>
              ) : backups.length === 0 ? (
                <div className={cn("border border-white/10 bg-white/5 p-6 text-center", radius.md)}>
                  <FileArchive className="mx-auto h-8 w-8 text-white/20" />
                  <p className="mt-3 text-sm text-white/40">{t("onboarding.welcome.restoreBackup.noBackups")}</p>
                  <p className="mt-1 text-xs text-white/30">{t("onboarding.welcome.restoreBackup.noBackupsHint")}</p>
                  <button
                    onClick={handleBrowseForBackup}
                    className={cn(
                      "mt-4 inline-flex items-center gap-2 px-4 py-2 rounded-lg",
                      "border border-blue-400/30 bg-blue-400/10",
                      "text-sm text-blue-300 font-medium",
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
                            <p className="truncate text-sm font-medium text-white">
                              {backup.filename}
                            </p>
                            {backup.encrypted && (
                              <Lock className="h-3 w-3 shrink-0 text-amber-400/70" />
                            )}
                          </div>
                          <p className="mt-0.5 text-[11px] text-white/40">
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
                    <p className="truncate text-sm font-medium text-white">
                      {selectedBackup.filename}
                    </p>
                    <p className="text-xs text-white/40">
                      {formatDate(selectedBackup.createdAt)} · v{selectedBackup.appVersion}
                    </p>
                  </div>
                </div>
              </div>

              {/* Info notice */}
              <div
                className={cn(
                  "flex items-start gap-2 border border-blue-400/30 bg-blue-400/10 px-3 py-2 text-xs text-blue-200",
                  radius.md,
                )}
              >
                <CheckCircle className="h-4 w-4 shrink-0 mt-0.5" />
                <span>{t("onboarding.welcome.restoreBackup.infoMessage")}</span>
              </div>

              {error && (
                <div
                  className={cn(
                    "flex items-center gap-2 border border-red-400/30 bg-red-400/10 px-3 py-2 text-sm text-red-200",
                    radius.md,
                  )}
                >
                  <AlertTriangle className="h-4 w-4 shrink-0" />
                  {error}
                </div>
              )}

              {selectedBackup.encrypted && (
                <div>
                  <label className="mb-1.5 block text-xs font-medium text-white/50">
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

      {/* Dynamic Memory Model Required Modal */}
      {showEmbeddingPrompt && (
        <motion.div
          className="absolute inset-0 z-10 flex items-end justify-center bg-black/60"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          onClick={(e) => e.stopPropagation()}
        >
          <motion.div
            className={cn(
              "w-full max-w-lg border border-white/10 bg-[#0b0b0d] p-6",
              "rounded-t-3xl",
              shadows.xl,
            )}
            initial={{ y: "100%" }}
            animate={{ y: 0 }}
            transition={{ type: "spring", damping: 30, stiffness: 350 }}
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className={cn(typography.h2.size, typography.h2.weight, "text-white mb-4")}>
              {t("onboarding.welcome.restoreBackup.embeddingTitle")}
            </h3>

            <div className="space-y-4">
              <div className="flex items-start gap-3 rounded-xl border border-amber-400/20 bg-amber-400/10 p-3">
                <HardDrive className="h-5 w-5 shrink-0 text-amber-400 mt-0.5" />
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-medium text-amber-200">{t("onboarding.welcome.restoreBackup.dynamicMemoryDetected")}</p>
                  <p className="mt-1 text-xs text-amber-200/70">
                    {t("onboarding.welcome.restoreBackup.dynamicMemoryMessage")}
                  </p>
                </div>
              </div>

              <p className="text-sm text-white/60">
                {t("onboarding.welcome.restoreBackup.embeddingOptions")}
              </p>

              <div className="flex flex-col gap-2 pt-2">
                <button
                  onClick={handleDownloadModel}
                  className={cn(
                    "flex items-center justify-center gap-2 px-6 py-3",
                    radius.md,
                    "border border-blue-400/40 bg-blue-400/20 text-blue-100",
                    typography.body.size,
                    typography.h3.weight,
                    interactive.transition.fast,
                    "hover:border-blue-400/60 hover:bg-blue-400/30",
                  )}
                >
                  <Download className="h-4 w-4" />
                  {t("onboarding.welcome.restoreBackup.downloadModel")}
                </button>
                <button
                  onClick={handleDisableAndContinue}
                  className={cn(
                    "px-6 py-3",
                    radius.md,
                    "border border-white/10 bg-white/5 text-white/60",
                    typography.body.size,
                    interactive.transition.fast,
                    "hover:border-white/20 hover:bg-white/10 hover:text-white",
                  )}
                >
                  {t("onboarding.welcome.restoreBackup.continueWithoutDynamic")}
                </button>
              </div>

              <p className="text-xs text-white/40 text-center">
                {t("onboarding.welcome.restoreBackup.embeddingNote")}
              </p>
            </div>
          </motion.div>
        </motion.div>
      )}
    </motion.div>
  );
}
