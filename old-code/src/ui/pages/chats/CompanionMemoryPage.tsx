import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { motion, AnimatePresence } from "framer-motion";
import {
  ArrowLeft,
  Bot,
  Brain,
  CalendarClock,
  ChevronDown,
  Clock3,
  Heart,
  Link2,
  Loader2,
  Pin,
  PinOff,
  Plus,
  RefreshCw,
  Save,
  Search,
  Shield,
  Snowflake,
  Sparkles,
  Trash2,
  User,
  X,
} from "lucide-react";
import { useParams, useSearchParams } from "react-router-dom";
import { confirmBottomMenu } from "../../components/ConfirmBottomMenu";
import { cn, components, interactive, radius } from "../../design-tokens";
import { Routes, useNavigationManager } from "../../navigation";
import {
  addMemory,
  readSettings,
  removeMemory,
  saveSession,
  setMemoryColdState,
  setMemoryObservedAt,
  toggleMemoryPin,
  updateMemory,
} from "../../../core/storage/repo";
import { BottomMenu } from "../../components/BottomMenu";
import { DateTimePicker } from "../../components/DateTimePicker";
import {
  companionCategoryLabel,
  COMPANION_CATEGORY_ORDER,
  emotionLabel,
  formatPercent,
  formatRelativeTime,
  isCompanionChat,
  normalizeCompanionCategory,
  topEmotionEntries,
  useCompanionSessionData,
  type CompanionMemoryCategory,
  type CompanionMemoryItem,
} from "./companionUi";
import { useI18n } from "../../../core/i18n/context";
import { RELATIONSHIP_AXIS_ANCHORS } from "../characters/utils/companionDefaults";
import { storageBridge } from "../../../core/storage/files";
import {
  MemoryCycleHub,
  type DynamicMemoryCycleStatus,
} from "../../components/MemoryCycleHub";
import { sessionEffectiveNowMs } from "./utils/companionTimeOverride";

type MemoryFilter = "all" | "active" | "superseded";

const sectionIcons: Record<CompanionMemoryCategory, React.ComponentType<{ className?: string; size?: number }>> = {
  relationship: Heart,
  milestone: Link2,
  boundary: Shield,
  preference: Brain,
  profile: Brain,
  routine: Clock3,
  episodic: Clock3,
  emotional_snapshot: Heart,
};

function PageHeader({
  title,
  subtitle,
  onBack,
  right,
}: {
  title: string;
  subtitle?: string;
  onBack: () => void;
  right?: React.ReactNode;
}) {
  const { t } = useI18n();

  return (
    <header
      className={cn(
        "z-20 shrink-0 border-b border-fg/8 pl-3 lg:pl-8",
        "pr-3 lg:pr-8",
        "bg-surface/95 backdrop-blur-xl",
      )}
      style={{
        paddingTop: "calc(env(safe-area-inset-top) + 12px)",
        paddingBottom: "12px",
      }}
    >
      <div className="flex h-10 items-center justify-between">
        <div className="flex min-w-0 items-center gap-2.5">
          <button
            onClick={onBack}
            className="flex shrink-0 items-center justify-center -ml-2 px-[0.6em] py-[0.3em] text-fg/80 transition hover:text-fg"
            aria-label={t("chats.companionMemoryPage.backLabel")}
          >
            <ArrowLeft size={18} strokeWidth={2.5} />
          </button>
          <div className="min-w-0">
            <p className="truncate text-[15px] font-semibold text-fg">{title}</p>
            {subtitle ? <p className="truncate text-[11px] text-fg/45">{subtitle}</p> : null}
          </div>
        </div>
        <div className="flex items-center gap-2">
          {right}
        </div>
      </div>
    </header>
  );
}

function SectionLabel({ children, right }: { children: React.ReactNode; right?: React.ReactNode }) {
  return (
    <div className="mb-2 flex items-center gap-2">
      <span className="text-[12px] font-semibold uppercase tracking-wider text-fg/50">
        {children}
      </span>
      {right ? <span className="ml-auto text-[10px] text-fg/35">{right}</span> : null}
    </div>
  );
}

function StatTile({
  label,
  value,
  tone = "default",
  low,
  mid,
  high,
  bipolar,
}: {
  label: string;
  value: number;
  tone?: "default" | "warm" | "warning";
  low?: string;
  mid?: string;
  high?: string;
  bipolar?: boolean;
}) {
  const barTone =
    tone === "warm"
      ? "bg-amber-400"
      : tone === "warning"
        ? "bg-rose-400"
        : "bg-accent";
  const v = bipolar ? Math.max(-1, Math.min(1, value)) : Math.max(0, Math.min(1, value));
  const pct = Math.round(v * 100);
  const mag = Math.abs(v) * (bipolar ? 50 : 100);
  return (
    <div className="rounded-xl border border-fg/8 bg-fg/2 px-3 py-2.5">
      <div className="text-[10px] font-semibold uppercase tracking-wider text-fg/45">
        {label}
      </div>
      <div className="mt-0.5 text-[17px] font-semibold tabular-nums text-fg/90">
        {bipolar && pct > 0 ? `+${pct}` : pct}%
      </div>
      {bipolar ? (
        <div className="relative mt-1.5 h-[3px] rounded-full bg-fg/6">
          <div className="absolute left-1/2 top-0 h-full w-px -translate-x-1/2 bg-fg/25" />
          <div
            className={cn(
              "absolute top-0 h-full",
              v >= 0 ? cn("rounded-r-full", barTone) : "rounded-l-full bg-rose-400",
            )}
            style={v >= 0 ? { left: "50%", width: `${mag}%` } : { right: "50%", width: `${mag}%` }}
          />
        </div>
      ) : (
        <div className="mt-1.5 h-[3px] rounded-full bg-fg/6">
          <div className={cn("h-full rounded-full", barTone)} style={{ width: `${pct}%` }} />
        </div>
      )}
      {low && high && (
        <div className="mt-1 flex items-center justify-between text-[9px] text-fg/40">
          <span>{low}</span>
          {bipolar && mid ? <span>{mid}</span> : null}
          <span>{high}</span>
        </div>
      )}
    </div>
  );
}

function Pill({
  children,
  tone = "default",
  className,
}: {
  children: React.ReactNode;
  tone?: "default" | "muted" | "accent" | "warning";
  className?: string;
}) {
  const toneClass =
    tone === "accent"
      ? "border-accent/25 bg-accent/10 text-accent"
      : tone === "warning"
        ? "border-warning/25 bg-warning/10 text-warning"
        : tone === "muted"
          ? "border-fg/8 bg-fg/3 text-fg/45"
          : "border-fg/10 bg-fg/4 text-fg/60";
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] font-medium",
        toneClass,
        className,
      )}
    >
      {children}
    </span>
  );
}

