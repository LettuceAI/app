import { useState, useEffect, useRef, type ComponentType, type SVGProps, type ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import QRCodeImport from "react-qr-code";
import {
  Smartphone,
  Monitor,
  Loader2,
  CheckCircle,
  AlertTriangle,
  Wifi,
  Copy,
  Check,
  HelpCircle,
  X,
  RefreshCw,
  Scan,
  Users,
  Radio,
  ArrowUp,
  ArrowDown,
  ArrowLeft,
  ArrowRight,
} from "lucide-react";
import { type as osType } from "@tauri-apps/plugin-os";
import {
  scan,
  cancel,
  checkPermissions,
  requestPermissions,
  Format,
} from "@tauri-apps/plugin-barcode-scanner";
import { cn, typography, spacing, interactive, radius } from "../../design-tokens";
import { BottomMenu, MenuButton } from "../../components/BottomMenu";
import { useI18n } from "../../../core/i18n/context";
import { storageBridge } from "../../../core/storage/files";
import { MissingModelRequirementsSheet } from "../../components/MissingModelRequirementsSheet";
import { SyncStageProgress } from "../../components/SyncStageProgress";
import { SyncConflictInbox } from "../../components/SyncConflictInbox";
import {
  buildModelRequirementsQueuePath,
  getPostSyncMissingModelRequirementsSettled,
  type ModelRequirement,
} from "../../modelRequirements";

type QRCodeComponentProps = SVGProps<SVGSVGElement> & {
  value: string;
  size?: number;
  bgColor?: string;
  fgColor?: string;
  level?: "L" | "M" | "H" | "Q";
  title?: string;
};

const QRCodeComponent =
  (
    QRCodeImport as unknown as {
      default?: ComponentType<QRCodeComponentProps>;
      QRCode?: ComponentType<QRCodeComponentProps>;
    }
  ).default ??
  (
    QRCodeImport as unknown as {
      default?: ComponentType<QRCodeComponentProps>;
      QRCode?: ComponentType<QRCodeComponentProps>;
    }
  ).QRCode ??
  (QRCodeImport as unknown as ComponentType<QRCodeComponentProps>);

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
  items_sent?: number | null;
  items_received?: number | null;
  bytes_sent?: number | null;
  bytes_received?: number | null;
  conflicts_detected?: number | null;
  branches_created?: number | null;
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

function getSyncProgress(details: SyncingDetails | null): { ratio: number | null; percent: number | null } {
  if (!details) return { ratio: null, percent: null };

  if (typeof details.progress === "number" && Number.isFinite(details.progress)) {
    const ratio = Math.min(1, Math.max(0, details.progress));
    return { ratio, percent: Math.round(ratio * 100) };
  }

  if (typeof details.bytes_total === "number" && details.bytes_total > 0) {
    const ratio = Math.min(1, Math.max(0, (details.bytes_done ?? 0) / details.bytes_total));
    return { ratio, percent: Math.round(ratio * 100) };
  }

  if (typeof details.items_total === "number" && details.items_total > 0) {
    const ratio = Math.min(1, Math.max(0, (details.items_done ?? 0) / details.items_total));
    return { ratio, percent: Math.round(ratio * 100) };
  }

  return { ratio: null, percent: null };
}

function formatProgressPercent(ratio: number | null): string | null {
  if (ratio === null) return null;
  const normalized = Math.min(1, Math.max(0, ratio)) * 100;
  if (normalized > 0 && normalized < 10) return normalized.toFixed(1);
  return Math.round(normalized).toString();
}

function SectionHeader({ title, icon, right }: { title: string; icon?: ReactNode; right?: ReactNode }) {
  return (
    <div className="mb-2.5 flex items-center justify-between gap-2 px-1">
      <div className="flex items-center gap-2">
        {icon && <span className="text-fg/30">{icon}</span>}
        <h2
          className={cn(
            typography.overline.size,
            typography.overline.weight,
            typography.overline.tracking,
            typography.overline.transform,
            "text-fg/40",
          )}
        >
          {title}
        </h2>
      </div>
      {right}
    </div>
  );
}

interface ModeTileProps {
  active: boolean;
  icon: ReactNode;
  title: string;
  description: string;
  tone: "info" | "accent";
  onClick: () => void;
}

function ModeTile({ active, icon, title, description, tone, onClick }: ModeTileProps) {
  const toneMap = {
    info: {
      border: "border-info/25",
      bg: "bg-info/8",
      iconBorder: "border-info/40",
      iconBg: "bg-info/15",
      iconText: "text-info/90",
    },
    accent: {
      border: "border-accent/25",
      bg: "bg-accent/8",
      iconBorder: "border-accent/40",
      iconBg: "bg-accent/15",
      iconText: "text-accent/90",
    },
  } as const;
  const s = toneMap[tone];

  return (
    <button
      onClick={onClick}
      className={cn(
        "group flex-1 rounded-xl border px-4 py-3.5 text-left",
        interactive.transition.default,
        interactive.active.scale,
        interactive.focus.ring,
        active ? `${s.border} ${s.bg}` : "border-fg/10 bg-fg/5 hover:border-fg/20",
      )}
    >
      <div className="flex items-center gap-3">
        <div
          className={cn(
            "flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border",
            interactive.transition.default,
            active ? `${s.iconBorder} ${s.iconBg} ${s.iconText}` : "border-fg/15 bg-fg/8 text-fg/60",
          )}
        >
          {icon}
        </div>
        <div className="min-w-0 flex-1">
          <p className={cn(typography.body.size, "font-medium text-fg")}>{title}</p>
          <p className="mt-0.5 text-[11px] leading-relaxed text-fg/50">{description}</p>
        </div>
      </div>
    </button>
  );
}

function Card({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div className={cn("rounded-xl border border-fg/10 bg-fg/5", className)}>{children}</div>
  );
}

function TransferRail({ reverse = false }: { reverse?: boolean }) {
  return (
    <span className="relative h-[3px] min-w-0 flex-1 overflow-hidden rounded-full bg-fg/10">
      <span
        className={cn(
          "absolute inset-y-0 left-0 w-1/3 rounded-full bg-gradient-to-r from-transparent via-info/80 to-transparent",
          reverse
            ? "animate-[indeterminate_1.8s_linear_infinite_reverse]"
            : "animate-[indeterminate_1.8s_linear_infinite]",
        )}
      />
    </span>
  );
}

function BidirectionalLink() {
  const { t } = useI18n();

  return (
    <div className="flex items-center justify-center gap-4 rounded-xl border border-info/20 bg-info/[0.06] px-4 py-3.5">
      <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border border-fg/10 bg-fg/5 text-fg/70">
        <Monitor className="h-4 w-4" />
      </div>

      <div className="flex w-full max-w-[200px] flex-col items-center gap-2">
        <div className="flex w-full items-center gap-1.5">
          <TransferRail />
          <ArrowRight className="h-3 w-3 shrink-0 text-info/70" />
        </div>
        <p
          className={cn(
            typography.overline.size,
            typography.overline.weight,
            typography.overline.transform,
            "text-center leading-tight tracking-[0.14em] text-info/70",
          )}
        >
          {t("sync.status.bidirectional")}
        </p>
        <div className="flex w-full items-center gap-1.5">
          <ArrowLeft className="h-3 w-3 shrink-0 text-info/70" />
          <TransferRail reverse />
        </div>
      </div>

      <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border border-fg/10 bg-fg/5 text-fg/70">
        <Smartphone className="h-4 w-4" />
      </div>
    </div>
  );
}

interface SyncingPanelProps {
  title: string;
  subtitle?: string | null;
  details: SyncingDetails | null;
  showSpinner?: boolean;
  showProgressBar?: boolean;
  showCounters?: boolean;
  showBreakdown?: boolean;
}

function SyncingPanel({
  title,
  subtitle,
  details,
  showSpinner = true,
  showProgressBar = true,
  showCounters = true,
  showBreakdown = true,
}: SyncingPanelProps) {
  const { t } = useI18n();
  const itemsDone = details?.items_done ?? null;
  const itemsTotal = details?.items_total ?? null;
  const bytesDone = details?.bytes_done ?? null;
  const bytesTotal = details?.bytes_total ?? null;
  const breakdown = details?.domain_progress ?? [];
  const itemsSent = details?.items_sent ?? null;
  const itemsReceived = details?.items_received ?? null;
  const bytesSent = details?.bytes_sent ?? null;
  const bytesReceived = details?.bytes_received ?? null;
  const hasDirectionalCounters =
    typeof itemsSent === "number" ||
    typeof itemsReceived === "number" ||
    typeof bytesSent === "number" ||
    typeof bytesReceived === "number";
  const { ratio, percent: pct } = getSyncProgress(details);
  const pctLabel = formatProgressPercent(ratio);
  const hasProgress = ratio !== null;
  const hasCounters =
    showCounters &&
    !hasDirectionalCounters &&
    ((typeof itemsTotal === "number" && itemsTotal > 0) ||
      (typeof bytesTotal === "number" && bytesTotal > 0));

  return (
    <div className="overflow-hidden rounded-xl border border-fg/10 bg-fg/[0.035]">
      {}
      <div className="flex items-center gap-3 px-4 pt-4 pb-3">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            {showSpinner && (
              <Loader2 className="h-3.5 w-3.5 shrink-0 animate-spin text-fg/60" />
            )}
            <p className="text-[13px] font-semibold text-fg">{title}</p>
          </div>
          {subtitle && (
            <p className="mt-0.5 truncate text-[11px] leading-relaxed text-fg/45">
              {subtitle}
            </p>
          )}
        </div>
        {pct !== null && pctLabel !== null && (
          <div className="shrink-0 text-right tabular-nums">
            <p className="font-mono text-2xl font-semibold leading-none text-fg/85">
              {pctLabel}
              <span className="ml-0.5 text-sm font-normal text-fg/35">%</span>
            </p>
          </div>
        )}
      </div>

      {}
      {showProgressBar && (
        <div className="px-4 pb-3.5">
          <div className="relative h-1 w-full overflow-hidden rounded-full bg-fg/8">
            <div
              className={cn(
                "h-full rounded-full bg-accent/80",
                hasProgress ? "transition-[width] duration-500 ease-out" : "w-1/4 animate-pulse",
              )}
              style={hasProgress ? { width: `${ratio! * 100}%` } : undefined}
            />
          </div>
        </div>
      )}

      {details?.phase && (
        <SyncStageProgress
          phase={details.phase}
          databaseWaitAttempt={details.database_wait_attempt}
          databaseWaitTotal={details.database_wait_total}
          databaseWaitMs={details.database_wait_ms}
        />
      )}

      {}
      {hasCounters && (
        <div className="grid grid-cols-2 border-t border-fg/8">
          <div className="px-4 py-3">
            <p
              className={cn(
                typography.overline.size,
                typography.overline.weight,
                typography.overline.tracking,
                typography.overline.transform,
                "text-fg/35",
              )}
            >
              {t("sync.panel.items")}
            </p>
            <p className="mt-1 font-mono text-[13px] font-semibold tabular-nums text-fg/85">
              {(itemsDone ?? 0).toLocaleString()}
              {typeof itemsTotal === "number" && itemsTotal > 0 && (
                <span className="text-fg/30"> / {itemsTotal.toLocaleString()}</span>
              )}
            </p>
          </div>
          <div className="border-l border-fg/8 px-4 py-3">
            <p
              className={cn(
                typography.overline.size,
                typography.overline.weight,
                typography.overline.tracking,
                typography.overline.transform,
                "text-fg/35",
              )}
            >
              {t("sync.panel.transferred")}
            </p>
            <p className="mt-1 font-mono text-[13px] font-semibold tabular-nums text-fg/85">
              {formatBytes(bytesDone ?? 0)}
              {typeof bytesTotal === "number" && bytesTotal > 0 && (
                <span className="text-fg/30"> / {formatBytes(bytesTotal)}</span>
              )}
            </p>
          </div>
        </div>
      )}

      {hasDirectionalCounters && (
        <div className="grid grid-cols-2 border-t border-fg/8">
          <div className="px-4 py-3">
            <div className="flex items-center gap-1.5 text-fg/35">
              <ArrowUp className="h-3 w-3" />
              <p className="text-[10px] font-semibold uppercase tracking-wider">
                {t("sync.panel.sent")}
              </p>
            </div>
            <p className="mt-1 font-mono text-[13px] font-semibold tabular-nums text-fg/85">
              {(itemsSent ?? 0).toLocaleString()}
              {typeof bytesSent === "number" && (
                <span className="ml-1 text-fg/30">· {formatBytes(bytesSent)}</span>
              )}
            </p>
          </div>
          <div className="border-l border-fg/8 px-4 py-3">
            <div className="flex items-center gap-1.5 text-fg/35">
              <ArrowDown className="h-3 w-3" />
              <p className="text-[10px] font-semibold uppercase tracking-wider">
                {t("sync.panel.received")}
              </p>
            </div>
            <p className="mt-1 font-mono text-[13px] font-semibold tabular-nums text-fg/85">
              {(itemsReceived ?? 0).toLocaleString()}
              {typeof bytesReceived === "number" && (
                <span className="ml-1 text-fg/30">· {formatBytes(bytesReceived)}</span>
              )}
            </p>
          </div>
        </div>
      )}

      {}
      {showBreakdown && breakdown.length > 0 && (
        <div className="border-t border-fg/8 px-4 py-3">
          <p
            className={cn(
              typography.overline.size,
              typography.overline.weight,
              typography.overline.tracking,
              typography.overline.transform,
              "mb-2 text-fg/35",
            )}
          >
            {t("sync.panel.breakdown")}
          </p>
          <ul className="space-y-1.5">
            {breakdown.map((d) => {
              const done = d.items_done >= d.items_total && d.items_total > 0;
              const active = !done && d.items_done > 0;
              return (
                <li
                  key={d.domain}
                  className="flex items-center gap-3 rounded-md py-1 text-[11px]"
                >
                  <span className="flex h-4 w-4 shrink-0 items-center justify-center">
                    {done ? (
                      <Check className="h-3.5 w-3.5 text-accent/85" />
                    ) : active ? (
                      <Loader2 className="h-3 w-3 animate-spin text-fg/55" />
                    ) : (
                      <span className="h-1.5 w-1.5 rounded-full bg-fg/20" />
                    )}
                  </span>
                  <span
                    className={cn(
                      "min-w-0 flex-1 truncate font-medium",
                      done ? "text-fg/65" : active ? "text-fg/75" : "text-fg/45",
                    )}
                  >
                    {d.domain}
                  </span>
                  <span className="shrink-0 font-mono text-[10px] tabular-nums text-fg/35">
                    {d.items_done.toLocaleString()}
                    <span className="text-fg/25">/{d.items_total.toLocaleString()}</span>
                  </span>
                </li>
              );
            })}
          </ul>
        </div>
      )}
    </div>
  );
}

export function SyncPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [activeTab, setActiveTab] = useState<"host" | "client">("client");
  const [status, setStatus] = useState<SyncStatus>({ status: "Idle" });
  const [hostIp, setHostIp] = useState("");
  const [localIp, setLocalIp] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [pin, setPin] = useState("");
  const [isStartingHost, setIsStartingHost] = useState(false);
  const [isConnectingToHost, setIsConnectingToHost] = useState(false);
  const [role, setRole] = useState<"host" | "client" | null>(null);
  const [isAccepting, setIsAccepting] = useState(false);
  const [isStartingSyncSession, setIsStartingSyncSession] = useState(false);
  const [isMobile, setIsMobile] = useState(false);
  const [missingRequirements, setMissingRequirements] = useState<ModelRequirement[]>([]);
  const handledCompletionRef = useRef(false);

  useEffect(() => {
    const checkMobile = async () => {
      try {
        const type = await osType();
        setIsMobile(type === "android" || type === "ios");
      } catch (e) {
        console.error("Failed to check OS", e);
      }
    };
    checkMobile();
  }, []);

  useEffect(() => {
    const fetchStatus = async () => {
      try {
        const s = await invoke<SyncStatus>("get_sync_status");
        setStatus(s);
      } catch (e) {
        console.error("Failed to get sync status", e);
      }
    };
    let unlisten: (() => void) | null = null;
    fetchStatus();
    void listen<SyncStatus>("sync-status-changed", (event) => {
      setStatus(event.payload);
    }).then((fn) => {
      unlisten = fn;
    }).catch((e) => {
      console.error("Failed to attach sync status listener", e);
    });
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  useEffect(() => {
    if (status.status === "Sharing") setIsStartingHost(false);
    if (status.status !== "Idle" && status.status !== "Error") setIsConnectingToHost(false);
    if (status.status !== "PendingApproval") setIsAccepting(false);
    if (status.status !== "PendingStart") setIsStartingSyncSession(false);
    if (status.status !== "Completed") handledCompletionRef.current = false;
  }, [status]);

  useEffect(() => {
    if (status.status !== "Completed" || handledCompletionRef.current) {
      return;
    }
    handledCompletionRef.current = true;

    let cancelled = false;

    const checkRequirements = async () => {
      try {
        const missing = await getPostSyncMissingModelRequirementsSettled();
        if (missing.length > 0 && !cancelled) {
          setMissingRequirements(missing);
        }
      } catch (e) {
        console.error("Failed to check local model requirements after sync", e);
      }
    };

    void checkRequirements();
    return () => {
      cancelled = true;
    };
  }, [status.status]);

  useEffect(() => {
    invoke<string>("get_local_ip").then(setLocalIp).catch(console.error);
  }, []);

  const startHost = async () => {
    setIsStartingHost(true);
    setRole("host");
    try {
      await invoke("share_sync_device", { port: 0 });
    } catch (e) {
      console.error("Failed to start driver", e);
      setIsStartingHost(false);
      setRole(null);
    }
  };

  const connectToHost = async (overrideIp?: string, overridePin?: string) => {
    setIsConnectingToHost(true);
    setRole("client");
    try {
      let ipToUse = overrideIp || hostIp;
      let pinToUse = overridePin || pin;
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
      setIsConnectingToHost(false);
      setRole(null);
    }
  };

  const stopSync = async () => {
    try {
      await invoke("stop_sync");
      setStatus({ status: "Idle" });
      setMissingRequirements([]);
    } catch (e) {
      console.error("Failed to stop sync", e);
    }
  };

  const handleDownloadRequirements = () => {
    const queued = [...missingRequirements];
    setMissingRequirements([]);
    navigate(buildModelRequirementsQueuePath(queued, "/sync"));
  };

  const handleContinueWithoutRequirements = async () => {
    const needsEmbedding = missingRequirements.some((requirement) => requirement.kind === "embedding");
    setMissingRequirements([]);
    if (needsEmbedding) {
      try {
        await storageBridge.backupDisableDynamicMemory();
      } catch (e) {
        console.error("Failed to disable dynamic memory", e);
      }
    }
  };

  const isIdle = status.status === "Idle";
  const isDriver = status.status === "Sharing";
  const isCompleted = status.status === "Completed";
  const isSyncing = status.status === "Syncing";
  const isError = status.status === "Error";
  const isConnecting = status.status === "Connecting";
  const isWaitingForApproval = status.status === "WaitingForApproval";
  const isPendingApproval = status.status === "PendingApproval";
  const isReadyToStart = status.status === "PendingStart";
  const warningMessage =
    status.status === "Syncing" && (status as any).details?.phase?.startsWith("Warning:")
      ? (status as any).details?.phase
      : null;

  const handleApproval = async (allow: boolean) => {
    if (status.status === "PendingApproval") {
      if (allow) setIsAccepting(true);
      try {
        await invoke("approve_connection", { ip: status.details.ip, allow });
      } catch (e) {
        console.error("Failed to approve connection", e);
        setIsAccepting(false);
      }
    }
  };

  const handleStartSync = async () => {
    if (status.status === "PendingStart") {
      setIsStartingSyncSession(true);
      try {
        await invoke("start_sync_session", { ip: status.details.ip });
      } catch (e) {
        console.error("Failed to start sync session", e);
        setIsStartingSyncSession(false);
      }
    }
  };

  const copyAddress = (ipToCopy: string | null, port: number) => {
    const addr = ipToCopy ? `${ipToCopy}:${port}` : null;
    if (addr) {
      navigator.clipboard.writeText(addr);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  const [isScanning, setIsScanning] = useState(false);

  const stopScan = async () => {
    try {
      await cancel();
      setIsScanning(false);
    } catch (e) {
      console.error("Failed to cancel scan", e);
    }
  };

  const startScan = async () => {
    try {
      const permission = await checkPermissions();
      if (permission === "prompt") {
        const result = await requestPermissions();
        if (result !== "granted") {
          console.error("Camera permission denied");
          return;
        }
      } else if (permission === "denied") {
        console.error("Camera permission denied");
        return;
      }

      setIsScanning(true);
      const scanned = await scan({ formats: [Format.QRCode] });
      if (scanned.content) {
        connectToHost(scanned.content);
      }
    } catch (e) {
      console.error("Scan failed", e);
    } finally {
      setIsScanning(false);
    }
  };

  return (
    <div className="relative flex h-full flex-col pb-16">
      <BottomMenu
        isOpen={isPendingApproval}
        onClose={() => handleApproval(false)}
        title={t("sync.modals.connectionRequest")}
      >
        <div className="mb-6 px-1 text-center">
          <div className="mb-4 flex justify-center">
            <div className="flex h-12 w-12 items-center justify-center rounded-xl border border-info/30 bg-info/15">
              <Smartphone className="h-8 w-8 text-info/90" />
            </div>
          </div>
          <h4 className="mb-1 text-lg font-medium text-fg">
            {(status as any).details?.device_name || t("sync.unknownDevice")}
          </h4>
          <p className="mb-2 font-mono text-sm text-fg/50">{(status as any).details?.ip}</p>
          <p className="text-sm text-fg/40">{t("sync.modals.requestMessage")}</p>
        </div>

        <div className="space-y-3">
          <MenuButton
            icon={Check}
            title={t("sync.modals.acceptConnection")}
            description={t("sync.modals.acceptDesc")}
            color="from-emerald-500 to-emerald-600"
            onClick={() => handleApproval(true)}
            loading={isAccepting}
          />
          <MenuButton
            icon={X}
            title={t("sync.modals.decline")}
            description={t("sync.modals.declineDesc")}
            color="from-rose-500 to-red-600"
            onClick={() => handleApproval(false)}
            disabled={isAccepting}
          />
        </div>
      </BottomMenu>

      <BottomMenu isOpen={isReadyToStart} onClose={() => {}} title={t("sync.modals.readyToSync")}>
        <div className="mb-6 px-1 text-center">
          <div className="mb-4 flex justify-center">
            <div className="flex h-16 w-16 items-center justify-center rounded-2xl border border-accent/30 bg-accent/15">
              <Check className="h-8 w-8 text-accent/90" />
            </div>
          </div>
          <h4 className="mb-1 text-lg font-medium text-fg">
            {t("sync.modals.connectionEstablished")}
          </h4>
          <p className="mb-2 text-sm text-fg/50">
            {(status as any).details?.device_name} {t("sync.modals.deviceReady")}
          </p>
          <p className="text-sm text-fg/40">{t("sync.modals.startSyncMessage")}</p>
        </div>

        <div className="space-y-3">
          <MenuButton
            icon={RefreshCw}
            title={t("sync.modals.startSyncing")}
            description={t("sync.modals.startSyncingDesc")}
            color="from-blue-500 to-blue-600"
            onClick={handleStartSync}
            loading={isStartingSyncSession}
          />
        </div>
      </BottomMenu>

      <section className={cn("flex-1 overflow-y-auto px-3 pt-3 pb-6", spacing.section)}>
        {}
        {isError && (
          <div className="flex items-center gap-3 rounded-xl border border-danger/25 bg-danger/8 p-3">
            <AlertTriangle className="h-5 w-5 shrink-0 text-danger/90" />
            <div className="min-w-0 flex-1">
              <p className="text-sm font-medium text-danger/90">{t("sync.messages.error")}</p>
              <p className="text-xs text-danger/60">{(status as any).details?.message}</p>
            </div>
            <button
              type="button"
              onClick={stopSync}
              className="px-1 text-lg text-danger/60 hover:text-danger/90"
              aria-label={t("sync.aria.dismissError")}
            >
              ×
            </button>
          </div>
        )}

        {warningMessage && (
          <div className="flex items-center gap-3 rounded-xl border border-warning/25 bg-warning/8 p-3">
            <AlertTriangle className="h-5 w-5 shrink-0 text-warning/90" />
            <div className="min-w-0 flex-1">
              <p className="text-sm font-medium text-warning/90">
                {t("sync.messages.outdatedClient")}
              </p>
              <p className="text-xs text-warning/70">{warningMessage}</p>
            </div>
          </div>
        )}

        {}
        {isIdle && (
          <div>
            <SectionHeader title={t("sync.sections.mode")} icon={<Wifi size={12} />} />
            <div className="flex gap-2">
              <ModeTile
                active={activeTab === "client"}
                icon={<Smartphone className="h-5 w-5" />}
                title={t("sync.modes.join")}
                description={t("sync.modes.joinDesc")}
                tone="info"
                onClick={() => setActiveTab("client")}
              />
              <ModeTile
                active={activeTab === "host"}
                icon={<Monitor className="h-5 w-5" />}
                title={t("sync.modes.host")}
                description={t("sync.modes.hostDesc")}
                tone="accent"
                onClick={() => setActiveTab("host")}
              />
            </div>
          </div>
        )}

        {}
        {(isIdle || isError) && activeTab === "client" && (
          <div>
            <SectionHeader title={t("sync.sections.connectToHost")} icon={<Smartphone size={12} />} />
            <div className="space-y-3">
              {isMobile && (
                <button
                  onClick={startScan}
                  className={cn(
                    "flex w-full items-center justify-center gap-2 rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 text-sm font-medium text-fg",
                    interactive.transition.default,
                    interactive.active.scale,
                    "hover:border-fg/20 hover:bg-fg/8",
                  )}
                >
                  <Scan className="h-4 w-4" /> {t("sync.buttons.scanQRCode")}
                </button>
              )}

              <Card className="space-y-3 px-4 py-4">
                <div>
                  <label className="mb-1.5 block text-[11px] font-medium uppercase tracking-wider text-fg/45">
                    {t("sync.fields.hostAddress")}
                  </label>
                  <input
                    type="text"
                    value={hostIp}
                    onChange={(e) => setHostIp(e.target.value)}
                    placeholder={t("sync.fields.hostPlaceholder")}
                    className={cn(
                      "w-full rounded-lg border border-fg/10 bg-surface-el/30 px-4 py-3 font-mono text-fg placeholder-fg/30",
                      interactive.transition.fast,
                      "focus:border-fg/30 focus:outline-none",
                    )}
                  />
                </div>
                <div>
                  <label className="mb-1.5 block text-[11px] font-medium uppercase tracking-wider text-fg/45">
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
                      "w-full rounded-lg border border-fg/10 bg-surface-el/30 px-4 py-3 text-center font-mono text-xl text-fg placeholder-fg/20",
                      pin ? "tracking-[0.3em]" : "tracking-normal",
                      interactive.transition.fast,
                      "focus:border-fg/30 focus:outline-none",
                    )}
                  />
                </div>
              </Card>

              <button
                onClick={() => connectToHost()}
                disabled={
                  !hostIp ||
                  (pin.length !== 6 && !hostIp.trim().startsWith("{")) ||
                  isConnectingToHost
                }
                className={cn(
                  "flex w-full items-center justify-center gap-2 rounded-xl border border-info/40 bg-info/15 px-4 py-3 text-sm font-semibold text-info",
                  interactive.transition.default,
                  interactive.active.scale,
                  "hover:border-info/60 hover:bg-info/20 disabled:cursor-not-allowed disabled:opacity-40",
                )}
              >
                {isConnectingToHost ? (
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
            </div>
          </div>
        )}

        {}
        {isIdle && activeTab === "host" && (
          <div>
            <SectionHeader title={t("sync.sections.startHosting")} icon={<Monitor size={12} />} />
            <div className="space-y-3">
              <Card className="px-4 py-4">
                <div className="flex items-start gap-3">
                  <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border border-accent/30 bg-accent/10 text-accent/90">
                    <Monitor className="h-4 w-4" />
                  </div>
                  <div className="min-w-0 flex-1">
                    <p className={cn(typography.body.size, "font-medium text-fg")}>
                      {t("sync.hostingDesc1")}
                    </p>
                    <p className="mt-1 text-[11px] leading-relaxed text-fg/50">
                      {t("sync.hostingDesc2")}
                    </p>
                  </div>
                </div>
              </Card>

              <button
                onClick={startHost}
                disabled={isStartingHost}
                className={cn(
                  "flex w-full items-center justify-center gap-2 rounded-xl border border-accent/40 bg-accent/15 px-4 py-3 text-sm font-semibold text-accent",
                  interactive.transition.default,
                  interactive.active.scale,
                  "hover:border-accent/60 hover:bg-accent/20 disabled:cursor-not-allowed disabled:opacity-40",
                )}
              >
                {isStartingHost ? (
                  <>
                    <Loader2 className="h-4 w-4 animate-spin" />
                    {t("sync.buttons.startingServer")}
                  </>
                ) : (
                  <>
                    <Wifi className="h-4 w-4" /> {t("sync.buttons.startHosting")}
                  </>
                )}
              </button>
            </div>
          </div>
        )}

        {isSyncing && (
          <div>
            <SectionHeader title={t("sync.sections.status")} icon={<Radio size={12} />} />
            <div className="space-y-3">
              <BidirectionalLink />
              <SyncingPanel
                title={t("sync.status.syncing")}
                details={(status as { status: "Syncing"; details: SyncingDetails }).details}
              />
              <button
                onClick={stopSync}
                className={cn(
                  "w-full rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 text-sm font-medium text-fg/70",
                  interactive.transition.default,
                  interactive.active.scale,
                  "hover:border-fg/20 hover:bg-fg/8",
                )}
              >
                {t("common.buttons.cancel")}
              </button>
            </div>
          </div>
        )}

        {}
        {role === "client" &&
          (isConnecting || isWaitingForApproval) && (
            <div>
              <SectionHeader title={t("sync.sections.status")} icon={<Radio size={12} />} />
              {(() => {
                const titleText = isConnecting
                  ? t("sync.status.connecting")
                  : isWaitingForApproval
                    ? t("sync.status.waitingConfirmation")
                    : t("sync.status.waitingConfirmation");

                const subtitle =
                  isWaitingForApproval
                    ? t("sync.status.waitingConfirmationDesc")
                    : null;

                return (
                  <div className="space-y-3">
                    <SyncingPanel
                      title={titleText}
                      subtitle={subtitle}
                      details={null}
                      showProgressBar={false}
                      showCounters={false}
                      showBreakdown={false}
                    />

                    <button
                      onClick={stopSync}
                      className={cn(
                        "w-full rounded-xl border border-fg/10 bg-fg/5 px-4 py-3 text-sm font-medium text-fg/70",
                        interactive.transition.default,
                        interactive.active.scale,
                        "hover:border-fg/20 hover:bg-fg/8",
                      )}
                    >
                      {t("common.buttons.cancel")}
                    </button>
                  </div>
                );
              })()}
            </div>
          )}

        {}
        {role === "host" && (isDriver || isPendingApproval || isReadyToStart) && (
          <div>
            <SectionHeader
              title={t("sync.sections.hosting")}
              icon={<Radio size={12} />}
              right={
                <div className="flex items-center gap-2">
                  <div className="flex items-center gap-1.5 rounded-full border border-info/25 bg-info/10 px-2 py-0.5">
                    <Users className="h-3 w-3 text-info/90" />
                    <span className="text-[10px] font-bold uppercase tracking-wider text-info/90">
                      {(status as any).details?.clients ?? 0} {t("sync.status.clientsUnit")}
                    </span>
                  </div>
                  <div className="flex items-center gap-1.5 rounded-full border border-accent/30 bg-accent/10 px-2 py-0.5">
                    <span className="relative flex h-1.5 w-1.5">
                      <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-accent/80"></span>
                      <span className="relative inline-flex h-1.5 w-1.5 rounded-full bg-accent"></span>
                    </span>
                    <span className="text-[10px] font-bold uppercase tracking-wider text-accent/90">
                      {t("sync.status.live")}
                    </span>
                  </div>
                </div>
              }
            />

            <div className="space-y-3">
              <Card className="overflow-hidden">
                  <div className="flex flex-col items-center px-5 pt-6 pb-4">
                    <div className="rounded-2xl bg-white p-3 shadow-[0_8px_24px_rgba(0,0,0,0.35)] ring-1 ring-fg/10">
                      <QRCodeComponent
                        value={JSON.stringify({
                          ip: (status as any).details?.ip || localIp,
                          port: (status as any).details?.port || 8000,
                          pin: (status as any).details?.pin || "",
                        })}
                        size={Math.min(window.innerWidth - 140, 168)}
                      />
                    </div>
                    <p className="mt-3 text-[11px] text-fg/45">{t("sync.pinDescription")}</p>
                  </div>

                  <div className="grid grid-cols-1 divide-y divide-fg/8 border-t border-fg/8 sm:grid-cols-2 sm:divide-x sm:divide-y-0">
                    {}
                    <div className="px-4 py-3.5">
                      <p
                        className={cn(
                          typography.overline.size,
                          typography.overline.weight,
                          typography.overline.tracking,
                          typography.overline.transform,
                          "text-fg/40",
                        )}
                      >
                        {t("sync.sections.localAddress")}
                      </p>
                      <div className="mt-2 flex items-center gap-2">
                        <code className="min-w-0 flex-1 truncate font-mono text-sm font-semibold text-fg">
                          {(status as any).details?.ip || localIp}:
                          {(status as any).details?.port || 8000}
                        </code>
                        <button
                          onClick={() =>
                            copyAddress(
                              (status as any).details?.ip || localIp,
                              (status as any).details?.port || 8000,
                            )
                          }
                          className={cn(
                            "shrink-0 rounded-lg border border-fg/10 bg-fg/5 p-1.5 text-fg/60",
                            interactive.transition.fast,
                            interactive.active.scale,
                            "hover:border-fg/20 hover:bg-fg/10 hover:text-fg",
                          )}
                          aria-label={t("sync.aria.copyAddress")}
                        >
                          {copied ? (
                            <Check className="h-4 w-4 text-accent/90" />
                          ) : (
                            <Copy className="h-4 w-4" />
                          )}
                        </button>
                      </div>
                    </div>

                    {}
                    <div className="bg-accent/5 px-4 py-3.5">
                      <p
                        className={cn(
                          typography.overline.size,
                          typography.overline.weight,
                          typography.overline.tracking,
                          typography.overline.transform,
                          "text-accent/70",
                        )}
                      >
                        {t("sync.sections.connectionPin")}
                      </p>
                      <code className="mt-2 block font-mono text-xl font-bold tracking-[0.35em] text-accent/95">
                        {(status as any).details?.pin || "------"}
                      </code>
                    </div>
                  </div>
              </Card>

              {}
              <details className="group rounded-xl border border-fg/10 bg-fg/5">
                  <summary className="flex cursor-pointer list-none items-center justify-between gap-2 px-4 py-3">
                    <div className="flex items-center gap-2">
                      <HelpCircle className="h-4 w-4 text-fg/40" />
                      <span
                        className={cn(
                          typography.overline.size,
                          typography.overline.weight,
                          typography.overline.tracking,
                          typography.overline.transform,
                          "text-fg/50",
                        )}
                      >
                        {t("sync.sections.setupGuide")}
                      </span>
                    </div>
                    <span className="text-[11px] text-fg/35 transition-transform group-open:rotate-180">
                      ▾
                    </span>
                  </summary>
                  <ul className="space-y-3 border-t border-fg/8 px-4 py-3.5">
                    {[
                      t("sync.setupSteps.step1"),
                      t("sync.setupSteps.step2"),
                      t("sync.setupSteps.step3"),
                    ].map((step, i) => (
                      <li key={i} className="flex items-start gap-3 text-xs text-fg/55">
                        <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full border border-fg/10 bg-fg/8 text-[10px] font-bold text-fg/50">
                          {i + 1}
                        </span>
                        <span className="leading-5">{step}</span>
                      </li>
                    ))}
                  </ul>
              </details>

              <button
                onClick={stopSync}
                className={cn(
                  "flex w-full items-center justify-center gap-2 rounded-xl border border-danger/25 bg-danger/10 px-4 py-3 text-sm font-semibold text-danger/90",
                  interactive.transition.default,
                  interactive.active.scale,
                  "hover:border-danger/40 hover:bg-danger/15",
                )}
              >
                <X className="h-4 w-4" /> {t("sync.buttons.stopHosting")}
              </button>
            </div>
          </div>
        )}

        {}
        {isCompleted && (
          <div>
            <SectionHeader title={t("sync.sections.status")} icon={<CheckCircle size={12} />} />
            <Card className="overflow-hidden border-accent/25 bg-accent/8">
              <div className="flex flex-col items-center px-5 py-6 text-center">
                <div className="mb-3 flex h-14 w-14 items-center justify-center rounded-2xl border border-accent/40 bg-accent/15 shadow-[0_0_24px_-6px_rgba(16,185,129,0.45)]">
                  <CheckCircle className="h-7 w-7 text-accent/95" />
                </div>
                <p className="text-base font-semibold text-accent/95">
                  {t("sync.messages.completed")}
                </p>
                <p className="mt-1 text-xs text-accent/65">{t("sync.messages.completedDesc")}</p>
                <div className="mt-4 h-1.5 w-32 overflow-hidden rounded-full bg-accent/15">
                  <div className="h-full w-full rounded-full bg-accent/70" />
                </div>
              </div>
              <div className="grid grid-cols-2 divide-x divide-accent/15 border-t border-accent/15">
                <button
                  onClick={startHost}
                  className={cn(
                    "flex items-center justify-center gap-2 px-4 py-3.5 text-sm font-semibold text-accent/95",
                    interactive.transition.default,
                    interactive.active.scale,
                    "hover:bg-accent/12",
                  )}
                >
                  <RefreshCw className="h-4 w-4" />
                  {t("sync.buttons.hostAgain")}
                </button>
                <button
                  onClick={stopSync}
                  className={cn(
                    "flex items-center justify-center gap-2 px-4 py-3.5 text-sm font-medium text-fg/70",
                    interactive.transition.default,
                    interactive.active.scale,
                    "hover:bg-fg/5 hover:text-fg",
                  )}
                >
                  <Check className="h-4 w-4" />
                  {t("sync.buttons.done")}
                </button>
              </div>
            </Card>
          </div>
        )}

        <SyncConflictInbox refreshKey={status.status} disabled={isSyncing} />

        <p className="px-1 pt-2 text-[11px] text-fg/35">{t("sync.disclaimer")}</p>
      </section>

      {}
      {isScanning && (
        <div className="fixed inset-0 z-50 flex flex-col items-center justify-end bg-transparent pb-20">
          <div className="mb-8 text-center">
            <div className={cn("relative mx-auto mb-4 h-64 w-64 overflow-hidden border-2 border-fg/50", radius.lg)}>
              <div className={cn("absolute inset-0 animate-pulse border-2 border-fg/20", radius.lg)}></div>
              <div className="absolute left-0 top-0 h-1 w-full animate-[scan_2s_infinite] bg-fg/70 shadow-[0_0_10px_currentColor]"></div>
            </div>
            <p className="font-medium text-fg drop-shadow-md shadow-black/50">
              {t("sync.scanner.title")}
            </p>
          </div>
          <button
            onClick={stopScan}
            className={cn(
              "flex items-center gap-2 rounded-full border border-fg/20 bg-fg/10 px-6 py-3 font-medium text-fg backdrop-blur-md",
              interactive.transition.default,
              interactive.active.scale,
            )}
          >
            <X className="h-5 w-5" /> {t("sync.scanner.cancel")}
          </button>
          <style>{`
            @keyframes scan {
              0% { transform: translateY(0); opacity: 0.5; }
              50% { opacity: 1; }
              100% { transform: translateY(256px); opacity: 0.5; }
            }
          `}</style>
        </div>
      )}

      {missingRequirements.length > 0 && (
        <MissingModelRequirementsSheet
          isOpen
          title={t("sync.requirements.title")}
          description={t("sync.requirements.description")}
          missing={missingRequirements}
          onClose={() => void handleContinueWithoutRequirements()}
          onDownload={handleDownloadRequirements}
          closeLabel={t("sync.requirements.continueForNow")}
          downloadLabel={t("sync.requirements.download")}
        />
      )}
    </div>
  );
}
