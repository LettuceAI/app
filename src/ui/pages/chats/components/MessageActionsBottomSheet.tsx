import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  Edit3,
  Copy,
  RotateCcw,
  Trash2,
  Bug,
  Pin,
  PinOff,
  Brain,
  BookOpen,
  GitBranch,
  Users,
  Paintbrush,
  Image,
  HeartPulse,
  type LucideIcon,
} from "lucide-react";
import { BottomMenu } from "../../../components/BottomMenu";
import type {
  StoredMessage,
  Settings,
  Model,
  ImageAttachment,
  CompanionTurnEffect,
} from "../../../../core/storage/schemas";
import { cn, radius } from "../../../design-tokens";
import { getMessageCompanionEffect, readSettings } from "../../../../core/storage/repo";
import { useI18n } from "../../../../core/i18n/context";
import { isDevelopmentMode } from "../../../../core/utils/env";
import { useSessionAttachments } from "../../../hooks/useSessionAttachment";

interface MessageActionState {
  message: StoredMessage;
  mode: "view" | "edit";
}

interface MessageActionsBottomSheetProps {
  messageAction: MessageActionState | null;
  actionError: string | null;
  actionStatus: string | null;
  actionBusy: boolean;
  editDraft: string;
  messages: StoredMessage[];
  setEditDraft: (value: string) => void;
  closeMessageActions: (force?: boolean) => void;
  setActionError: (value: string | null) => void;
  setActionStatus: (value: string | null) => void;
  handleSaveEdit: (attachments?: ImageAttachment[]) => Promise<void>;
  handleDeleteMessage: (message: StoredMessage) => Promise<void>;
  handleRewindToMessage: (message: StoredMessage) => Promise<void>;
  handleBranchFromMessage: (message: StoredMessage) => Promise<string | null>;
  onBranchToCharacter: (message: StoredMessage) => void;
  onBranchToGroupChat: (message: StoredMessage) => void;
  handleTogglePin: (message: StoredMessage) => Promise<void>;
  setMessageAction: (value: MessageActionState | null) => void;
  onOpenSceneImageFlow: (message: StoredMessage) => void;
  onOpenChatAppearance?: () => void;
  hasSceneImage?: boolean;
  sceneGenerationEnabled?: boolean;
  characterMemoryType?: string | null;
  characterDefaultModelId?: string | null;
  characterId?: string;
  sessionId?: string | null;
  isCompanionChat?: boolean;
}

function formatEffectKey(key: string): string {
  return key
    .replace(/[_-]+/g, " ")
    .replace(/\b\w/g, (char) => char.toUpperCase())
    .trim();
}

function formatDelta(value: number): string {
  const percent = value * 100;
  const sign = percent > 0 ? "+" : "";
  return `${sign}${percent.toFixed(Math.abs(percent) >= 10 ? 0 : 1)}%`;
}

function flattenEmotionDeltas(effect: CompanionTurnEffect): Array<{
  key: string;
  value: number;
}> {
  return Object.entries(effect.emotionDelta).flatMap(([target, deltas]) =>
    Object.entries(deltas).map(([emotion, value]) => ({
      key: target === "companion" ? emotion : `${target} ${emotion}`,
      value,
    })),
  );
}

function getMemoryChangeCount(effect: CompanionTurnEffect): number {
  return (
    effect.memoryChanges.added.length +
    effect.memoryChanges.updated.length +
    effect.memoryChanges.superseded.length
  );
}

function getMemoryPreview(value: unknown): string | null {
  if (!value || typeof value !== "object") return null;
  const record = value as Record<string, unknown>;
  const candidates = [record.text, record.content, record.summary, record.canonicalText];
  const text = candidates.find((candidate): candidate is string => typeof candidate === "string");
  return text ? text.trim() : null;
}