function MetricMini({ label, value, tone = "default" }: { label: string; value: number; tone?: "default" | "warm" | "warning" }) {
  const barTone =
    tone === "warm" ? "bg-amber-400" : tone === "warning" ? "bg-rose-400" : "bg-accent";
  const pct = Math.round(Math.max(0, Math.min(1, value)) * 100);
  return (
    <div>
      <div className="mb-1 flex items-center justify-between">
        <span className="text-[10px] uppercase tracking-wider text-fg/45">{label}</span>
        <span className="text-[10px] font-medium tabular-nums text-fg/70">{pct}%</span>
      </div>
      <div className="h-[3px] rounded-full bg-fg/6">
        <div className={cn("h-full rounded-full", barTone)} style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}

type CardProps = {
  memory: CompanionMemoryItem;
  expanded: boolean;
  editing: boolean;
  editValue: string;
  editCategory: CompanionMemoryCategory;
  saving: boolean;
  actionBusy: boolean;
  effectiveNow: number;
  onToggleExpand: () => void;
  onStartEdit: (memory: CompanionMemoryItem) => void;
  onEditValue: (value: string) => void;
  onEditCategory: (value: CompanionMemoryCategory) => void;
  onSaveEdit: (memory: CompanionMemoryItem) => void;
  onCancelEdit: () => void;
  onTogglePin: (memory: CompanionMemoryItem) => void;
  onToggleCold: (memory: CompanionMemoryItem) => void;
  onEditDate: (memory: CompanionMemoryItem) => void;
  onDelete: (memory: CompanionMemoryItem) => void;
};

function MemoryCard({
  memory,
  expanded,
  editing,
  editValue,
  editCategory,
  saving,
  actionBusy,
  effectiveNow,
  onToggleExpand,
  onStartEdit,
  onEditValue,
  onEditCategory,
  onSaveEdit,
  onCancelEdit,
  onTogglePin,
  onToggleCold,
  onEditDate,
  onDelete,
}: CardProps) {
  const { t } = useI18n();
  const Icon = sectionIcons[memory.category];
  const isUser = memory.sourceRole === "user";
  const SourceIcon = isUser ? User : Bot;

  return (
    <motion.article
      layout
      variants={{
        hidden: { opacity: 0, y: 8 },
        visible: { opacity: 1, y: 0 },
      }}
      exit={{ opacity: 0, scale: 0.97 }}
      transition={{ duration: 0.15 }}
      className={cn(
        "group overflow-hidden rounded-xl border",
        expanded
          ? "border-fg/12 bg-fg/3"
          : memory.isActive
            ? "border-fg/6 bg-fg/2 hover:border-fg/10 hover:bg-fg/3"
            : "border-fg/6 bg-fg/2 opacity-70 hover:opacity-90",
      )}
    >
      <button
        type="button"
        onClick={onToggleExpand}
        className="block w-full cursor-pointer px-4 py-3 text-left"
      >
        <div className="flex items-start gap-2.5">
          <div className="mt-0.5 shrink-0 text-fg/40">
            <SourceIcon size={13} className={isUser ? "text-emerald-400/80" : "text-blue-400/80"} />
          </div>
          <div className="min-w-0 flex-1">
            <p
              className={cn(
                "text-sm leading-relaxed text-fg/90",
                !expanded && "line-clamp-2",
                !memory.isActive && "text-fg/55",
              )}
            >
              {memory.text}
            </p>
            <div className="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-[10px] text-fg/40">
              <span className="inline-flex items-center gap-1">
                <Icon size={10} />
                {companionCategoryLabel(t, memory.category)}
              </span>
              <span className="text-fg/20">·</span>
              <span>{formatRelativeTime(t, memory.observedAt ?? memory.createdAt, effectiveNow)}</span>
              {memory.isPinned && (
                <>
                  <span className="text-fg/20">·</span>
                  <span className="inline-flex items-center gap-1 text-amber-400/80">
                    <Pin size={10} /> {t("chats.companionMemoryPage.pinned")}
                  </span>
                </>
              )}
              {memory.isCold && (
                <>
                  <span className="text-fg/20">·</span>
                  <span className="inline-flex items-center gap-1 text-fg/45">
                    <Snowflake size={10} /> {t("chats.companionMemoryPage.cold")}
                  </span>
                </>
              )}
              {!memory.isActive && (
                <>
                  <span className="text-fg/20">·</span>
                  <span className="text-warning/80">{t("chats.companionMemoryPage.superseded")}</span>
                </>
              )}
            </div>
          </div>
          <ChevronDown
            size={14}
            className={cn(
              "mt-1 shrink-0 text-fg/30 transition-transform",
              expanded && "rotate-180",
            )}
          />
        </div>
      </button>

      <AnimatePresence initial={false}>
        {expanded && (
          <motion.div
            key="body"
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.18, ease: "easeOut" }}
            className="overflow-hidden border-t border-fg/6"
          >
            <div className="space-y-3 px-4 py-3">
              {editing ? (
                <div className="space-y-2">
                  <textarea
                    value={editValue}
                    onChange={(event) => onEditValue(event.target.value)}
                    className={cn(
                      components.input.base,
                      "min-h-[88px] w-full resize-y px-3 py-2 text-sm text-fg",
                    )}
                    placeholder={t("chats.companionMemoryPage.refineMemoryPlaceholder")}
                  />
                  <select
                    value={editCategory}
                    onChange={(event) => onEditCategory(normalizeCompanionCategory(event.target.value))}
                    className={cn(components.input.base, "w-full px-3 py-2 text-xs text-fg")}
                  >
                    {COMPANION_CATEGORY_ORDER.map((category) => (
                      <option key={category} value={category}>
                        {companionCategoryLabel(t, category)}
                      </option>
                    ))}
                  </select>
                </div>
              ) : (
                <div className="grid gap-2 sm:grid-cols-3">
                  <MetricMini label={t("chats.companionMemoryPage.metrics.persistence")} value={memory.persistenceImportance} />
                  <MetricMini label={t("chats.companionMemoryPage.metrics.promptWeight")} value={memory.promptImportance} tone="warm" />
                  <MetricMini label={t("chats.companionMemoryPage.metrics.volatility")} value={memory.volatility} tone="warning" />
                </div>
              )}

              {!editing && memory.canonicalEntities.length ? (
                <div className="flex flex-wrap gap-1">
                  {memory.canonicalEntities.map((entity) => (
                    <Pill
                      key={`${memory.id}-${entity.canonicalKey}-${entity.surface}`}
                      tone="muted"
                    >
                      {entity.canonicalName}
                      <span className="text-fg/30">·{entity.label}</span>
                    </Pill>
                  ))}
                </div>
              ) : null}

              {!editing &&
              (memory.factSignature ||
                memory.supersedes.length ||
                memory.supersededAt ||
                memory.lastAccessedAt ||
                memory.observedAt) ? (
                <div className="flex flex-wrap gap-x-3 gap-y-1 text-[10px] text-fg/40">
                  {memory.observedAt ? (
                    <span className="inline-flex items-center gap-1 text-fg/55">
                      <CalendarClock size={10} />
                      {t("chats.companionMemoryPage.detail.dated", { time: formatRelativeTime(t, memory.observedAt, effectiveNow) })}
                    </span>
                  ) : null}
                  {memory.lastAccessedAt ? (
                    <span>{t("chats.companionMemoryPage.detail.lastUsed", { time: formatRelativeTime(t, memory.lastAccessedAt, effectiveNow) })}</span>
                  ) : null}
                  {memory.factSignature ? <span>{t("chats.companionMemoryPage.detail.key", { key: memory.factSignature })}</span> : null}
                  {memory.supersedes.length ? (
                    <span>{t("chats.companionMemoryPage.detail.replaces", { count: memory.supersedes.length })}</span>
                  ) : null}
                  {memory.supersededAt ? (
                    <span>{t("chats.companionMemoryPage.detail.superseded", { time: formatRelativeTime(t, memory.supersededAt, effectiveNow) })}</span>
                  ) : null}
                </div>
              ) : null}

              <div className="flex flex-wrap items-center gap-1.5 pt-1">
                {editing ? (
                  <>
                    <ActionPill
                      icon={Save}
                      label={saving ? t("common.buttons.saving") : t("common.buttons.save")}
                      onClick={() => onSaveEdit(memory)}
                      disabled={saving}
                      tone="accent"
                    />
                    <ActionPill icon={X} label={t("common.buttons.cancel")} onClick={onCancelEdit} disabled={saving} />
                  </>
                ) : (
                  <>
                    <ActionPill
                      icon={memory.isPinned ? PinOff : Pin}
                      label={memory.isPinned ? t("chats.togglePin.unpin") : t("chats.togglePin.pin")}
                      onClick={() => onTogglePin(memory)}
                      disabled={actionBusy}
                      tone={memory.isPinned ? "accent" : "default"}
                    />
                    <ActionPill
                      icon={memory.isCold ? Brain : Snowflake}
                      label={memory.isCold ? t("chats.companionMemoryPage.actions.warmUp") : t("chats.companionMemoryPage.actions.coolDown")}
                      onClick={() => onToggleCold(memory)}
                      disabled={actionBusy}
                    />
                    <ActionPill
                      icon={Save}
                      label={t("common.buttons.edit")}
                      onClick={() => onStartEdit(memory)}
                      disabled={actionBusy}
                    />
                    <ActionPill
                      icon={CalendarClock}
                      label={t("chats.companionMemoryPage.actions.date")}
                      onClick={() => onEditDate(memory)}
                      disabled={actionBusy}
                    />
                    <ActionPill
                      icon={Trash2}
                      label={t("common.buttons.delete")}
                      onClick={() => onDelete(memory)}
                      disabled={actionBusy}
                      tone="danger"
                    />
                  </>
                )}
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.article>
  );
}

function ActionPill({
  icon: Icon,
  label,
  onClick,
  disabled,
  tone = "default",
}: {
  icon: React.ComponentType<{ size?: number; className?: string }>;
  label: string;
  onClick: () => void;
  disabled?: boolean;
  tone?: "default" | "accent" | "danger";
}) {
  const toneClass =
    tone === "accent"
      ? "border-accent/30 bg-accent/12 text-accent hover:bg-accent/16"
      : tone === "danger"
        ? "border-fg/8 bg-fg/3 text-fg/55 hover:border-danger/30 hover:bg-danger/10 hover:text-danger"
        : "border-fg/8 bg-fg/3 text-fg/60 hover:border-fg/15 hover:bg-fg/6 hover:text-fg/85";
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "inline-flex items-center gap-1.5 rounded-md border px-2 py-1 text-[11px] font-medium transition",
        toneClass,
        "disabled:pointer-events-none disabled:opacity-40",
      )}
    >
      <Icon size={12} />
      {label}
    </button>
  );
}

