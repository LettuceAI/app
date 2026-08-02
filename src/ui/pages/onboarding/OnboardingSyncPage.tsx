import { useState, useEffect, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { type as osType } from "@tauri-apps/plugin-os";
import {
  scan,
  cancel,
  checkPermissions,
  requestPermissions,
  Format,
} from "@tauri-apps/plugin-barcode-scanner";
import {
  AlertTriangle,
  ArrowLeft,
  CheckCircle,
  Loader2,
  Scan,
  Smartphone,
  Wifi,
  X,
} from "lucide-react";

import { cn, interactive, radius, shadows, typography } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import { setOnboardingCompleted } from "../../../core/storage/appState";
import { storageBridge } from "../../../core/storage/files";
import { MissingModelRequirementsSheet } from "../../components/MissingModelRequirementsSheet";
import { SyncStageProgress } from "../../components/SyncStageProgress";
import {
  buildModelRequirementsQueuePath,
  getPostSyncMissingModelRequirementsSettled,
  type ModelRequirement,
} from "../../modelRequirements";

type DomainProgress = {
  domain: string;
  items_done: number;
  items_total: number;
};

type SyncingDetails = {
  phase: string;
  progress: number | null;
  current_domain?: string | null;
  items_done?: number | null;
  items_total?: number | null;
  bytes_done?: number | null;
  bytes_total?: number | null;
  domain_progress?: DomainProgress[];
  database_wait_attempt?: number | null;
  database_wait_total?: number | null;
  database_wait_ms?: number | null;
};

type SyncStatus =
  | { status: "Idle" }
  | { status: "Sharing"; details: { ip: string; port: number; pin: string; clients: number } }
  | { status: "PendingApproval"; details: { ip: string; device_name: string } }
  | { status: "PendingStart"; details: { ip: string; device_name: string } }
  | { status: "Connecting" }
  | { status: "WaitingForApproval"; details: { peer: string } }
  | { status: "Syncing"; details: SyncingDetails }
  | { status: "Completed" }
  | { status: "Error"; details: { message: string } };

const COMPLETION_DESTINATION = "/chat";

function formatBytes(n: number): string {
  if (!Number.isFinite(n) || n <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 && i > 0 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

function getProgressRatio(details: SyncingDetails | null): number | null {
  if (!details) return null;
  if (typeof details.progress === "number" && Number.isFinite(details.progress)) {
    return Math.min(1, Math.max(0, details.progress));
  }
  if (typeof details.bytes_total === "number" && details.bytes_total > 0) {
    return Math.min(1, Math.max(0, (details.bytes_done ?? 0) / details.bytes_total));
  }
  if (typeof details.items_total === "number" && details.items_total > 0) {
    return Math.min(1, Math.max(0, (details.items_done ?? 0) / details.items_total));
  }
  return null;
}

export function OnboardingSyncStep() {
  const { t } = useI18n();
  const navigate = useNavigate();

  const [status, setStatus] = useState<SyncStatus>({ status: "Idle" });
  const [hostIp, setHostIp] = useState("");
  const [pin, setPin] = useState("");
  const [isConnecting, setIsConnecting] = useState(false);
  const [isMobile, setIsMobile] = useState(false);
  const [isScanning, setIsScanning] = useState(false);
  const [completing, setCompleting] = useState(false);
  const [missingRequirements, setMissingRequirements] = useState<ModelRequirement[]>([]);
  const [sawSyncing, setSawSyncing] = useState(false);
  const [rejected, setRejected] = useState(false);
  const handledCompletionRef = useRef(false);

  useEffect(() => {
    try {
      const type = osType();
      setIsMobile(type === "android" || type === "ios");
    } catch (e) {
      console.error("Failed to detect OS", e);
    }
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    invoke<SyncStatus>("get_sync_status").then(setStatus).catch(console.error);
    void listen<SyncStatus>("sync-status-changed", (event) => {
      setStatus(event.payload);
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(console.error);
    return () => {
      if (unlisten) unlisten();
      invoke("stop_sync").catch(console.error);
    };
  }, []);

  useEffect(() => {
    if (status.status !== "Idle" && status.status !== "Error") setIsConnecting(false);
    if (status.status === "Syncing") setSawSyncing(true);
  }, [status]);

  useEffect(() => {
    if (status.status !== "Completed") {
      handledCompletionRef.current = false;
      return;
    }

    if (handledCompletionRef.current) return;
    handledCompletionRef.current = true;

    if (!sawSyncing) {
      setRejected(true);
      invoke("stop_sync").catch(console.error);
      return;
    }

    let cancelled = false;

    const finish = async () => {
      setCompleting(true);
      try {
        const missing = await getPostSyncMissingModelRequirementsSettled();
        if (missing.length > 0) {
          if (!cancelled) {
            setCompleting(false);
            setMissingRequirements(missing);
          }
          return;
        }

        void setOnboardingCompleted(true).catch((e) => {
          console.error("Failed to persist onboarding completion after sync", e);
        });

        if (!cancelled) navigate(COMPLETION_DESTINATION);
      } catch (e) {
        console.error("Failed to finalize sync onboarding", e);
        if (!cancelled) navigate(COMPLETION_DESTINATION);
      }
    };

    void finish();
    return () => {
      cancelled = true;
    };
  }, [status.status, sawSyncing, navigate]);

  const handleConnect = async (override?: string) => {
    setIsConnecting(true);
    try {
      let ipToUse = override ?? hostIp;
      let pinToUse = pin;
      let portToUse = 8000;

      try {
        const data = JSON.parse(ipToUse);
        if (data.ip && data.port && data.pin) {
          ipToUse = data.ip;
          portToUse = data.port;
          pinToUse = data.pin;
          setHostIp(`${ipToUse}:${portToUse}`);
          setPin(pinToUse);
        }
      } catch {
        if (ipToUse.includes(":")) {
          const parts = ipToUse.split(":");
          ipToUse = parts[0];
          portToUse = parseInt(parts[1]);
        }
      }

      await invoke("connect_sync_device", { ip: ipToUse, port: portToUse, pin: pinToUse });
    } catch (e) {
      console.error("Failed to connect", e);
      setIsConnecting(false);
    }
  };

  const startScan = async () => {
    try {
      const permission = await checkPermissions();
      if (permission === "prompt") {
        const result = await requestPermissions();
        if (result !== "granted") return;
      } else if (permission === "denied") {
        return;
      }
      setIsScanning(true);
      const scanned = await scan({ formats: [Format.QRCode] });
      if (scanned.content) {
        void handleConnect(scanned.content);
      }
    } catch (e) {
      console.error("Scan failed", e);
    } finally {
      setIsScanning(false);
    }
  };

  const stopScan = async () => {
    try {
      await cancel();
    } catch (e) {
      console.error("Failed to cancel scan", e);
    }
    setIsScanning(false);
  };

  const handleBack = async () => {
    try {
      await invoke("stop_sync");
    } catch (e) {
      console.error("Failed to stop sync", e);
    }
    navigate("/welcome");
  };

  const handleDownloadEmbedding = () => {
    const queued = [...missingRequirements];
    setMissingRequirements([]);
    void setOnboardingCompleted(true).catch((e) => {
      console.error("Failed to persist onboarding completion before model download", e);
    });
    navigate(buildModelRequirementsQueuePath(queued, COMPLETION_DESTINATION));
  };

  const handleContinueWithoutEmbedding = async () => {
    const needsEmbedding = missingRequirements.some((requirement) => requirement.kind === "embedding");
    setMissingRequirements([]);
    if (needsEmbedding) {
      try {
        await storageBridge.backupDisableDynamicMemory();
      } catch (e) {
        console.error("Failed to disable dynamic memory", e);
      }
    }
    void setOnboardingCompleted(true).catch((e) => {
      console.error("Failed to persist onboarding completion after skipping model download", e);
    });
    navigate(COMPLETION_DESTINATION);
  };

  const isIdle = status.status === "Idle";
  const isError = status.status === "Error";
  const isConnectionInProgress = status.status === "Connecting";
  const isWaitingForApproval = status.status === "WaitingForApproval";
  const isSyncing = status.status === "Syncing";
  const isCompleted = status.status === "Completed" && !rejected;

  const syncingDetails = isSyncing
    ? (status as Extract<SyncStatus, { status: "Syncing" }>).details
    : null;
  const ratio = getProgressRatio(syncingDetails);
  const percent = ratio !== null ? Math.round(ratio * 100) : null;

  return (
    <div className="relative flex flex-1 flex-col text-gray-200">
      {/* Header — inline, transparent so the shared onboarding background shows through */}
      <div className="relative z-30 flex items-center justify-between px-4 py-4 pt-[calc(env(safe-area-inset-top)+16px)] lg:px-8 lg:py-6">
        <button
          onClick={() => void handleBack()}
          disabled={isSyncing || completing}
          className={cn(
            "flex h-10 w-10 items-center justify-center rounded-full border border-white/15 bg-white/8 text-white",
            interactive.transition.default,
            "hover:border-white/30 hover:bg-white/15 active:scale-[0.98]",
            "disabled:opacity-40 disabled:pointer-events-none",
            "lg:h-11 lg:w-11",
          )}
          aria-label={t("common.buttons.cancel")}
        >
          <ArrowLeft size={16} />
        </button>
        <div className="w-10 lg:w-11" />
      </div>

      <main className="relative z-10 flex flex-1 flex-col items-center px-4 pb-10 lg:px-8">
        <div className="w-full max-w-md space-y-5">
          {/* Intro */}
          <div className="text-center">
            <div className="mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-2xl border border-emerald-400/30 bg-emerald-400/10 text-emerald-300">
              <Smartphone className="h-6 w-6" />
            </div>
            <h1
              className={cn(
                typography.h1.size,
                typography.h1.weight,
                "text-white tracking-tight",
              )}
            >
              {t("onboarding.syncOnboarding.title")}
            </h1>
            <p className="mt-2 text-[14px] leading-relaxed text-white/80">
              {t("onboarding.syncOnboarding.subtitle")}
            </p>
          </div>

          {isError && (
            <div className="flex items-center gap-3 rounded-xl border border-red-400/25 bg-red-400/10 p-3">
              <AlertTriangle className="h-5 w-5 shrink-0 text-red-300" />
              <div className="min-w-0 flex-1">
                <p className="text-sm font-medium text-red-200">{t("sync.messages.error")}</p>
                <p className="text-xs text-red-200/70">
                  {(status as Extract<SyncStatus, { status: "Error" }>).details.message}
                </p>
              </div>
            </div>
          )}

          {(isIdle || isError) && (
            <div className="space-y-3">
              {isMobile && (
                <button
                  onClick={() => void startScan()}
                  className={cn(
                    "flex w-full items-center justify-center gap-2 rounded-xl border border-white/25 bg-black/55 px-4 py-3 text-sm font-medium text-white backdrop-blur-md",
                    interactive.transition.default,
                    interactive.active.scale,
                    "hover:border-white/40 hover:bg-black/65",
                  )}
                >
                  <Scan className="h-4 w-4" /> {t("sync.buttons.scanQRCode")}
                </button>
              )}

              <div
                className={cn(
                  "space-y-3 rounded-2xl border border-white/15 bg-black/60 p-4 backdrop-blur-md",
                  shadows.lg,
                )}
              >
                <div>
                  <label className="mb-1.5 block text-[11px] font-medium uppercase tracking-wider text-white/70">
                    {t("sync.fields.hostAddress")}
                  </label>
                  <input
                    type="text"
                    value={hostIp}
                    onChange={(e) => setHostIp(e.target.value)}
                    placeholder={t("sync.fields.hostPlaceholder")}
                    className={cn(
                      "w-full rounded-lg border border-white/20 bg-black/70 px-4 py-3 font-mono text-sm text-white placeholder-white/40",
                      interactive.transition.fast,
                      "focus:border-emerald-400/60 focus:outline-none focus:ring-1 focus:ring-emerald-400/30",
                    )}
                  />
                </div>
                <div>
                  <label className="mb-1.5 block text-[11px] font-medium uppercase tracking-wider text-white/70">
                    {t("sync.fields.pinCode")}
                  </label>
                  <input
                    type="text"
                    value={pin}
                    onChange={(e) => setPin(e.target.value.replace(/\D/g, "").slice(0, 6))}
                    placeholder={t("sync.fields.pinPlaceholder")}
                    maxLength={6}
                    inputMode="numeric"
                    className={cn(
                      "w-full rounded-lg border border-white/20 bg-black/70 px-4 py-3 text-center font-mono text-xl text-white placeholder-white/35",
                      pin ? "tracking-[0.3em]" : "tracking-normal",
                      interactive.transition.fast,
                      "focus:border-emerald-400/60 focus:outline-none focus:ring-1 focus:ring-emerald-400/30",
                    )}
                  />
                </div>
              </div>

              <button
                onClick={() => void handleConnect()}
                disabled={
                  !hostIp ||
                  (pin.length !== 6 && !hostIp.trim().startsWith("{")) ||
                  isConnecting
                }
                className={cn(
                  "flex w-full items-center justify-center gap-2 rounded-xl border border-emerald-400/60 bg-emerald-500/35 px-4 py-3 text-sm font-semibold text-white shadow-[0_8px_24px_-8px_rgba(16,185,129,0.45)] backdrop-blur-md",
                  interactive.transition.default,
                  interactive.active.scale,
                  "hover:border-emerald-300/80 hover:bg-emerald-500/45 disabled:cursor-not-allowed disabled:opacity-40",
                )}
              >
                {isConnecting ? (
                  <>
                    <Loader2 className="h-4 w-4 animate-spin" />
                    {t("sync.buttons.connecting")}
                  </>
                ) : (
                  <>
                    <Wifi className="h-4 w-4" /> {t("sync.buttons.connect")}
                  </>
                )}
              </button>

              <p className="px-1 pt-1 text-center text-[12px] text-white/65">
                {t("onboarding.syncOnboarding.hostHint")}
              </p>
            </div>
          )}

          {(isConnectionInProgress ||
            isWaitingForApproval ||
            isSyncing ||
            isCompleted) && (
            <div
              className={cn(
                "overflow-hidden rounded-2xl border backdrop-blur-md",
                isCompleted
                  ? "border-emerald-400/40 bg-emerald-500/15"
                  : "border-white/15 bg-black/60",
              )}
            >
              <div className="flex items-center gap-3 px-4 pt-4 pb-3">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    {!isCompleted && (
                      <Loader2 className="h-3.5 w-3.5 shrink-0 animate-spin text-white/65" />
                    )}
                    {isCompleted && (
                      <CheckCircle className="h-4 w-4 shrink-0 text-emerald-300" />
                    )}
                    <p className="text-[13px] font-semibold text-white">
                      {isCompleted
                        ? t("sync.messages.completed")
                        : isConnectionInProgress
                          ? t("sync.status.connecting")
                          : isWaitingForApproval
                            ? t("sync.status.waitingConfirmation")
                            : t("sync.status.syncing")}
                    </p>
                  </div>
                  {isWaitingForApproval && (
                    <p className="mt-0.5 truncate text-[11px] leading-relaxed text-white/50">
                      {t("sync.status.waitingConfirmationDesc")}
                    </p>
                  )}
                  {isCompleted && (
                    <p className="mt-0.5 truncate text-[11px] leading-relaxed text-emerald-200/70">
                      {t("sync.messages.completedDesc")}
                    </p>
                  )}
                </div>
                {percent !== null && isSyncing && (
                  <div className="shrink-0 text-right tabular-nums">
                    <p className="font-mono text-2xl font-semibold leading-none text-white/85">
                      {percent}
                      <span className="ml-0.5 text-sm font-normal text-white/35">%</span>
                    </p>
                  </div>
                )}
              </div>

              {isSyncing && (
                <>
                  <div className="px-4 pb-4">
                    <div className="relative h-1 w-full overflow-hidden rounded-full bg-white/8">
                      <div
                        className={cn(
                          "h-full rounded-full bg-emerald-400/80",
                          ratio !== null
                            ? "transition-[width] duration-500 ease-out"
                            : "w-1/4 animate-pulse",
                        )}
                        style={ratio !== null ? { width: `${ratio * 100}%` } : undefined}
                      />
                    </div>
                    {(syncingDetails?.bytes_total ?? 0) > 0 && (
                      <p className="mt-2 text-[11px] tabular-nums text-white/45">
                        {formatBytes(syncingDetails?.bytes_done ?? 0)} /{" "}
                        {formatBytes(syncingDetails?.bytes_total ?? 0)}
                      </p>
                    )}
                  </div>
                  {syncingDetails?.phase && (
                    <SyncStageProgress
                      phase={syncingDetails.phase}
                      surface="onboarding"
                      databaseWaitAttempt={syncingDetails.database_wait_attempt}
                      databaseWaitTotal={syncingDetails.database_wait_total}
                      databaseWaitMs={syncingDetails.database_wait_ms}
                    />
                  )}
                </>
              )}

              {completing && isCompleted && (
                <div className="border-t border-white/8 px-4 py-3 text-center text-[12px] text-white/55">
                  {t("onboarding.syncOnboarding.finalizing")}
                </div>
              )}
            </div>
          )}

          {(isConnectionInProgress || isWaitingForApproval) && (
            <button
              onClick={() => void handleBack()}
              className={cn(
                "w-full rounded-xl border border-white/20 bg-black/55 px-4 py-3 text-sm font-medium text-white/85 backdrop-blur-md",
                interactive.transition.default,
                interactive.active.scale,
                "hover:border-white/35 hover:bg-black/65 hover:text-white",
              )}
            >
              {t("common.buttons.cancel")}
            </button>
          )}
        </div>
      </main>

      {missingRequirements.length > 0 && (
        <MissingModelRequirementsSheet
          isOpen
          title={t("onboarding.syncOnboarding.missingModels.title")}
          description={t("onboarding.syncOnboarding.missingModels.description")}
          missing={missingRequirements}
          onClose={() => void handleContinueWithoutEmbedding()}
          onDownload={handleDownloadEmbedding}
          closeLabel={t("onboarding.syncOnboarding.missingModels.continueForNow")}
          downloadLabel={t("onboarding.syncOnboarding.missingModels.downloadRequired")}
        />
      )}

      {isScanning && (
        <div className="fixed inset-0 z-50 flex flex-col items-center justify-end bg-transparent pb-20">
          <div className="mb-8 text-center">
            <div
              className={cn(
                "relative mx-auto mb-4 h-64 w-64 overflow-hidden border-2 border-white/50",
                radius.lg,
              )}
            >
              <div
                className={cn(
                  "absolute inset-0 animate-pulse border-2 border-white/20",
                  radius.lg,
                )}
              />
              <div className="absolute left-0 top-0 h-1 w-full animate-[onbsync-scan_2s_infinite] bg-white/70 shadow-[0_0_10px_currentColor]" />
            </div>
            <p className="font-medium text-white">{t("sync.scanner.title")}</p>
          </div>
          <button
            onClick={() => void stopScan()}
            className={cn(
              "flex items-center gap-2 rounded-full border border-white/20 bg-white/10 px-6 py-3 font-medium text-white backdrop-blur-md",
              interactive.transition.default,
              interactive.active.scale,
            )}
          >
            <X className="h-5 w-5" /> {t("sync.scanner.cancel")}
          </button>
          <style>{`
            @keyframes onbsync-scan {
              0% { transform: translateY(0); opacity: 0.5; }
              50% { opacity: 1; }
              100% { transform: translateY(256px); opacity: 0.5; }
            }
          `}</style>
        </div>
      )}
    </div>
  );
}