function DeltaPill({ label, value }: { label: string; value: number }) {
  const positive = value > 0;
  const neutral = Math.abs(value) < 0.0001;
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-full border px-2 py-1 text-[11px] font-medium",
        neutral && "border-white/10 bg-white/5 text-white/55",
        !neutral &&
          positive &&
          "border-emerald-400/20 bg-emerald-400/10 text-emerald-200",
        !neutral && !positive && "border-rose-400/20 bg-rose-400/10 text-rose-200",
      )}
    >
      <span className="text-white/60">{label}</span>
      <span className="tabular-nums">{formatDelta(value)}</span>
    </span>
  );
}

// Action row component
function ActionRow({
  icon: Icon,
  label,
  onClick,
  disabled = false,
  variant = "default",
  iconBg,
}: {
  icon: LucideIcon;
  label: string;
  onClick: () => void;
  disabled?: boolean;
  variant?: "default" | "danger";
  iconBg?: string;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "flex w-full items-center gap-3 px-1 py-2.5 transition-all rounded-lg",
        "hover:bg-white/5 active:bg-white/10",
        "disabled:opacity-40 disabled:pointer-events-none",
        variant === "danger" && "hover:bg-red-500/10",
      )}
    >
      <div
        className={cn(
          "flex items-center justify-center w-8 h-8 rounded-lg",
          iconBg || "bg-white/10",
        )}
      >
        <Icon size={16} className={cn(variant === "danger" ? "text-red-400" : "text-white")} />
      </div>
      <span
        className={cn(
          "text-[15px] text-left",
          variant === "danger" ? "text-red-400" : "text-white/90",
        )}
      >
        {label}
      </span>
    </button>
  );
}