export function CompanionMemoryPage() {
  const { t } = useI18n();
  const { characterId } = useParams<{ characterId: string }>();
  const [searchParams] = useSearchParams();
  const sessionId = searchParams.get("sessionId");
  const { go, backOrReplace } = useNavigationManager();
  const { session, setSession, character, loading, error, reload, memoryItems } =
    useCompanionSessionData(characterId, sessionId);

  const [searchTerm, setSearchTerm] = useState("");
  const [categoryFilter, setCategoryFilter] = useState<"all" | CompanionMemoryCategory>("all");
  const [stateFilter, setStateFilter] = useState<MemoryFilter>("active");
  const [showComposer, setShowComposer] = useState(false);
  const [newMemory, setNewMemory] = useState("");
  const [newCategory, setNewCategory] = useState<CompanionMemoryCategory>("relationship");
  const [composerBusy, setComposerBusy] = useState(false);
  const [actionBusyId, setActionBusyId] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingValue, setEditingValue] = useState("");
  const [editingCategory, setEditingCategory] = useState<CompanionMemoryCategory>("relationship");
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [triggering, setTriggering] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [progressStep, setProgressStep] = useState<number | null>(null);
  const [genTokens, setGenTokens] = useState<number | null>(null);
  const [genTps, setGenTps] = useState<number | null>(null);
  const [genLastBeatAt, setGenLastBeatAt] = useState<number | null>(null);
  const effectiveNow = sessionEffectiveNowMs(session, Date.now());
  const [genRecentText, setGenRecentText] = useState<string | null>(null);
  const [showLiveOutput, setShowLiveOutput] = useState(false);
  const [developerMode, setDeveloperMode] = useState(false);
  const [nowTick, setNowTick] = useState(0);
  const liveOutputRef = useRef<HTMLPreElement | null>(null);
  const [showSummaryEditor, setShowSummaryEditor] = useState(false);
  const [summaryDraft, setSummaryDraft] = useState("");
  const [savingSummary, setSavingSummary] = useState(false);
  const [cycleStatus, setCycleStatus] = useState<DynamicMemoryCycleStatus | null>(null);
  const [retrying, setRetrying] = useState(false);
  const [retrySuccess, setRetrySuccess] = useState(false);

  const companion = character?.companion ?? null;
  const companionState = session?.companionState;
  const relationshipState = companionState?.relationshipState;
  const emotionalState = companionState?.emotionalState;
  const activeSignals = companionState?.activeSignals ?? [];
  const isDynamic = character?.memoryType === "dynamic";

  useEffect(() => {
    if (!isDynamic || !session?.id) {
      setCycleStatus(null);
      return;
    }
    let cancelled = false;
    storageBridge
      .dynamicMemoryCycleStatus(session.id)
      .then((status) => {
        if (!cancelled) setCycleStatus(status);
      })
      .catch(() => {
        if (!cancelled) setCycleStatus(null);
      });
    return () => {
      cancelled = true;
    };
  }, [isDynamic, session?.id, session?.memoryStatus, session?.memoryToolEvents?.length]);

  useEffect(() => {
    if (!session?.id) return;
    const listeners: Array<() => void> = [];

    const resetGeneration = () => {
      setProgressStep(null);
      setGenTokens(null);
      setGenTps(null);
      setGenLastBeatAt(null);
      setGenRecentText(null);
    };

    const setup = async () => {
      const events = [
        "dynamic-memory:success",
        "dynamic-memory:error",
        "dynamic-memory:cancelled",
        "dynamic-memory:processing",
      ];
      for (const name of events) {
        const unlisten = await listen(name, (event: any) => {
          if (event.payload?.sessionId !== session.id) return;
          if (name !== "dynamic-memory:processing") {
            resetGeneration();
          }
          void reload();
        });
        listeners.push(unlisten);
      }

      const unlistenProgress = await listen("dynamic-memory:progress", (event: any) => {
        if (event.payload?.sessionId === session.id) {
          setProgressStep(Number(event.payload.step));
        }
      });
      listeners.push(unlistenProgress);

      const unlistenHeartbeat = await listen("llm-generation-heartbeat", (event: any) => {
        const requestId: string = event.payload?.requestId ?? "";
        if (!requestId.startsWith("dynamic-memory:")) return;
        setGenTokens(Number(event.payload?.tokens ?? 0));
        setGenTps(Number(event.payload?.tokensPerSecond ?? 0));
        setGenLastBeatAt(Date.now());
        if (typeof event.payload?.recentText === "string") {
          setGenRecentText(event.payload.recentText);
        }
      });
      listeners.push(unlistenHeartbeat);
    };

    void setup();
    return () => {
      listeners.forEach((unlisten) => unlisten());
    };
  }, [reload, session?.id]);

  const memoryProcessing = session?.memoryStatus === "processing";
  const memoryCycleActive = memoryProcessing || triggering || retrying;

  useEffect(() => {
    let mounted = true;
    void readSettings().then((settings) => {
      if (mounted) setDeveloperMode(settings.advancedSettings?.developerModeEnabled ?? false);
    });
    return () => {
      mounted = false;
    };
  }, []);

  useEffect(() => {
    if (!memoryCycleActive) return;
    const id = window.setInterval(() => setNowTick((n) => n + 1), 1000);
    return () => window.clearInterval(id);
  }, [memoryCycleActive]);

  useEffect(() => {
    const el = liveOutputRef.current;
    if (el && showLiveOutput) el.scrollTop = el.scrollHeight;
  }, [genRecentText, showLiveOutput]);

  const generationStalledMs = genLastBeatAt != null ? Date.now() - genLastBeatAt : null;
  const generationStalled = generationStalledMs != null && generationStalledMs > 15000;
  void nowTick;

  const handleTriggerMemory = useCallback(async () => {
    if (!session?.id || triggering || memoryProcessing) return;
    setTriggering(true);
    setProgressStep(null);
    setGenTokens(null);
    setGenTps(null);
    setGenLastBeatAt(null);
    setGenRecentText(null);
    try {
      await storageBridge.triggerDynamicMemory(session.id);
      await reload();
    } catch (err) {
      console.error("Failed to trigger companion memory processing:", err);
    } finally {
      setTriggering(false);
    }
  }, [session?.id, triggering, memoryProcessing, reload]);

  const handleCancelMemory = useCallback(async () => {
    if (!session || cancelling) return;
    setCancelling(true);
    try {
      if (session.memoryStatus === "processing") {
        await storageBridge.abortDynamicMemory(session.id);
      }
      const next = { ...session, memoryStatus: "idle" as const, memoryError: null };
      await saveSession(next, { preserveDynamicMemory: false });
      setSession(next);
    } catch (err) {
      console.error("Failed to cancel companion memory cycle:", err);
    } finally {
      setCancelling(false);
    }
  }, [session, cancelling, setSession]);

  const handleRetryMemory = useCallback(async () => {
    if (!session?.id || retrying) return;
    setRetrying(true);
    setRetrySuccess(false);
    try {
      await storageBridge.retryDynamicMemory(session.id);
      await reload();
      setRetrySuccess(true);
      window.setTimeout(() => setRetrySuccess(false), 3000);
    } catch (err) {
      console.error("Failed to retry companion memory processing:", err);
      await reload();
    } finally {
      setRetrying(false);
    }
  }, [session?.id, retrying, reload]);

  useEffect(() => {
    if (!showSummaryEditor) setSummaryDraft(session?.memorySummary ?? "");
  }, [session?.memorySummary, showSummaryEditor]);

  const handleSaveSummary = useCallback(async () => {
    if (!session || savingSummary) return;
    setSavingSummary(true);
    try {
      const updated = { ...session, memorySummary: summaryDraft };
      await saveSession(updated, { preserveDynamicMemory: false });
      setSession(updated);
      setShowSummaryEditor(false);
    } catch (err) {
      console.error("Failed to save companion context summary:", err);
    } finally {
      setSavingSummary(false);
    }
  }, [session, savingSummary, summaryDraft, setSession]);

  const filteredItems = useMemo(() => {
    return memoryItems.filter((item) => {
      const matchesSearch =
        !searchTerm.trim() || item.text.toLowerCase().includes(searchTerm.trim().toLowerCase());
      const matchesCategory = categoryFilter === "all" || item.category === categoryFilter;
      const matchesState =
        stateFilter === "all" ||
        (stateFilter === "active" ? item.isActive : !item.isActive);
      return matchesSearch && matchesCategory && matchesState;
    });
  }, [categoryFilter, memoryItems, searchTerm, stateFilter]);

  const topFelt = useMemo(() => topEmotionEntries(emotionalState?.felt, 4), [emotionalState?.felt]);
  const counts = useMemo(
    () => ({
      total: memoryItems.length,
      active: memoryItems.filter((item) => item.isActive).length,
      superseded: memoryItems.filter((item) => !item.isActive).length,
      pinned: memoryItems.filter((item) => item.isPinned).length,
      ai: memoryItems.filter((item) => item.sourceRole !== "user").length,
      user: memoryItems.filter((item) => item.sourceRole === "user").length,
    }),
    [memoryItems],
  );

  const toggleExpand = useCallback((id: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const startEdit = useCallback((memory: CompanionMemoryItem) => {
    setEditingId(memory.id);
    setEditingValue(memory.text);
    setEditingCategory(memory.category);
    setExpandedIds((prev) => {
      const next = new Set(prev);
      next.add(memory.id);
      return next;
    });
  }, []);

  const cancelEdit = useCallback(() => {
    setEditingId(null);
    setEditingValue("");
    setEditingCategory("relationship");
  }, []);

  const saveEdit = useCallback(
    async (memory: CompanionMemoryItem) => {
      if (!session?.id) return;
      const trimmed = editingValue.trim();
      if (!trimmed) return;

      setActionBusyId(memory.id);
      try {
        const updated = await updateMemory(session.id, memory.index, trimmed, editingCategory);
        if (updated) setSession(updated);
        cancelEdit();
      } finally {
        setActionBusyId(null);
      }
    },
    [cancelEdit, editingCategory, editingValue, session?.id, setSession],
  );

  const handleAddMemory = useCallback(async () => {
    if (!session?.id || !newMemory.trim()) return;
    setComposerBusy(true);
    try {
      const updated = await addMemory(session.id, newMemory.trim(), newCategory);
      if (updated) setSession(updated);
      setNewMemory("");
      setNewCategory("relationship");
      setShowComposer(false);
    } finally {
      setComposerBusy(false);
    }
  }, [newCategory, newMemory, session?.id, setSession]);

  const handleDeleteMemory = useCallback(
    async (memory: CompanionMemoryItem) => {
      if (!session?.id) return;
      const confirmed = await confirmBottomMenu({
        title: t("chats.companionMemoryPage.deleteMemoryTitle"),
        message: t("chats.companionMemoryPage.deleteMemoryDesc"),
        confirmLabel: t("common.buttons.delete"),
        destructive: true,
      });
      if (!confirmed) return;

      setActionBusyId(memory.id);
      try {
        const updated = await removeMemory(session.id, memory.index);
        if (updated) setSession(updated);
      } finally {
        setActionBusyId(null);
      }
    },
    [session?.id, setSession, t],
  );

  const handleTogglePin = useCallback(
    async (memory: CompanionMemoryItem) => {
      if (!session?.id) return;
      setActionBusyId(memory.id);
      try {
        const updated = await toggleMemoryPin(session.id, memory.index);
        if (updated) setSession(updated);
      } finally {
        setActionBusyId(null);
      }
    },
    [session?.id, setSession],
  );

  const handleToggleCold = useCallback(
    async (memory: CompanionMemoryItem) => {
      if (!session?.id || !session.memoryEmbeddings?.length) return;
      setActionBusyId(memory.id);
      try {
        const updated = await setMemoryColdState(session.id, memory.index, !memory.isCold);
        if (updated) setSession(updated);
      } finally {
        setActionBusyId(null);
      }
    },
    [session?.id, session?.memoryEmbeddings?.length, setSession],
  );

  const [dateMenuMemory, setDateMenuMemory] = useState<CompanionMemoryItem | null>(null);
  const [dateDraftMs, setDateDraftMs] = useState(() => Date.now());

  const startEditDate = useCallback((memory: CompanionMemoryItem) => {
    setDateDraftMs(memory.observedAt ?? Date.now());
    setDateMenuMemory(memory);
  }, []);

  const commitObservedAt = useCallback(
    async (observedAt: number | null) => {
      if (!session?.id || !dateMenuMemory) return;
      setActionBusyId(dateMenuMemory.id);
      try {
        const updated = await setMemoryObservedAt(session.id, dateMenuMemory.index, observedAt);
        if (updated) setSession(updated);
      } finally {
        setActionBusyId(null);
        setDateMenuMemory(null);
      }
    },
    [session?.id, dateMenuMemory, setSession],
  );

  if (loading) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-base text-fg">
        <div className="flex items-center gap-3 text-sm text-fg/60">
          <Loader2 className="h-4 w-4 animate-spin" />
          {t("chats.companionMemoryPage.loading")}
        </div>
      </div>
    );
  }

  if (!characterId || !session || !character || error) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-base px-6">
        <div className={cn("w-full max-w-md border border-fg/10 bg-surface p-5 text-center", radius.lg)}>
          <p className="text-base font-semibold text-fg">{t("chats.companionMemoryPage.unavailable")}</p>
          <p className="mt-2 text-sm text-fg/60">{error || t("chats.companionMemoryPage.sessionLoadFailed")}</p>
          <button
            onClick={() => backOrReplace(characterId ? Routes.chatSession(characterId, sessionId) : Routes.chat)}
            className={cn("mt-4 inline-flex items-center justify-center px-4 py-2 text-sm text-fg", components.button.primary, "border border-fg/10 bg-fg/5")}
          >
            {t("chats.companionMemoryPage.backToChat")}
          </button>
        </div>
      </div>
    );
  }

  if (!isCompanionChat(character, session)) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-base px-6">
        <div className={cn("w-full max-w-lg border border-fg/10 bg-surface p-5", radius.lg)}>
          <p className="text-base font-semibold text-fg">{t("chats.companionMemoryPage.notCompanionTitle")}</p>
          <p className="mt-2 text-sm text-fg/60">
            {t("chats.companionMemoryPage.notCompanionDesc")}
          </p>
          <div className="mt-4 flex gap-3">
            <button
              onClick={() => go(Routes.chatMemories(character.id, session.id))}
              className={cn("px-4 py-2 text-sm text-fg", components.button.primary, "border border-fg/10 bg-fg/5")}
            >
              {t("chats.companionMemoryPage.openRegularMemories")}
            </button>
            <button
              onClick={() => backOrReplace(Routes.chatSession(character.id, session.id))}
              className={cn("px-4 py-2 text-sm text-fg/70", components.button.primary, "border border-fg/10 bg-transparent")}
            >
              {t("chats.companionMemoryPage.backToChat")}
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className={cn("flex h-full flex-col bg-base text-fg")}>
      <PageHeader
        title={t("chats.companionMemoryPage.pageTitle")}
        subtitle={session.title || character.name}
        onBack={() => backOrReplace(Routes.chatSession(character.id, session.id))}
        right={
          <button
            onClick={() => go(Routes.chatCompanionRelationship(character.id, session.id))}
            className={cn(
              "inline-flex items-center gap-1.5 rounded-md border border-fg/10 bg-fg/4 px-2.5 py-1.5 text-[11px] font-medium text-fg/70",
              "hover:border-fg/20 hover:bg-fg/8 hover:text-fg",
              interactive.transition.fast,
            )}
          >
            <Heart size={12} /> {t("chats.companionMemoryPage.relationship")}
          </button>
        }
      />

      <main className="flex-1 overflow-y-auto pb-[calc(env(safe-area-inset-bottom)+24px)]">
        {isDynamic &&
          (cycleStatus ||
            memoryCycleActive ||
            session.memoryError ||
            retrySuccess) && (
            <MemoryCycleHub
              status={cycleStatus}
              running={memoryCycleActive}
              retrying={retrying}
              retrySuccess={retrySuccess}
              errorMessage={session.memoryError ?? null}
              step={progressStep ?? session.memoryProgressStep ?? null}
              generationTokens={genTokens}
              generationTps={genTps}
              generationStalled={generationStalled}
              generationStalledSeconds={Math.round((generationStalledMs ?? 0) / 1000)}
              liveOutputAvailable={developerMode && genRecentText != null}
              onRun={() => void handleTriggerMemory()}
              onCancel={() => void handleCancelMemory()}
              onRetry={() => void handleRetryMemory()}
              onDismissError={() => void handleCancelMemory()}
              onDismissSuccess={() => setRetrySuccess(false)}
              onShowLiveOutput={() => setShowLiveOutput(true)}
              conversationScope={session.parentSessionId ? "branch" : "conversation"}
            />
          )}
        <div className="px-3 pt-4 lg:px-8">
          <motion.div
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.2, ease: "easeOut" }}
          className="mx-auto grid w-full max-w-7xl gap-6 xl:grid-cols-[minmax(280px,340px)_1fr] xl:items-start"
        >
          {/* Snapshot */}
          <section className="space-y-3 xl:sticky xl:top-4">
            <SectionLabel right={t("chats.companionMemoryPage.updatedAt", { time: formatRelativeTime(t, emotionalState?.updatedAt, effectiveNow) })}>
              {t("chats.companionMemoryPage.currentState")}
            </SectionLabel>
            <div className="grid grid-cols-2 gap-2 xl:grid-cols-2">
              <StatTile
                label={t("chats.companionMemoryPage.stats.closeness")}
                value={relationshipState?.closeness ?? companion?.relationshipDefaults?.closeness ?? 0.1}
                low={RELATIONSHIP_AXIS_ANCHORS.closeness.low}
                mid={RELATIONSHIP_AXIS_ANCHORS.closeness.mid}
                high={RELATIONSHIP_AXIS_ANCHORS.closeness.high}
                bipolar
              />
              <StatTile
                label={t("chats.companionMemoryPage.stats.trust")}
                value={relationshipState?.trust ?? companion?.relationshipDefaults?.trust ?? 0.1}
                low={RELATIONSHIP_AXIS_ANCHORS.trust.low}
                mid={RELATIONSHIP_AXIS_ANCHORS.trust.mid}
                high={RELATIONSHIP_AXIS_ANCHORS.trust.high}
                bipolar
              />
              <StatTile
                label={t("chats.companionMemoryPage.stats.affection")}
                value={relationshipState?.affection ?? companion?.relationshipDefaults?.affection ?? 0.05}
                tone="warm"
                low={RELATIONSHIP_AXIS_ANCHORS.affection.low}
                mid={RELATIONSHIP_AXIS_ANCHORS.affection.mid}
                high={RELATIONSHIP_AXIS_ANCHORS.affection.high}
                bipolar
              />
              <StatTile
                label={t("chats.companionMemoryPage.stats.tension")}
                value={relationshipState?.tension ?? companion?.relationshipDefaults?.tension ?? 0}
                tone="warning"
                low={RELATIONSHIP_AXIS_ANCHORS.tension.low}
                high={RELATIONSHIP_AXIS_ANCHORS.tension.high}
              />
            </div>

            {(topFelt.length > 0 || activeSignals.length > 0) && (
              <div className="mt-3 rounded-xl border border-fg/8 bg-fg/2 p-3">
                {topFelt.length > 0 && (
                  <div>
                    <div className="mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-fg/45">
                      {t("chats.companionMemoryPage.feltRightNow")}
                    </div>
                    <div className="flex flex-wrap gap-1.5">
                      {topFelt.map((entry) => (
                        <Pill key={entry.key} tone={entry.value >= 0.5 ? "accent" : "default"}>
                          {emotionLabel(entry.key)}
                          <span className="text-fg/40">{formatPercent(entry.value)}</span>
                        </Pill>
                      ))}
                    </div>
                  </div>
                )}
                {activeSignals.length > 0 && (
                  <div className={cn(topFelt.length > 0 && "mt-3")}>
                    <div className="mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-fg/45">
                      {t("chats.companionMemoryPage.activeDrivers")}
                    </div>
                    <div className="flex flex-wrap gap-1.5">
                      {activeSignals.map((signal) => (
                        <Pill key={signal} tone="muted">{signal}</Pill>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            )}

            <button
              type="button"
              onClick={() => setShowSummaryEditor(true)}
              className={cn(
                "w-full rounded-xl border border-emerald-400/22 bg-emerald-400/8 px-4 py-3 text-left",
                "transition-all hover:border-emerald-400/30 hover:bg-emerald-400/10 active:scale-[0.99]",
              )}
            >
              <div className="mb-1.5 flex items-center gap-2">
                <Sparkles size={13} className="shrink-0 text-emerald-500" />
                <span className="text-[11px] font-semibold uppercase tracking-wider text-emerald-600">
                  {t("chats.companionMemoryPage.contextSummary")}
                </span>
                {session?.memorySummaryTokenCount && session.memorySummaryTokenCount > 0 ? (
                  <span className="ml-auto text-[10px] text-fg/45">
                    {t("chats.companionMemoryPage.tokenCount", { count: session.memorySummaryTokenCount.toLocaleString() })}
                  </span>
                ) : null}
              </div>
              <p
                className={cn(
                  "min-h-14 text-[13px] leading-relaxed line-clamp-4",
                  summaryDraft ? "text-fg/78" : "italic text-fg/42",
                )}
              >
                {summaryDraft || t("chats.companionMemoryPage.addContextSummaryPrompt")}
              </p>
            </button>
          </section>

          {/* Memory store */}
          <section>
            <SectionLabel right={t("chats.companionMemoryPage.storeSummary", { total: counts.total, pinned: counts.pinned })}>
              {t("chats.companionMemoryPage.memoryStore")}
            </SectionLabel>

            {/* Search + Add row */}
            <div className="mb-3 flex items-center gap-2">
              <div className="relative min-w-0 flex-1">
                <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-fg/35" />
                <input
                  value={searchTerm}
                  onChange={(event) => setSearchTerm(event.target.value)}
                  placeholder={t("chats.companionMemoryPage.searchPlaceholder")}
                  className={cn(
                    "w-full py-2.5 pl-10 pr-9 text-sm text-fg placeholder:text-fg/35",
                    components.input.base,
                    radius.lg,
                  )}
                />
                {searchTerm && (
                  <button
                    type="button"
                    onClick={() => setSearchTerm("")}
                    className="absolute right-3 top-1/2 -translate-y-1/2 text-fg/35 hover:text-fg/70"
                    aria-label={t("common.buttons.clearSearch")}
                  >
                    <X size={14} />
                  </button>
                )}
              </div>
              <button
                onClick={() => setShowComposer((prev) => !prev)}
                className={cn(
                  "flex h-[42px] w-[42px] shrink-0 items-center justify-center rounded-lg border border-fg/10 bg-fg/4 text-fg/55",
                  "hover:bg-fg/8 hover:text-fg",
                  "transition-all active:scale-95",
                  showComposer && "border-accent/30 bg-accent/12 text-accent",
                )}
                aria-label={t("chats.addMemory")}
              >
                {showComposer ? <X size={18} /> : <Plus size={18} />}
              </button>
            </div>

            {/* Composer */}
            <AnimatePresence initial={false}>
              {showComposer && (
                <motion.div
                  key="composer"
                  initial={{ opacity: 0, height: 0 }}
                  animate={{ opacity: 1, height: "auto" }}
                  exit={{ opacity: 0, height: 0 }}
                  transition={{ duration: 0.18, ease: "easeOut" }}
                  className="mb-3 overflow-hidden"
                >
                  <div className="rounded-xl border border-fg/10 bg-fg/3 p-3 space-y-2">
                    <textarea
                      value={newMemory}
                      onChange={(event) => setNewMemory(event.target.value)}
                      placeholder={t("chats.companionMemoryPage.composerPlaceholder")}
                      className={cn(
                        components.input.base,
                        "min-h-[88px] w-full resize-y px-3 py-2 text-sm text-fg placeholder:text-fg/35",
                      )}
                    />
                    <div className="flex flex-wrap items-center gap-2">
                      <select
                        value={newCategory}
                        onChange={(event) => setNewCategory(normalizeCompanionCategory(event.target.value))}
                        className={cn(components.input.base, "px-3 py-2 text-xs text-fg")}
                      >
                        {COMPANION_CATEGORY_ORDER.map((category) => (
                          <option key={category} value={category}>
                            {companionCategoryLabel(t, category)}
                          </option>
                        ))}
                      </select>
                      <button
                        onClick={() => void handleAddMemory()}
                        disabled={composerBusy || !newMemory.trim()}
                        className={cn(
                          "ml-auto inline-flex items-center justify-center gap-1.5 rounded-md border border-accent/30 bg-accent/12 px-3 py-2 text-xs font-medium text-accent",
                          "hover:bg-accent/18 transition-all active:scale-[0.98]",
                          "disabled:pointer-events-none disabled:opacity-45",
                        )}
                      >
                        {composerBusy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Plus size={13} />}
                        {t("chats.companionMemoryPage.saveMemory")}
                      </button>
                    </div>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>

            {/* Filter chips */}
            <div className="mb-2 flex flex-wrap gap-1.5">
              {(["all", "active", "superseded"] as const).map((option) => (
                <FilterChip
                  key={option}
                  active={stateFilter === option}
                  onClick={() => setStateFilter(option)}
                >
                  {option === "all"
                    ? t("chats.companionMemoryPage.filters.allStates")
                    : option === "active"
                      ? t("chats.companionMemoryPage.filters.active", { count: counts.active })
                      : t("chats.companionMemoryPage.filters.superseded", { count: counts.superseded })}
                </FilterChip>
              ))}
            </div>
            <div className="mb-3 flex flex-wrap gap-1.5">
              <FilterChip
                active={categoryFilter === "all"}
                onClick={() => setCategoryFilter("all")}
              >
                {t("chats.companionMemoryPage.filters.all")}
              </FilterChip>
              {COMPANION_CATEGORY_ORDER.map((category) => {
                const count = memoryItems.filter((item) => item.category === category).length;
                if (!count && categoryFilter !== category) return null;
                return (
                  <FilterChip
                    key={category}
                    active={categoryFilter === category}
                    onClick={() => setCategoryFilter(category)}
                  >
                    {companionCategoryLabel(t, category)}
                    {count > 0 && <span className="ml-1 text-fg/35">{count}</span>}
                  </FilterChip>
                );
              })}
            </div>

            <div className="mb-2 flex items-center gap-2 text-[10px] text-fg/35">
              <span>{t("chats.companionMemoryPage.aiCount", { count: counts.ai })}</span>
              <span>·</span>
              <span>{t("chats.companionMemoryPage.youCount", { count: counts.user })}</span>
              {filteredItems.length !== memoryItems.length && (
                <>
                  <span className="ml-auto">{t("chats.companionMemoryPage.shownCount", { count: filteredItems.length })}</span>
                </>
              )}
            </div>

            {filteredItems.length === 0 ? (
              <motion.div
                initial={{ opacity: 0, y: 12 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.25 }}
                className="flex flex-col items-center justify-center py-14"
              >
                <div className="mb-4 flex h-14 w-14 items-center justify-center rounded-2xl border border-fg/10 bg-fg/4">
                  {searchTerm ? (
                    <Search className="h-6 w-6 text-fg/25" />
                  ) : (
                    <Sparkles className="h-6 w-6 text-fg/25" />
                  )}
                </div>
                <h3 className="mb-1 text-sm font-semibold text-fg/85">
                  {searchTerm ? t("chats.companionMemoryPage.noMatchingTitle") : t("chats.companionMemoryPage.emptyTitle")}
                </h3>
                <p className="max-w-sm text-center text-xs text-fg/45">
                  {searchTerm
                    ? t("chats.companionMemoryPage.noMatchingDesc")
                    : t("chats.companionMemoryPage.emptyDesc")}
                </p>
              </motion.div>
            ) : (
              <motion.div
                className="space-y-2"
                initial="hidden"
                animate="visible"
                variants={{ visible: { transition: { staggerChildren: 0.025 } } }}
              >
                <AnimatePresence>
                  {filteredItems.map((memory) => (
                    <MemoryCard
                      key={memory.id}
                      memory={memory}
                      expanded={expandedIds.has(memory.id) || editingId === memory.id}
                      editing={editingId === memory.id}
                      editValue={editingId === memory.id ? editingValue : memory.text}
                      editCategory={editingId === memory.id ? editingCategory : memory.category}
                      saving={actionBusyId === memory.id}
                      actionBusy={actionBusyId === memory.id}
                      effectiveNow={effectiveNow}
                      onToggleExpand={() => toggleExpand(memory.id)}
                      onStartEdit={startEdit}
                      onEditValue={setEditingValue}
                      onEditCategory={setEditingCategory}
                      onSaveEdit={(item) => void saveEdit(item)}
                      onCancelEdit={cancelEdit}
                      onTogglePin={(item) => void handleTogglePin(item)}
                      onToggleCold={(item) => void handleToggleCold(item)}
                      onEditDate={(item) => startEditDate(item)}
                      onDelete={(item) => void handleDeleteMemory(item)}
                    />
                  ))}
                </AnimatePresence>
              </motion.div>
            )}
          </section>
          </motion.div>
        </div>
      </main>

      <BottomMenu
        isOpen={showSummaryEditor}
        onClose={() => setShowSummaryEditor(false)}
        title={t("chats.companionMemoryPage.contextSummary")}
      >
        <div className="space-y-4 text-fg">
          <textarea
            value={summaryDraft}
            onChange={(event) => setSummaryDraft(event.target.value)}
            rows={6}
            className={cn(
              "w-full p-3",
              radius.lg,
              "border border-fg/10 bg-surface-el/40",
              "resize-none text-sm leading-relaxed text-fg/90",
              "focus:border-fg/20 focus:outline-none focus:ring-1 focus:ring-fg/10",
              "placeholder:text-fg/30",
            )}
            placeholder={t("chats.companionMemoryPage.summaryEditorPlaceholder")}
            autoFocus
          />
          {session?.memorySummaryTokenCount && session.memorySummaryTokenCount > 0 ? (
            <p className="text-[10px] text-fg/30">
              {t("chats.companionMemoryPage.tokenCount", { count: session.memorySummaryTokenCount.toLocaleString() })}
            </p>
          ) : null}
          <div className="flex gap-2">
            <button
              onClick={() => {
                setSummaryDraft(session?.memorySummary ?? "");
                setShowSummaryEditor(false);
              }}
              className={cn(
                "flex-1 px-4 py-2.5",
                radius.lg,
                "border border-fg/10 bg-fg/5 text-sm font-medium text-fg/60",
                "transition-all hover:border-fg/15 hover:bg-fg/8 hover:text-fg/80 active:scale-[0.98]",
              )}
            >
              {t("common.buttons.cancel")}
            </button>
            <button
              onClick={() => void handleSaveSummary()}
              disabled={savingSummary || summaryDraft === (session?.memorySummary ?? "")}
              className={cn(
                "flex-1 px-4 py-2.5 flex items-center justify-center gap-2",
                radius.lg,
                "border border-emerald-400/30 bg-emerald-500/15 text-sm font-semibold text-emerald-200",
                "transition-all hover:border-emerald-400/50 hover:bg-emerald-500/25 active:scale-[0.98]",
                "disabled:pointer-events-none disabled:opacity-40",
              )}
            >
              {savingSummary ? t("common.buttons.saving") : t("common.buttons.save")}
            </button>
          </div>
        </div>
      </BottomMenu>

      <BottomMenu
        isOpen={showLiveOutput}
        onClose={() => setShowLiveOutput(false)}
        title={t("chats.companionMemoryPage.liveOutputTitle")}
      >
        <div className="space-y-2">
          <div className="flex items-center gap-2 text-[11px] text-fg/45">
            {memoryCycleActive ? (
              <>
                <RefreshCw className="h-3 w-3 animate-spin text-blue-400" />
                <span>
                  {t("chats.companionMemoryPage.generating")}
                  {genTokens != null ? ` · ${t("chats.companionMemoryPage.tokensSuffix", { count: genTokens })}` : ""}
                  {genTps && genTps > 0 ? ` · ${t("chats.companionMemoryPage.tokensPerSecond", { tps: genTps.toFixed(1) })}` : ""}
                </span>
              </>
            ) : (
              <span>{t("chats.companionMemoryPage.generationFinished")}</span>
            )}
          </div>
          <pre
            ref={liveOutputRef}
            className="max-h-[55vh] overflow-y-auto whitespace-pre-wrap break-words rounded-lg border border-fg/10 bg-black/40 p-3 font-mono text-[11px] leading-relaxed text-fg/75"
          >
            {genRecentText || t("chats.companionMemoryPage.waitingForOutput")}
          </pre>
        </div>
      </BottomMenu>

      <BottomMenu
        isOpen={dateMenuMemory !== null}
        onClose={() => setDateMenuMemory(null)}
        title={t("chats.companionMemoryPage.memoryDateTitle")}
      >
        <div className="space-y-4 text-fg">
          <p className="text-xs text-fg/50">
            {t("chats.companionMemoryPage.memoryDateDesc")}
          </p>
          <DateTimePicker valueMs={dateDraftMs} onChange={setDateDraftMs} />
          <div className="flex gap-2">
            {dateMenuMemory?.observedAt != null ? (
              <button
                onClick={() => void commitObservedAt(null)}
                className={cn(
                  "px-4 py-2.5",
                  radius.lg,
                  "border border-fg/10 bg-fg/5 text-sm font-medium text-fg/60",
                  "transition-all hover:border-fg/15 hover:bg-fg/8 hover:text-fg/80 active:scale-[0.98]",
                )}
              >
                {t("chats.authorNote.clear")}
              </button>
            ) : null}
            <button
              onClick={() => setDateMenuMemory(null)}
              className={cn(
                "flex-1 px-4 py-2.5",
                radius.lg,
                "border border-fg/10 bg-fg/5 text-sm font-medium text-fg/60",
                "transition-all hover:border-fg/15 hover:bg-fg/8 hover:text-fg/80 active:scale-[0.98]",
              )}
            >
              {t("common.buttons.cancel")}
            </button>
            <button
              onClick={() => void commitObservedAt(dateDraftMs)}
              className={cn(
                "flex-1 px-4 py-2.5 flex items-center justify-center gap-2",
                radius.lg,
                "border border-emerald-400/30 bg-emerald-500/15 text-sm font-semibold text-emerald-200",
                "transition-all hover:border-emerald-400/50 hover:bg-emerald-500/25 active:scale-[0.98]",
              )}
            >
              {t("common.buttons.save")}
            </button>
          </div>
        </div>
      </BottomMenu>
    </div>
  );
}

function FilterChip({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "rounded-full border px-2.5 py-1 text-[11px] font-medium transition-colors",
        active
          ? "border-fg/20 bg-fg/12 text-fg/85"
          : "border-fg/8 bg-fg/3 text-fg/45 hover:bg-fg/6 hover:text-fg/70",
      )}
    >
      {children}
    </button>
  );
}
