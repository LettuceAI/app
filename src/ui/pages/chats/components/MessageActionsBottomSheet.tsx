import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  Edit3,
  Copy,
  RotateCcw,
  Trash2,
  X,
  Bug,
  Pin,
  PinOff,
  Brain,
  BookOpen,
  GitBranch,
  Users,
  Paintbrush,
  Image,
  TriangleAlert,
  type LucideIcon,
} from "lucide-react";
import { BottomMenu } from "../../../components/BottomMenu";
import type {
  StoredMessage,
  Settings,
  Model,
  ImageAttachment,
} from "../../../../core/storage/schemas";
import { cn, radius } from "../../../design-tokens";
import { readSettings } from "../../../../core/storage/repo";
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
  hasSceneImage?: boolean;
  sceneGenerationEnabled?: boolean;
  characterMemoryType?: string | null;
  characterDefaultModelId?: string | null;
  characterId?: string;
  sessionId?: string | null;
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
  hasSceneImage = false,
  sceneGenerationEnabled = false,
  characterMemoryType,
  characterDefaultModelId,
  characterId,
  sessionId,
}: MessageActionsBottomSheetProps) {
  const navigate = useNavigate();
  const { t } = useI18n();
  const [settings, setSettings] = useState<Settings | null>(null);
  const [modelName, setModelName] = useState<string | null>(null);
  const [modelProviderId, setModelProviderId] = useState<string | null>(null);
  const [editAttachments, setEditAttachments] = useState<ImageAttachment[]>([]);
  const isSceneMessage = messageAction?.message.role === "scene";
  const isAssistantLikeMessage =
    messageAction?.message.role === "assistant" || messageAction?.message.role === "scene";

  const canEdit =
    isAssistantLikeMessage ||
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
    }
  }, [messageAction]);

  useEffect(() => {
    const messageModelId = messageAction?.message.modelId ?? null;
    const resolvedModelId =
      messageModelId ?? characterDefaultModelId ?? settings?.defaultModelId ?? null;

    if (resolvedModelId && settings) {
      const model = settings.models.find((m: Model) => m.id === resolvedModelId);
      setModelName(model ? model.displayName : resolvedModelId);
      setModelProviderId(model?.providerId ?? null);
    } else {
      setModelName(null);
      setModelProviderId(null);
    }
  }, [messageAction, settings, characterDefaultModelId]);

  const modelLabel =
    modelName ?? (settings ? t("chats.actions.unknownModel") : t("chats.actions.loadingModel"));
  const usedFallback = Boolean(messageAction?.message.fallbackFromModelId);
  const usedLorebookEntries = messageAction?.message.usedLorebookEntries ?? [];
  const isLlamaMessage = modelProviderId === "llamacpp";
  const firstTokenMs = messageAction?.message.usage?.firstTokenMs;
  const tokensPerSecond = messageAction?.message.usage?.tokensPerSecond;
  const loadedEditAttachments = useSessionAttachments(editAttachments);
  const canOpenDebug =
    isDevelopmentMode() &&
    Boolean(characterId && sessionId) &&
    messageAction?.message.role === "assistant";

  const handleCopy = async () => {
    if (!messageAction) return;
    try {
      await navigator.clipboard?.writeText(messageAction.message.content);
      setActionStatus("Copied!");
      setTimeout(() => setActionStatus(null), 1500);
    } catch (copyError) {
      setActionError(copyError instanceof Error ? copyError.message : String(copyError));
    }
  };

  return (
    <BottomMenu
      isOpen={Boolean(messageAction)}
      includeExitIcon={false}
      onClose={() => closeMessageActions(true)}
      title={
        isSceneMessage
          ? t("chats.message.sceneLabel")
          : isAssistantLikeMessage
            ? t("chats.actions.assistantMessage")
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
                    {usedFallback && (
                      <span
                        title={t("chats.actions.fallbackModelUsed")}
                        aria-label={t("chats.actions.fallbackModelUsed")}
                      >
                        <TriangleAlert size={12} className="text-amber-300" />
                      </span>
                    )}
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
              {isLlamaMessage &&
                (typeof firstTokenMs === "number" || typeof tokensPerSecond === "number") && (
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

              {!isSceneMessage && <div className="h-px bg-white/5 my-2" />}

              {/* Chat flow actions */}
              {(messageAction.message.role === "assistant" ||
                messageAction.message.role === "scene" ||
                messageAction.message.role === "user") && (
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

              {!isSceneMessage && characterId && (
                <ActionRow
                  icon={Paintbrush}
                  label={t("chats.actions.chatAppearance")}
                  iconBg="bg-purple-500/20"
                  onClick={() => {
                    closeMessageActions(true);
                    navigate(`/settings/accessibility/chat?characterId=${characterId}`);
                  }}
                />
              )}

              {!isSceneMessage && (
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
                  <div className="text-xs font-medium uppercase tracking-wide text-white/50">
                    Attachments
                  </div>
                  <div className="flex flex-wrap gap-2">
                    {loadedEditAttachments.map((attachment) => (
                      <div
                        key={attachment.id}
                        className={cn(
                          radius.md,
                          "relative overflow-hidden border border-white/15 bg-black/20",
                        )}
                      >
                        <button
                          type="button"
                          onClick={() =>
                            setEditAttachments((prev) =>
                              prev.filter((candidate) => candidate.id !== attachment.id),
                            )
                          }
                          className="absolute right-2 top-2 z-10 flex h-7 w-7 items-center justify-center rounded-full border border-white/15 bg-black/75 text-white/80 transition hover:bg-black hover:text-white"
                          aria-label={`Remove ${attachment.filename || "attachment"}`}
                        >
                          <X size={14} />
                        </button>
                        {attachment.data ? (
                          <img
                            src={attachment.data}
                            alt={attachment.filename || t("chats.message.attachedImage")}
                            className="max-h-36 max-w-full object-contain"
                            style={{
                              maxWidth:
                                attachment.width && attachment.width > 220
                                  ? 220
                                  : attachment.width || 220,
                            }}
                          />
                        ) : (
                          <div
                            className="flex items-center justify-center bg-white/5"
                            style={{
                              width: Math.min(attachment.width || 150, 220),
                              height: Math.min(attachment.height || 100, 144),
                            }}
                          >
                            <div className="h-5 w-5 animate-spin rounded-full border-2 border-white/20 border-t-white/60" />
                          </div>
                        )}
                      </div>
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
  );
}