export function MessageActionsBottomSheet({
  messageAction,
  actionError,
  actionStatus,
  actionBusy,
  editDraft,
  messages,
  setEditDraft,
  closeMessageActions,
  setActionError,
  setActionStatus,
  handleSaveEdit,
  handleDeleteMessage,
  handleRewindToMessage,
  handleBranchFromMessage,
  onBranchToCharacter,
  onBranchToGroupChat,
  handleTogglePin,
  setMessageAction,
  onOpenSceneImageFlow,
  onOpenChatAppearance,
  hasSceneImage = false,
  sceneGenerationEnabled = false,
  characterMemoryType,
  characterDefaultModelId,
  characterId,
  sessionId,
  isCompanionChat = false,
}: MessageActionsBottomSheetProps) {
  const navigate = useNavigate();
  const { t } = useI18n();
  const [settings, setSettings] = useState<Settings | null>(null);
  const [modelName, setModelName] = useState<string | null>(null);
  const [editAttachments, setEditAttachments] = useState<ImageAttachment[]>([]);
  const [editingAttachmentId, setEditingAttachmentId] = useState<string | null>(null);
  const [companionEffect, setCompanionEffect] = useState<CompanionTurnEffect | null>(null);
  const [companionEffectLoading, setCompanionEffectLoading] = useState(false);
  const [companionEffectError, setCompanionEffectError] = useState<string | null>(null);
  const isSceneMessage = messageAction?.message.role === "scene";
  const isVisibleSystemMessage =
    messageAction?.message.role === "system" && Boolean(messageAction.message.visibleInChat);
  const isAssistantLikeMessage =
    messageAction?.message.role === "assistant" || messageAction?.message.role === "scene";
  const canDeleteMessage = !isSceneMessage || isCompanionChat;
  const companionEffectMessageId =
    isCompanionChat && sessionId && messageAction?.message.role === "assistant"
      ? messageAction.message.id
      : null;

  const canEdit =
    isAssistantLikeMessage ||
    isVisibleSystemMessage ||
    (() => {
      const userMessages = messages.filter(
        (m) => m.role === "user" && !m.id.startsWith("placeholder"),
      );
      const latestUserMessage = userMessages[userMessages.length - 1];
      return latestUserMessage?.id === messageAction?.message.id;
    })();

  useEffect(() => {
    readSettings().then(setSettings).catch(console.error);
  }, []);

  useEffect(() => {
    if (messageAction?.mode === "edit") {
      setEditAttachments([...(messageAction.message.attachments ?? [])]);
    } else if (!messageAction) {
      setEditAttachments([]);
      setEditingAttachmentId(null);
    }
  }, [messageAction]);

  useEffect(() => {
    const messageModelId = messageAction?.message.modelId ?? null;
    const resolvedModelId =
      messageModelId ?? characterDefaultModelId ?? settings?.defaultModelId ?? null;

    if (resolvedModelId && settings) {
      const model = settings.models.find((m: Model) => m.id === resolvedModelId);
      setModelName(model ? model.displayName : resolvedModelId);
    } else {
      setModelName(null);
    }
  }, [messageAction, settings, characterDefaultModelId]);

  useEffect(() => {
    let cancelled = false;
    let retryTimer: number | null = null;

    const clearRetry = () => {
      if (retryTimer !== null) {
        window.clearTimeout(retryTimer);
        retryTimer = null;
      }
    };

    if (!sessionId || !companionEffectMessageId) {
      setCompanionEffect(null);
      setCompanionEffectLoading(false);
      setCompanionEffectError(null);
      return clearRetry;
    }

    const loadEffect = async (showLoading: boolean) => {
      clearRetry();
      if (showLoading) {
        setCompanionEffectLoading(true);
      }
      try {
        const effect = await getMessageCompanionEffect(sessionId, companionEffectMessageId);
        if (cancelled) return;
        setCompanionEffect(effect);
        setCompanionEffectError(null);
        if (effect?.status === "processing") {
          retryTimer = window.setTimeout(() => void loadEffect(false), 1500);
        }
      } catch (error) {
        if (cancelled) return;
        setCompanionEffectError(error instanceof Error ? error.message : String(error));
      } finally {
        if (!cancelled) {
          setCompanionEffectLoading(false);
        }
      }
    };

    void loadEffect(true);

    return () => {
      cancelled = true;
      clearRetry();
    };
  }, [sessionId, companionEffectMessageId]);

  const modelLabel =
    modelName ?? (settings ? t("chats.actions.unknownModel") : t("chats.actions.loadingModel"));
  const usedLorebookEntries = messageAction?.message.usedLorebookEntries ?? [];
  const firstTokenMs = messageAction?.message.usage?.firstTokenMs;
  const tokensPerSecond = messageAction?.message.usage?.tokensPerSecond;
  const loadedEditAttachments = useSessionAttachments(editAttachments);
  const editingAttachment =
    loadedEditAttachments.find((attachment) => attachment.id === editingAttachmentId) ?? null;
  const canOpenDebug =
    isDevelopmentMode() &&
    Boolean(characterId && sessionId) &&
    messageAction?.message.role === "assistant";
  const relationshipDeltas = companionEffect
    ? Object.entries(companionEffect.relationshipDelta).filter(([, value]) => Math.abs(value) > 0)
    : [];
  const emotionDeltas = companionEffect
    ? flattenEmotionDeltas(companionEffect).filter(({ value }) => Math.abs(value) > 0)
    : [];
  const memoryChangeCount = companionEffect ? getMemoryChangeCount(companionEffect) : 0;
  const showCompanionEffectCard =
    isCompanionChat &&
    messageAction?.mode === "view" &&
    messageAction.message.role === "assistant" &&
    (companionEffectLoading || Boolean(companionEffect) || Boolean(companionEffectError));

  const handleCopy = async () => {
    if (!messageAction) return;
    try {
      await navigator.clipboard?.writeText(messageAction.message.content);
      setActionStatus(t("common.buttons.copied"));
      setTimeout(() => setActionStatus(null), 1500);
    } catch (copyError) {
      setActionError(copyError instanceof Error ? copyError.message : String(copyError));
    }
  };

  return (
    <>
      <BottomMenu
        isOpen={Boolean(messageAction)}
        includeExitIcon={false}
        onClose={() => closeMessageActions(true)}
        title={
          isSceneMessage
            ? t("chats.message.sceneLabel")
            : isAssistantLikeMessage
              ? t("chats.actions.assistantMessage")
              : isVisibleSystemMessage
                ? t("chats.actions.systemMessage")
                : t("chats.actions.userMessage")
        }
      >
        {messageAction && (
          <div className="text-white">
          {/* Token usage */}
          {!isSceneMessage && messageAction.message.usage && (
            <div className="mb-4 space-y-2">
              <div className="flex items-center gap-x-3 text-xs text-white/40">
                <div className="flex items-center gap-2 border-r border-white/10 pr-3">
                  <span title={t("chats.actions.promptTokens")}>
                    ↓{messageAction.message.usage.promptTokens ?? 0}
                  </span>
                  <span title={t("chats.actions.completionTokens")}>
                    ↑{messageAction.message.usage.completionTokens ?? 0}
                  </span>
                </div>
                <div className="flex-1">
                  <span className="inline-flex items-center gap-1 text-white/60">
                    <span>{modelLabel}</span>
                  </span>
                </div>
                <div className="tabular-nums">
                  {(messageAction.message.usage.totalTokens ?? 0).toLocaleString()}{" "}
                  <span className="text-[12px] uppercase opacity-50">
                    {t("chats.actions.total")}
                  </span>
                </div>
              </div>
              {(typeof firstTokenMs === "number" || typeof tokensPerSecond === "number") && (
                  <div className="flex items-center gap-3 text-[11px] text-white/45 tabular-nums">
                    {typeof firstTokenMs === "number" && (
                      <span title={t("chats.actions.timeToFirstToken")}>TTFT {firstTokenMs}ms</span>
                    )}
                    {typeof tokensPerSecond === "number" && (
                      <span title={t("chats.actions.completionTokenSpeed")}>
                        {tokensPerSecond.toFixed(1)} tok/s
                      </span>
                    )}
                  </div>
                )}
            </div>
          )}

          {/* Status messages */}
          {actionStatus && (
            <div className="mb-3 px-3 py-2 rounded-lg border border-emerald-400/20 bg-emerald-400/10">
              <p className="text-sm text-emerald-200">{actionStatus}</p>
            </div>
          )}
          {actionError && (
            <div className="mb-3 px-3 py-2 rounded-lg border border-red-400/20 bg-red-400/10">
              <p className="text-sm text-red-200">{actionError}</p>
            </div>
          )}

          {messageAction.mode === "view" ? (
            <div className="space-y-1">
              {showCompanionEffectCard && (
                <div className="mb-3 rounded-xl border border-amber-400/20 bg-amber-400/[0.08] p-3">
                  <div className="mb-2 flex items-start gap-2">
                    <div className="mt-0.5 flex h-7 w-7 items-center justify-center rounded-lg bg-amber-400/15">
                      <HeartPulse size={15} className="text-amber-200" />
                    </div>
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center justify-between gap-3">
                        <span className="text-xs font-semibold text-amber-100">
                          {t("chats.actions.companionChanges")}
                        </span>
                        {companionEffect?.status === "processing" && (
                          <span className="text-[10px] uppercase tracking-[0.18em] text-amber-200/55">
                            {t("chats.actions.companionProcessing")}
                          </span>
                        )}
                        {companionEffect?.status === "failed" && (
                          <span className="text-[10px] uppercase tracking-[0.18em] text-rose-200/65">
                            {t("chats.actions.companionFailed")}
                          </span>
                        )}
                      </div>
                      <p className="mt-1 text-xs leading-relaxed text-amber-50/70">
                        {companionEffectError
                          ? t("chats.actions.companionLoadError")
                          : companionEffect?.status === "failed"
                            ? t("chats.actions.companionAnalysisFailed")
                          : companionEffect?.summary ||
                            (companionEffectLoading || companionEffect?.status === "processing"
                              ? t("chats.actions.companionAnalyzing")
                              : t("chats.actions.companionNoChanges"))}
                      </p>
                    </div>
                  </div>

                  {companionEffect && companionEffect.status === "ready" && (
                    <div className="space-y-2">
                      {relationshipDeltas.length > 0 && (
                        <div>
                          <div className="mb-1.5 text-[10px] font-semibold uppercase tracking-[0.18em] text-white/35">
                            {t("chats.actions.companionRelationship")}
                          </div>
                          <div className="flex flex-wrap gap-1.5">
                            {relationshipDeltas.map(([key, value]) => (
                              <DeltaPill key={key} label={formatEffectKey(key)} value={value} />
                            ))}
                          </div>
                        </div>
                      )}

                      {emotionDeltas.length > 0 && (
                        <div>
                          <div className="mb-1.5 text-[10px] font-semibold uppercase tracking-[0.18em] text-white/35">
                            {t("chats.actions.companionEmotions")}
                          </div>
                          <div className="flex flex-wrap gap-1.5">
                            {emotionDeltas.map(({ key, value }) => (
                              <DeltaPill key={key} label={formatEffectKey(key)} value={value} />
                            ))}
                          </div>
                        </div>
                      )}

                      {(companionEffect.signalChanges.added.length > 0 ||
                        companionEffect.signalChanges.removed.length > 0) && (
                        <div className="grid gap-1.5 text-xs text-white/65">
                          {companionEffect.signalChanges.added.length > 0 && (
                            <div>
                              <span className="text-emerald-200/80">
                                {t("chats.actions.companionSignalsAdded")}{" "}
                              </span>
                              {companionEffect.signalChanges.added
                                .map(formatEffectKey)
                                .join(", ")}
                            </div>
                          )}
                          {companionEffect.signalChanges.removed.length > 0 && (
                            <div>
                              <span className="text-rose-200/80">
                                {t("chats.actions.companionSignalsRemoved")}{" "}
                              </span>
                              {companionEffect.signalChanges.removed
                                .map(formatEffectKey)
                                .join(", ")}
                            </div>
                          )}
                        </div>
                      )}

                      {memoryChangeCount > 0 && (
                        <div className="rounded-lg border border-white/10 bg-black/20 p-2">
                          <div className="mb-1 text-[11px] font-medium text-white/70">
                            {t("chats.actions.companionMemoryChanges", {
                              count: memoryChangeCount,
                            })}
                          </div>
                          <div className="flex flex-wrap gap-1.5 text-[11px] text-white/55">
                            {companionEffect.memoryChanges.added.length > 0 && (
                              <span>
                                {t("chats.actions.companionMemoryAdded", {
                                  count: companionEffect.memoryChanges.added.length,
                                })}
                              </span>
                            )}
                            {companionEffect.memoryChanges.updated.length > 0 && (
                              <span>
                                {t("chats.actions.companionMemoryUpdated", {
                                  count: companionEffect.memoryChanges.updated.length,
                                })}
                              </span>
                            )}
                            {companionEffect.memoryChanges.superseded.length > 0 && (
                              <span>
                                {t("chats.actions.companionMemorySuperseded", {
                                  count: companionEffect.memoryChanges.superseded.length,
                                })}
                              </span>
                            )}
                          </div>
                          {companionEffect.memoryChanges.added
                            .map(getMemoryPreview)
                            .filter((text): text is string => Boolean(text))
                            .slice(0, 2)
                            .map((text, index) => (
                              <p
                                key={`${text}-${index}`}
                                className="mt-1.5 line-clamp-2 text-xs leading-relaxed text-white/70"
                              >
                                {text}
                              </p>
                            ))}
                        </div>
                      )}
                    </div>
                  )}
                </div>
              )}

              {/* Memories section */}
              {!isSceneMessage &&
                characterMemoryType === "dynamic" &&
                (messageAction.message.memoryRefs?.length ?? 0) > 0 && (
                  <div className="mb-3 p-3 rounded-lg border border-emerald-500/20 bg-emerald-500/10">
                    <div className="flex items-center gap-2 mb-2">
                      <Brain size={14} className="text-emerald-400" />
                      <span className="text-xs font-medium text-emerald-300">
                        {t("chats.actions.memoriesUsed", {
                          count: messageAction.message.memoryRefs?.length ?? 0,
                        })}
                      </span>
                    </div>
                    <div className="space-y-2 max-h-64 overflow-y-auto pr-1">
                      {(messageAction.message.memoryRefs || []).map((ref, idx) => {
                        const match = ref.match(/^(\d+(\.\d+)?)::(.*)$/);
                        const score = match ? parseFloat(match[1]) : null;
                        const text = match ? match[3] : ref;
                        return (
                          <div
                            key={idx}
                            className="bg-black/20 rounded p-2 text-xs border border-emerald-500/10"
                          >
                            {score !== null && (
                              <div className="text-[10px] font-bold text-emerald-400 mb-1">
                                {t("chats.actions.matchScore", { score: (score * 100).toFixed(0) })}
                              </div>
                            )}
                            <div className="text-emerald-100/90 leading-relaxed whitespace-pre-wrap">
                              {text}
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  </div>
                )}

              {!isSceneMessage && usedLorebookEntries.length > 0 && (
                <div className="mb-3 p-3 rounded-lg border border-sky-500/20 bg-sky-500/10">
                  <div className="flex items-center gap-2 mb-2">
                    <BookOpen size={14} className="text-sky-300" />
                    <span className="text-xs font-medium text-sky-200">
                      {t("chats.actions.lorebookUsage")}
                    </span>
                  </div>
                  <p className="text-xs text-sky-100/90 mb-2">
                    {t("chats.actions.lorebookUsageDesc")}
                  </p>
                  <div className="space-y-1">
                    {usedLorebookEntries.map((entry, idx) => (
                      <div
                        key={`${entry}-${idx}`}
                        className="text-xs text-sky-100/85 rounded bg-black/20 border border-sky-500/10 px-2 py-1.5"
                      >
                        {entry}
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* Basic actions */}
              {canEdit && (
                <ActionRow
                  icon={Edit3}
                  label={t("chats.actions.edit")}
                  iconBg="bg-blue-500/20"
                  onClick={() => {
                    setActionError(null);
                    setActionStatus(null);
                    setMessageAction({ message: messageAction.message, mode: "edit" });
                    setEditDraft(messageAction.message.content);
                    setEditAttachments([...(messageAction.message.attachments ?? [])]);
                    setEditingAttachmentId(null);
                  }}
                />
              )}

              {!isSceneMessage && (
                <ActionRow
                  icon={Copy}
                  label={t("chats.actions.copy")}
                  iconBg="bg-violet-500/20"
                  onClick={() => void handleCopy()}
                />
              )}

              {!isSceneMessage && (
                <ActionRow
                  icon={messageAction.message.isPinned ? PinOff : Pin}
                  label={
                    messageAction.message.isPinned
                      ? t("chats.actions.unpin")
                      : t("chats.actions.pin")
                  }
                  iconBg="bg-amber-500/20"
                  onClick={() => void handleTogglePin(messageAction.message)}
                  disabled={actionBusy}
                />
              )}

              <div className="h-px bg-white/5 my-2" />

              {/* Chat flow actions */}
              {(messageAction.message.role === "assistant" ||
                messageAction.message.role === "scene" ||
                messageAction.message.role === "user" ||
                isVisibleSystemMessage) && (
                <ActionRow
                  icon={RotateCcw}
                  label={t("chats.actions.rewindToHere")}
                  iconBg="bg-cyan-500/20"
                  onClick={() => void handleRewindToMessage(messageAction.message)}
                  disabled={actionBusy}
                />
              )}

              {sceneGenerationEnabled &&
                (messageAction.message.role === "assistant" ||
                  messageAction.message.role === "scene" ||
                  messageAction.message.role === "user") && (
                  <ActionRow
                    icon={Image}
                    label={
                      hasSceneImage
                        ? t("chats.actions.regenerateSceneImage")
                        : t("chats.actions.generateSceneImage")
                    }
                    iconBg="bg-emerald-500/20"
                    onClick={() => onOpenSceneImageFlow(messageAction.message)}
                    disabled={actionBusy}
                  />
                )}

              {!isSceneMessage && (
                <ActionRow
                  icon={GitBranch}
                  label={t("chats.actions.branchFromHere")}
                  iconBg="bg-emerald-500/20"
                  onClick={() => void handleBranchFromMessage(messageAction.message)}
                  disabled={actionBusy}
                />
              )}

              {!isSceneMessage && (
                <ActionRow
                  icon={Users}
                  label={t("chats.actions.branchToGroupChat")}
                  iconBg="bg-rose-500/20"
                  onClick={() => onBranchToGroupChat(messageAction.message)}
                  disabled={actionBusy}
                />
              )}

              {!isSceneMessage && (
                <ActionRow
                  icon={Users}
                  label={t("chats.actions.branchToCharacter")}
                  iconBg="bg-pink-500/20"
                  onClick={() => onBranchToCharacter(messageAction.message)}
                  disabled={actionBusy}
                />
              )}

              {!isSceneMessage && <div className="h-px bg-white/5 my-2" />}

              {canDeleteMessage && (
                <ActionRow
                  icon={Trash2}
                  label={
                    messageAction.message.isPinned
                      ? t("chats.actions.unpinToDelete")
                      : t("chats.actions.delete")
                  }
                  onClick={() => void handleDeleteMessage(messageAction.message)}
                  disabled={actionBusy || messageAction.message.isPinned}
                  variant="danger"
                />
              )}

              {!isSceneMessage && characterId && (
                <ActionRow
                  icon={Paintbrush}
                  label={t("chats.actions.chatAppearance")}
                  iconBg="bg-purple-500/20"
                  onClick={() => {
                    closeMessageActions(true);
                    if (onOpenChatAppearance) {
                      onOpenChatAppearance();
                      return;
                    }
                    navigate(`/settings/customization/chat?characterId=${characterId}`);
                  }}
                />
              )}

              {canOpenDebug && messageAction && characterId && sessionId && (
                <ActionRow
                  icon={Bug}
                  label={t("chats.actions.debug")}
                  iconBg="bg-white/10"
                  onClick={() => {
                    closeMessageActions(true);
                    navigate(`/chat/${characterId}/debug/${sessionId}/${messageAction.message.id}`);
                  }}
                />
              )}
            </div>
          ) : (
            <div className="space-y-4">
              <textarea
                value={editDraft}
                onChange={(event) => setEditDraft(event.target.value)}
                rows={14}
                className={cn(
                  "min-h-90 w-full p-3 text-sm text-white placeholder-white/40",
                  "border border-white/10 bg-black/30",
                  "focus:border-white/20 focus:outline-none resize-none",
                  radius.lg,
                )}
                placeholder={t("chats.actions.editPlaceholder")}
                disabled={actionBusy}
                autoFocus
              />
              {loadedEditAttachments.length > 0 && (
                <div className="space-y-2">
                  <div className="flex items-center justify-between">
                    <div className="text-[11px] font-semibold uppercase tracking-[0.22em] text-white/45">
                      {t("chats.actions.attachments")}
                    </div>
                    <div className="text-[11px] tabular-nums text-white/35">
                      {loadedEditAttachments.length}
                    </div>
                  </div>
                  <div className="-mx-1 flex gap-2.5 overflow-x-auto px-1 pb-1">
                    {loadedEditAttachments.map((attachment) => (
                      <button
                        key={attachment.id}
                        type="button"
                        onClick={() => setEditingAttachmentId(attachment.id)}
                        className={cn(
                          "relative block h-28 w-28 shrink-0 overflow-hidden border border-white/10 bg-black/30 transition",
                          "hover:border-white/25 focus:outline-none focus-visible:border-white/30",
                          radius.lg,
                        )}
                        aria-label={attachment.filename || t("chats.message.attachedImage")}
                      >
                        {attachment.data ? (
                          <img
                            src={attachment.data}
                            alt={attachment.filename || t("chats.message.attachedImage")}
                            className="h-full w-full object-cover"
                          />
                        ) : (
                          <div className="flex h-full w-full items-center justify-center">
                            <div className="h-5 w-5 animate-spin rounded-full border-2 border-white/20 border-t-white/60" />
                          </div>
                        )}
                      </button>
                    ))}
                  </div>
                </div>
              )}
              <div className="flex gap-3">
                <button
                  onClick={() => {
                    setActionError(null);
                    setActionStatus(null);
                    setMessageAction({ message: messageAction.message, mode: "view" });
                    setEditDraft(messageAction.message.content);
                    setEditAttachments([...(messageAction.message.attachments ?? [])]);
                    setEditingAttachmentId(null);
                  }}
                  className={cn(
                    "flex-1 px-4 py-3 text-sm font-medium text-white/70 transition",
                    "border border-white/10 bg-white/5",
                    "hover:bg-white/10 hover:text-white",
                    "active:scale-[0.98]",
                    radius.lg,
                  )}
                >
                  {t("common.buttons.cancel")}
                </button>
                <button
                  onClick={() => void handleSaveEdit(editAttachments)}
                  disabled={actionBusy}
                  className={cn(
                    "flex-1 px-4 py-3 text-sm font-semibold text-white transition",
                    "bg-emerald-500",
                    "hover:bg-emerald-400",
                    "active:scale-[0.98]",
                    "disabled:cursor-not-allowed disabled:opacity-50",
                    radius.lg,
                  )}
                >
                  {actionBusy ? t("common.buttons.saving") : t("common.buttons.save")}
                </button>
              </div>
            </div>
          )}
        </div>
        )}
      </BottomMenu>

      <BottomMenu
        isOpen={Boolean(editingAttachment)}
        includeExitIcon={false}
        onClose={() => setEditingAttachmentId(null)}
        title={t("chats.message.attachedImage")}
      >
        {editingAttachment && (
          <div className="space-y-4 text-white">
            <div
              className={cn(
                radius.lg,
                "flex items-center justify-center overflow-hidden border border-white/10 bg-black/30",
              )}
            >
              {editingAttachment.data ? (
                <img
                  src={editingAttachment.data}
                  alt={editingAttachment.filename || t("chats.message.attachedImage")}
                  className="max-h-[50vh] w-auto max-w-full object-contain"
                />
              ) : (
                <div className="flex min-h-48 w-full items-center justify-center">
                  <div className="h-6 w-6 animate-spin rounded-full border-2 border-white/20 border-t-white/60" />
                </div>
              )}
            </div>
            {editingAttachment.filename && (
              <div className="space-y-1">
                <div className="text-[11px] font-semibold uppercase tracking-[0.22em] text-white/45">
                  {t("chats.actions.filename")}
                </div>
                <div className="line-clamp-3 break-words text-sm text-white/75">
                  {editingAttachment.filename}
                </div>
              </div>
            )}
            <div className="flex gap-3">
              <button
                type="button"
                onClick={() => setEditingAttachmentId(null)}
                className={cn(
                  "flex-1 px-4 py-3 text-sm font-medium text-white/70 transition",
                  "border border-white/10 bg-white/5",
                  "hover:bg-white/10 hover:text-white",
                  radius.lg,
                )}
              >
                {t("common.buttons.cancel")}
              </button>
              <button
                type="button"
                onClick={() => {
                  const attachmentId = editingAttachment.id;
                  setEditAttachments((prev) =>
                    prev.filter((candidate) => candidate.id !== attachmentId),
                  );
                  setEditingAttachmentId(null);
                }}
                className={cn(
                  "flex-1 px-4 py-3 text-sm font-semibold text-red-100 transition",
                  "border border-red-500/30 bg-red-500/20",
                  "hover:bg-red-500/30",
                  radius.lg,
                )}
              >
                {t("common.buttons.remove")}
              </button>
            </div>
          </div>
        )}
      </BottomMenu>
    </>
  );
}
