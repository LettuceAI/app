import { useState, useEffect } from "react";
import { Copy, Trash2, RotateCcw, Edit3, Users } from "lucide-react";

import type { Character, Settings, Model } from "../../../../core/storage/schemas";
import { useAvatar } from "../../../hooks/useAvatar";
import { AvatarImage } from "../../../components/AvatarImage";
import { BottomMenu } from "../../../components/BottomMenu";
import { MarkdownRenderer } from "../../chats/components/MarkdownRenderer";
import { radius, cn, interactive } from "../../../design-tokens";
import type { MessageActionState } from "../reducers/groupChatReducer";
import { readSettings } from "../../../../core/storage/repo";

function ActionRow({
  icon: Icon,
  label,
  onClick,
  disabled = false,
  variant = "default",
  iconBg,
}: {
  icon: typeof Copy;
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
        "hover:bg-fg/5 active:bg-fg/10",
        "disabled:opacity-40 disabled:pointer-events-none",
        variant === "danger" && "hover:bg-danger/10",
      )}
    >
      <div
        className={cn(
          "flex items-center justify-center w-8 h-8 rounded-lg",
          iconBg || "bg-fg/10",
        )}
      >
        <Icon size={16} className={cn(variant === "danger" ? "text-danger" : "text-fg")} />
      </div>
      <span
        className={cn(
          "text-[15px] text-left",
          variant === "danger" ? "text-danger" : "text-fg/90",
        )}
      >
        {label}
      </span>
    </button>
  );
}

export function GroupChatMessageActionsBottomSheet({
  messageAction,
  actionError,
  actionStatus,
  actionBusy,
  editDraft,
  setEditDraft,
  closeMessageActions,
  setActionError,
  setActionStatus,
  handleSaveEdit,
  handleDeleteMessage,
  handleRewindToMessage,
  handleCopyMessage,
  setMessageAction,
  onRegenerate,
  characters,
}: {
  messageAction: MessageActionState | null;
  actionError: string | null;
  actionStatus: string | null;
  actionBusy: boolean;
  editDraft: string;
  setEditDraft: (value: string) => void;
  closeMessageActions: () => void;
  setActionError: (value: string | null) => void;
  setActionStatus: (value: string | null) => void;
  handleSaveEdit: () => Promise<void>;
  handleDeleteMessage: () => Promise<void>;
  handleRewindToMessage: () => Promise<void>;
  handleCopyMessage: () => Promise<void>;
  setMessageAction: (value: MessageActionState | null) => void;
  onRegenerate: (characterId?: string) => void;
  characters: Character[];
}) {
  const [showCharacterPicker, setShowCharacterPicker] = useState(false);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [modelName, setModelName] = useState<string | null>(null);

  const isAssistant = messageAction?.message.role === "assistant";

  useEffect(() => {
    readSettings().then(setSettings).catch(console.error);
  }, []);

  useEffect(() => {
    console.log("ModelId debug:", {
      messageAction: messageAction?.message,
      modelId: messageAction?.message.modelId,
      settings: settings?.models.length,
      isAssistant,
    });

    if (messageAction?.message.modelId && settings) {
      const model = settings.models.find((m: Model) => m.id === messageAction.message.modelId);
      console.log("Found model:", model);
      setModelName(model ? model.displayName : null);
    } else {
      setModelName(null);
    }
  }, [messageAction?.message.modelId, settings, isAssistant]);

  return (
    <>
      <BottomMenu
        isOpen={Boolean(messageAction) && !showCharacterPicker}
        includeExitIcon={false}
        onClose={closeMessageActions}
        title={isAssistant ? "Character Message" : "Your Message"}
      >
        {messageAction && (
          <div className="text-fg">
            {isAssistant && messageAction.message.selectionReasoning && (
              <div className="mb-4 px-3 py-2 rounded-lg border border-fg/10 bg-fg/5">
                <p className="text-[10px] text-fg/40 uppercase tracking-wide mb-1">
                  Why this character responded
                </p>
                <div className="text-xs text-fg/70 italic">
                  <MarkdownRenderer
                    content={messageAction.message.selectionReasoning}
                    className="text-xs text-fg/70 italic"
                  />
                </div>
              </div>
            )}

            {messageAction.message.usage && (
              <div className="flex items-center gap-x-3 text-xs text-fg/40 mb-4">
                <div className="flex items-center gap-2 border-r border-fg/10 pr-3">
                  <span title="Prompt Tokens">
                    ↓{messageAction.message.usage.promptTokens ?? 0}
                  </span>
                  <span title="Completion Tokens">
                    ↑{messageAction.message.usage.completionTokens ?? 0}
                  </span>
                </div>
                <div className="flex-1">
                  <span className="text-fg/60">
                    {modelName || messageAction.message.modelId}
                  </span>
                </div>
                <div className="tabular-nums">
                  {(messageAction.message.usage.totalTokens ?? 0).toLocaleString()}{" "}
                  <span className="text-[12px] uppercase opacity-50">total</span>
                </div>
              </div>
            )}

            {actionStatus && (
              <div className="mb-3 px-3 py-2 rounded-lg border border-accent/20 bg-accent/10">
                <p className="text-sm text-accent/80">{actionStatus}</p>
              </div>
            )}
            {actionError && (
              <div className="mb-3 px-3 py-2 rounded-lg border border-danger/20 bg-danger/10">
                <p className="text-sm text-danger">{actionError}</p>
              </div>
            )}

            {messageAction.mode === "view" ? (
              <div className="space-y-1">
                <ActionRow
                  icon={Edit3}
                  label="Edit"
                  iconBg="bg-info/20"
                  onClick={() => {
                    setActionError(null);
                    setActionStatus(null);
                    setMessageAction({ message: messageAction.message, mode: "edit" });
                    setEditDraft(messageAction.message.content);
                  }}
                />

                <ActionRow
                  icon={Copy}
                  label="Copy"
                  iconBg="bg-secondary/20"
                  onClick={() => void handleCopyMessage()}
                />

                {isAssistant && (
                  <ActionRow
                    icon={Users}
                    label="Regenerate with different character"
                    iconBg="bg-accent/20"
                    onClick={() => setShowCharacterPicker(true)}
                  />
                )}

                <div className="h-px bg-fg/5 my-2" />

                <ActionRow
                  icon={RotateCcw}
                  label="Rewind to here"
                  iconBg="bg-info/20"
                  onClick={() => void handleRewindToMessage()}
                  disabled={actionBusy}
                />

                <div className="h-px bg-fg/5 my-2" />

                <ActionRow
                  icon={Trash2}
                  label={messageAction.message.isPinned ? "Unpin to delete" : "Delete"}
                  onClick={() => void handleDeleteMessage()}
                  disabled={actionBusy || messageAction.message.isPinned}
                  variant="danger"
                />
              </div>
            ) : (
              <div className="space-y-4">
                <textarea
                  value={editDraft}
                  onChange={(event) => setEditDraft(event.target.value)}
                  rows={5}
                  className={cn(
                    "w-full p-3 text-sm text-fg placeholder-fg/40",
                    "border border-fg/10 bg-surface-el/30",
                    "focus:border-fg/20 focus:outline-none resize-none",
                    radius.lg,
                  )}
                  placeholder="Edit your message..."
                  disabled={actionBusy}
                  autoFocus
                />
                <div className="flex gap-3">
                  <button
                    onClick={() => {
                      setActionError(null);
                      setActionStatus(null);
                      setMessageAction({ message: messageAction.message, mode: "view" });
                      setEditDraft(messageAction.message.content);
                    }}
                    className={cn(
                      "flex-1 px-4 py-3 text-sm font-medium text-fg/70 transition",
                      "border border-fg/10 bg-fg/5",
                      "hover:bg-fg/10 hover:text-fg",
                      "active:scale-[0.98]",
                      radius.lg,
                    )}
                  >
                    Cancel
                  </button>
                  <button
                    onClick={() => void handleSaveEdit()}
                    disabled={actionBusy}
                    className={cn(
                      "flex-1 px-4 py-3 text-sm font-semibold text-fg transition",
                      "bg-accent",
                      "hover:bg-accent",
                      "active:scale-[0.98]",
                      "disabled:cursor-not-allowed disabled:opacity-50",
                      radius.lg,
                    )}
                  >
                    {actionBusy ? "Saving..." : "Save"}
                  </button>
                </div>
              </div>
            )}
          </div>
        )}
      </BottomMenu>

      <BottomMenu
        isOpen={showCharacterPicker}
        includeExitIcon={false}
        onClose={() => setShowCharacterPicker(false)}
        title="Choose Character"
      >
        <div className="space-y-2">
          <p className="text-sm text-fg/50 mb-3">
            Select which character should respond instead:
          </p>
          {characters.map((char) => (
            <CharacterPickerItem
              key={char.id}
              character={char}
              onClick={() => {
                setShowCharacterPicker(false);
                onRegenerate(char.id);
              }}
            />
          ))}
        </div>
      </BottomMenu>
    </>
  );
}

function CharacterPickerItem({
  character,
  onClick,
}: {
  character: Character;
  onClick: () => void;
}) {
  const avatarUrl = useAvatar("character", character.id, character.avatarPath, "round");
  const description = character.description || character.definition;

  return (
    <button
      onClick={onClick}
      className={cn(
        "flex w-full items-center gap-3 p-3 text-left",
        radius.lg,
        "border border-fg/10 bg-fg/5",
        "hover:border-fg/20 hover:bg-fg/10",
        interactive.transition.fast,
      )}
    >
      <div
        className={cn(
          "h-10 w-10 rounded-full overflow-hidden",
          "bg-linear-to-br from-white/10 to-white/5",
          "ring-1 ring-white/10",
        )}
      >
        {avatarUrl ? (
          <AvatarImage src={avatarUrl} alt={character.name} crop={character.avatarCrop} applyCrop />
        ) : (
          <div className="flex h-full w-full items-center justify-center text-sm font-bold text-fg/60">
            {character.name.slice(0, 1).toUpperCase()}
          </div>
        )}
      </div>
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium text-fg truncate">{character.name}</p>
        {description && <p className="text-xs text-fg/50 truncate">{description}</p>}
      </div>
    </button>
  );
}
