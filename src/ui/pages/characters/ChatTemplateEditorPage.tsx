import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { RefObject } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { Reorder, useDragControls, motion } from "framer-motion";
import {
  Plus,
  GripVertical,
  Trash2,
  Edit2,
  User,
  Bot,
  Check,
  X,
  Loader2,
  MessageSquare,
  ChevronDown,
  SlidersHorizontal,
} from "lucide-react";
import { listCharacters, saveCharacter } from "../../../core/storage/repo";
import { listPromptTemplates } from "../../../core/prompts/service";
import { useI18n } from "../../../core/i18n/context";
import {
  APP_DYNAMIC_SUMMARY_TEMPLATE_ID,
  APP_DYNAMIC_MEMORY_TEMPLATE_ID,
  APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_ID,
  APP_HELP_ME_REPLY_TEMPLATE_ID,
} from "../../../core/prompts/constants";
import type {
  Character,
  ChatTemplate,
  ChatTemplateMessage,
  SystemPromptTemplate,
  StoredMessage,
} from "../../../core/storage/schemas";
import { ChatMessage } from "../chats/components/ChatMessage";
import { getDefaultThemeSync } from "../../../core/utils/imageAnalysis";
import { BottomMenu, MenuButton, MenuButtonGroup } from "../../components/BottomMenu";

const defaultTheme = getDefaultThemeSync();
const noop = () => {};
const noopAsync = async () => {};
const noVariants = () => ({ total: 0, selectedIndex: -1 });

function formatSceneLabel(
  scene: { direction?: string; content?: string } | null,
  noSceneLabel: string,
  untitledLabel: string,
): string {
  if (!scene) return noSceneLabel;
  const raw = (scene.content || scene.direction || "").trim();
  if (!raw) return untitledLabel;
  return raw.replace(/\s+/g, " ").replace(/^[*\-+#>\s]+/, "");
}

/* ─── Convert template message to StoredMessage for ChatMessage ───── */

function toStoredMessage(msg: ChatTemplateMessage): StoredMessage {
  return {
    id: msg.id,
    role: msg.role,
    content: msg.content,
    createdAt: Date.now(),
  } as StoredMessage;
}

/* ─── Reorderable wrapper with drag handle + actions overlay ──────── */

function DraggableMessage({
  msg,
  onEdit,
  onDelete,
  character,
}: {
  msg: ChatTemplateMessage;
  onEdit: () => void;
  onDelete: () => void;
  character: Character;
}) {
  const { t } = useI18n();
  const controls = useDragControls();

  return (
    <Reorder.Item
      value={msg}
      layout
      dragListener={false}
      dragControls={controls}
      whileDrag={{
        zIndex: 50,
        scale: 1.02,
        boxShadow: "0 16px 32px rgba(0,0,0,0.3), 0 4px 12px rgba(0,0,0,0.2)",
      }}
      transition={{ layout: { duration: 0.2, ease: "easeOut" } }}
      style={{ position: "relative", zIndex: 0 }}
      className="group"
    >
      <div>
        <ChatMessage
          message={toStoredMessage(msg)}
          index={0}
          messagesLength={0}
          heldMessageId={null}
          regeneratingMessageId={null}
          sending={false}
          eventHandlers={{}}
          getVariantState={noVariants}
          handleVariantDrag={noop}
          handleRegenerate={noopAsync}
          isStartingSceneMessage={false}
          theme={defaultTheme}
          character={character}
          persona={null}
        />
        {/* Message controls */}
        <div
          className={`mt-1 flex items-center gap-1 px-2 opacity-100 transition sm:opacity-0 sm:group-hover:opacity-100 sm:group-focus-within:opacity-100 ${
            msg.role === "user" ? "justify-end" : "justify-start"
          }`}
        >
          <button
            onPointerDown={(e) => controls.start(e)}
            className="flex h-6 w-6 cursor-grab items-center justify-center rounded bg-surface/80 text-fg/40 backdrop-blur-sm transition active:cursor-grabbing active:text-fg/70"
            style={{ touchAction: "none" }}
            aria-label={t("characters.templateEditor.dragMessage")}
          >
            <GripVertical className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            onClick={onEdit}
            className="flex h-6 w-6 items-center justify-center rounded bg-surface/80 text-fg/40 backdrop-blur-sm transition hover:text-fg/70"
            aria-label={t("characters.templateEditor.editMessage")}
          >
            <Edit2 className="h-3 w-3" />
          </button>
          <button
            type="button"
            onClick={onDelete}
            className="flex h-6 w-6 items-center justify-center rounded bg-surface/80 text-fg/40 backdrop-blur-sm transition hover:text-error"
            aria-label={t("characters.templateEditor.deleteMessage")}
          >
            <Trash2 className="h-3 w-3" />
          </button>
        </div>
      </div>
    </Reorder.Item>
  );
}

/* ─── Inline editing form ─────────────────────────────────────────── */

function EditingForm({
  msg,
  editContent,
  setEditContent,
  editTextareaRef,
  onToggleRole,
  onSave,
  onCancel,
  characterName,
}: {
  msg: ChatTemplateMessage;
  editContent: string;
  setEditContent: (v: string) => void;
  editTextareaRef: RefObject<HTMLTextAreaElement | null>;
  onToggleRole: () => void;
  onSave: () => void;
  onCancel: () => void;
  characterName: string;
}) {
  const { t } = useI18n();

  return (
    <motion.div layout className="space-y-2 px-2">
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={onToggleRole}
          className="flex items-center gap-1.5 rounded-lg border border-fg/10 bg-fg/5 px-2.5 py-1 text-xs font-medium text-fg/60 transition active:bg-fg/10"
        >
          {msg.role === "user" ? <User className="h-3 w-3" /> : <Bot className="h-3 w-3" />}
          {msg.role === "user" ? "You" : characterName}
        </button>
      </div>
      <textarea
        ref={editTextareaRef}
        value={editContent}
        onChange={(e) => setEditContent(e.target.value)}
        rows={4}
        placeholder={t("characters.templateEditor.writeMessagePlaceholder")}
        className="w-full resize-none rounded-xl border border-accent/30 bg-accent/5 px-3.5 py-2.5 text-sm text-fg outline-none placeholder:text-fg/30 focus:border-accent/50"
      />
      <div className="flex gap-2">
        <button
          type="button"
          onClick={onSave}
          className="flex items-center gap-1 rounded-lg bg-accent/20 px-3 py-1.5 text-xs font-medium text-accent transition active:bg-accent/30"
        >
          <Check className="h-3 w-3" />
          Done
        </button>
        <button
          type="button"
          onClick={onCancel}
          className="flex items-center gap-1 rounded-lg bg-fg/5 px-3 py-1.5 text-xs font-medium text-fg/50 transition active:bg-fg/10"
        >
          <X className="h-3 w-3" />
          Cancel
        </button>
      </div>
    </motion.div>
  );
}

/* ─── Main page ───────────────────────────────────────────────────── */

export default function ChatTemplateEditorPage() {
  const { characterId, templateId } = useParams<{
    characterId: string;
    templateId: string;
  }>();
  const navigate = useNavigate();
  const { t } = useI18n();
  const isNew = templateId === "new";

  const [character, setCharacter] = useState<Character | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  const [name, setName] = useState("");
  const [messages, setMessages] = useState<ChatTemplateMessage[]>([]);
  const [nextRole, setNextRole] = useState<"user" | "assistant">("assistant");

  // Scene selector
  const [selectedSceneId, setSelectedSceneId] = useState<string | null>(null);
  const [showSceneMenu, setShowSceneMenu] = useState(false);
  const [_showMobileOptionsMenu, setShowMobileOptionsMenu] = useState(false);
  const [showPromptTemplateMenu, setShowPromptTemplateMenu] = useState(false);
  const [promptTemplates, setPromptTemplates] = useState<SystemPromptTemplate[]>([]);
  const [selectedPromptTemplateId, setSelectedPromptTemplateId] = useState<string | null>(null);

  // Inline editing
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editContent, setEditContent] = useState("");
  const editTextareaRef = useRef<HTMLTextAreaElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Track initial state for change detection
  const initialStateRef = useRef<{
    name: string;
    messages: string;
    selectedSceneId: string | null;
    selectedPromptTemplateId: string | null;
  } | null>(null);

  const loadCharacter = useCallback(async () => {
    if (!characterId) return;
    try {
      const chars = await listCharacters();
      const templates = await listPromptTemplates();
      setPromptTemplates(
        templates.filter(
          (template) =>
            template.id !== APP_DYNAMIC_SUMMARY_TEMPLATE_ID &&
            template.id !== APP_DYNAMIC_MEMORY_TEMPLATE_ID &&
            template.id !== APP_HELP_ME_REPLY_TEMPLATE_ID &&
            template.id !== APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_ID,
        ),
      );
      const c = chars.find((ch) => ch.id === characterId);
      if (c) {
        setCharacter(c);
        if (!isNew && templateId) {
          const existing = c.chatTemplates?.find((t) => t.id === templateId);
          if (existing) {
            setName(existing.name);
            setMessages([...existing.messages]);
            // Existing templates should reflect their own saved scene state.
            // If sceneId is missing/null, treat it as "No scene" instead of
            // falling back to the character's default scene.
            const sceneId = existing.sceneId ?? null;
            setSelectedSceneId(sceneId);
            const promptTemplateId = existing.promptTemplateId ?? null;
            setSelectedPromptTemplateId(promptTemplateId);
            initialStateRef.current = {
              name: existing.name,
              messages: JSON.stringify(existing.messages),
              selectedSceneId: sceneId,
              selectedPromptTemplateId: promptTemplateId,
            };
          }
        } else {
          // New template: default to character's default scene
          const sceneId = c.defaultSceneId ?? (c.scenes?.length === 1 ? c.scenes[0].id : null);
          setSelectedSceneId(sceneId);
          const promptTemplateId = c.promptTemplateId ?? null;
          setSelectedPromptTemplateId(promptTemplateId);
          initialStateRef.current = {
            name: "",
            messages: "[]",
            selectedSceneId: sceneId,
            selectedPromptTemplateId: promptTemplateId,
          };
        }
      }
    } finally {
      setLoading(false);
    }
  }, [characterId, templateId, isNew]);

  useEffect(() => {
    loadCharacter();
  }, [loadCharacter]);

  // Auto-set next role based on last message
  useEffect(() => {
    if (messages.length === 0) {
      setNextRole("assistant");
    } else {
      const lastRole = messages[messages.length - 1].role;
      setNextRole(lastRole === "assistant" ? "user" : "assistant");
    }
  }, [messages]);

  const addMessage = useCallback(() => {
    const newMsg: ChatTemplateMessage = {
      id: globalThis.crypto?.randomUUID?.() ?? crypto.randomUUID(),
      role: nextRole,
      content: "",
    };
    setMessages((prev) => [...prev, newMsg]);
    setEditingId(newMsg.id);
    setEditContent("");
    requestAnimationFrame(() => {
      scrollRef.current?.scrollTo({
        top: scrollRef.current.scrollHeight,
        behavior: "smooth",
      });
    });
  }, [nextRole]);

  const deleteMessage = useCallback((id: string) => {
    setMessages((prev) => prev.filter((m) => m.id !== id));
    setEditingId((curr) => (curr === id ? null : curr));
  }, []);

  const startEditing = useCallback((msg: ChatTemplateMessage) => {
    setEditingId(msg.id);
    setEditContent(msg.content);
  }, []);

  const saveEditing = useCallback(() => {
    if (!editingId) return;
    setMessages((prev) =>
      prev.map((m) => (m.id === editingId ? { ...m, content: editContent } : m)),
    );
    setEditingId(null);
    setEditContent("");
  }, [editingId, editContent]);

  const cancelEditing = useCallback(() => {
    if (editingId) {
      setMessages((prev) => {
        const msg = prev.find((m) => m.id === editingId);
        if (msg && !msg.content && !editContent) {
          return prev.filter((m) => m.id !== editingId);
        }
        return prev;
      });
    }
    setEditingId(null);
    setEditContent("");
  }, [editingId, editContent]);

  const toggleEditingRole = useCallback((msgId: string) => {
    setMessages((prev) =>
      prev.map((m) =>
        m.id === msgId ? { ...m, role: m.role === "user" ? "assistant" : "user" } : m,
      ),
    );
  }, []);

  // Focus textarea when editing starts
  useEffect(() => {
    if (editingId && editTextareaRef.current) {
      editTextareaRef.current.focus();
    }
  }, [editingId]);

  const handleSave = useCallback(async () => {
    if (!character || !name.trim()) return;
    setSaving(true);
    try {
      const now = Date.now();
      const template: ChatTemplate = {
        id: isNew ? (globalThis.crypto?.randomUUID?.() ?? crypto.randomUUID()) : templateId!,
        name: name.trim(),
        messages: messages.filter((m) => m.content.trim()),
        createdAt: isNew
          ? now
          : (character.chatTemplates?.find((t) => t.id === templateId)?.createdAt ?? now),
        sceneId: selectedSceneId,
        promptTemplateId: selectedPromptTemplateId,
      };

      const existingTemplates = character.chatTemplates ?? [];
      const updatedTemplates = isNew
        ? [...existingTemplates, template]
        : existingTemplates.map((t) => (t.id === templateId ? template : t));

      const updated = { ...character, chatTemplates: updatedTemplates };
      await saveCharacter(updated);
      navigate(-1);
    } finally {
      setSaving(false);
    }
  }, [
    character,
    name,
    messages,
    isNew,
    templateId,
    navigate,
    selectedSceneId,
    selectedPromptTemplateId,
  ]);

  // Change detection: only enable save when something actually changed
  const canSave = useMemo(() => {
    if (!name.trim() || saving) return false;
    const initial = initialStateRef.current;
    if (!initial) return false;
    const hasContent = messages.some((m) => m.content.trim());
    // For new templates, require at least a name
    if (isNew && !hasContent && messages.length === 0) {
      return name.trim().length > 0 && name !== initial.name;
    }
    return (
      name !== initial.name ||
      JSON.stringify(messages) !== initial.messages ||
      selectedSceneId !== initial.selectedSceneId ||
      selectedPromptTemplateId !== initial.selectedPromptTemplateId
    );
  }, [name, messages, saving, isNew, selectedSceneId, selectedPromptTemplateId]);

  // Wire up save button to global TopNav
  useEffect(() => {
    const g = window as any;
    g.__saveCharacter = handleSave;
    g.__saveCharacterCanSave = canSave;
    g.__saveCharacterSaving = saving;
    return () => {
      delete g.__saveCharacter;
      delete g.__saveCharacterCanSave;
      delete g.__saveCharacterSaving;
    };
  }, [handleSave, canSave, saving]);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-fg/40" />
      </div>
    );
  }

  if (!character) {
    return (
      <div className="flex h-full items-center justify-center text-fg/50">
        {t("characters.templateEditor.characterNotFound")}
      </div>
    );
  }

  const scenes = character.scenes ?? [];
  const selectedScene = scenes.find((s) => s.id === selectedSceneId) ?? null;
  const selectedSceneLabel = formatSceneLabel(
    selectedScene,
    t("characters.templateEditor.noScene"),
    t("characters.templateEditor.untitled"),
  );
  const selectedPromptTemplate = promptTemplates.find(
    (template) => template.id === selectedPromptTemplateId,
  );

  // Build scene message for display
  const sceneStoredMessage: StoredMessage | null = selectedScene
    ? ({
        id: "scene-preview",
        role: "assistant" as const,
        content: selectedScene.content || selectedScene.direction || "",
        createdAt: Date.now(),
      } as StoredMessage)
    : null;

  /* ── Chat preview panel ────────────────────────────────────────────── */

  const chatPreview = (
    <div className="flex min-h-0 flex-1 flex-col">
      <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto px-3 py-4 lg:px-6">
        {/* Scene message at top */}
        {sceneStoredMessage && sceneStoredMessage.content && (
          <div className="mb-4">
            <div className="mb-1 flex items-center gap-1.5 px-2">
              <span className="text-[10px] font-semibold uppercase tracking-wider text-fg/30">
                {t("characters.templateEditor.scene")}
              </span>
            </div>
            <div className="relative">
              <ChatMessage
                message={sceneStoredMessage}
                index={0}
                messagesLength={0}
                heldMessageId={null}
                regeneratingMessageId={null}
                sending={false}
                eventHandlers={{}}
                getVariantState={noVariants}
                handleVariantDrag={noop}
                handleRegenerate={noopAsync}
                isStartingSceneMessage={true}
                theme={defaultTheme}
                character={character}
                persona={null}
              />
            </div>
          </div>
        )}

        {/* Template messages */}
        {messages.length === 0 && !sceneStoredMessage?.content ? (
          <div className="flex h-full flex-col items-center justify-center text-center">
            <div className="mb-3 rounded-2xl border border-fg/10 bg-fg/5 p-4">
              <MessageSquare className="h-8 w-8 text-fg/20" />
            </div>
            <p className="text-sm font-medium text-fg/50">
              {t("characters.templateEditor.noMessagesYet")}
            </p>
            <p className="mt-1 max-w-60 text-xs text-fg/30">
              {t("characters.templateEditor.addMessagesDesc", { name: character.name })}
            </p>
          </div>
        ) : (
          <Reorder.Group
            axis="y"
            values={messages}
            onReorder={setMessages}
            className="flex flex-col gap-1"
          >
            {messages.map((msg) =>
              editingId === msg.id ? (
                <EditingForm
                  key={msg.id}
                  msg={msg}
                  editContent={editContent}
                  setEditContent={setEditContent}
                  editTextareaRef={editTextareaRef}
                  onToggleRole={() => toggleEditingRole(msg.id)}
                  onSave={saveEditing}
                  onCancel={cancelEditing}
                  characterName={character.name}
                />
              ) : (
                <DraggableMessage
                  key={msg.id}
                  msg={msg}
                  onEdit={() => startEditing(msg)}
                  onDelete={() => deleteMessage(msg.id)}
                  character={character}
                />
              ),
            )}
          </Reorder.Group>
        )}
      </div>

      {/* Add message bar */}
      <div className="border-t border-fg/10 px-4 py-3">
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setNextRole((r) => (r === "user" ? "assistant" : "user"))}
            className="flex items-center gap-1.5 rounded-lg border border-fg/10 bg-fg/5 px-2.5 py-2 text-xs font-medium text-fg/60 transition active:bg-fg/10"
          >
            {nextRole === "user" ? (
              <User className="h-3.5 w-3.5" />
            ) : (
              <Bot className="h-3.5 w-3.5" />
            )}
            {nextRole === "user" ? "You" : character.name}
          </button>
          <motion.button
            whileTap={{ scale: 0.97 }}
            onClick={addMessage}
            className="flex flex-1 items-center justify-center gap-1.5 rounded-lg border border-accent/50 bg-accent/20 py-2 text-sm font-medium text-accent transition active:bg-accent/30"
          >
            <Plus className="h-4 w-4" />
            {t("characters.templateEditor.addMessage")}
          </motion.button>
        </div>
      </div>
    </div>
  );

  /* ── Settings panel (desktop left side) ────────────────────────────── */

  const settingsPanel = (
    <div className="flex flex-col gap-5 p-4">
      {/* Name */}
      <div>
        <label className="mb-1.5 block text-[11px] font-semibold uppercase tracking-wider text-fg/40">
          {t("characters.templateEditor.name")}
        </label>
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder={t("characters.templateEditor.nameExample")}
          className="w-full rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-sm text-fg outline-none placeholder:text-fg/30 focus:border-accent/40"
        />
      </div>

      {/* Scene selector */}
      {scenes.length > 0 && (
        <div>
          <label className="mb-1.5 block text-[11px] font-semibold uppercase tracking-wider text-fg/40">
            {t("characters.templateEditor.startingScene")}
          </label>
          <button
            type="button"
            onClick={() => setShowSceneMenu(true)}
            className="flex w-full items-center justify-between rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-left text-sm text-fg transition hover:bg-fg/10"
          >
            <span className={selectedScene ? "text-fg" : "text-fg/40"}>
              {selectedScene
                ? selectedSceneLabel.slice(0, 40) + (selectedSceneLabel.length > 40 ? "..." : "")
                : t("characters.templateEditor.noScene")}
            </span>
            <ChevronDown className="h-3.5 w-3.5 shrink-0 text-fg/40" />
          </button>
        </div>
      )}

      {/* Prompt template selector */}
      <div>
        <label className="mb-1.5 block text-[11px] font-semibold uppercase tracking-wider text-fg/40">
          {t("characters.templateEditor.systemPrompt")}
        </label>
        <button
          type="button"
          onClick={() => setShowPromptTemplateMenu(true)}
          className="flex w-full items-center justify-between rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-left text-sm text-fg transition hover:bg-fg/10"
        >
          <span className={selectedPromptTemplate ? "text-fg" : "text-fg/40"}>
            {selectedPromptTemplate?.name ?? t("characters.templateEditor.characterDefault")}
          </span>
          <ChevronDown className="h-3.5 w-3.5 shrink-0 text-fg/40" />
        </button>
      </div>

      {/* Next role selector */}
      <div>
        <label className="mb-1.5 block text-[11px] font-semibold uppercase tracking-wider text-fg/40">
          {t("characters.templateEditor.nextMessageAs")}
        </label>
        <div className="flex flex-col gap-1.5">
          <button
            type="button"
            onClick={() => setNextRole("assistant")}
            className={`flex items-center gap-2 rounded-lg border px-3 py-2 text-xs font-medium transition ${
              nextRole === "assistant"
                ? "border-accent/40 bg-accent/10 text-accent"
                : "border-fg/10 bg-fg/5 text-fg/50 hover:bg-fg/10"
            }`}
          >
            <Bot className="h-3.5 w-3.5" />
            {character.name}
          </button>
          <button
            type="button"
            onClick={() => setNextRole("user")}
            className={`flex items-center gap-2 rounded-lg border px-3 py-2 text-xs font-medium transition ${
              nextRole === "user"
                ? "border-accent/40 bg-accent/10 text-accent"
                : "border-fg/10 bg-fg/5 text-fg/50 hover:bg-fg/10"
            }`}
          >
            <User className="h-3.5 w-3.5" />
            You
          </button>
        </div>
      </div>

      {/* Stats */}
      {messages.length > 0 && (
        <div className="rounded-lg border border-fg/10 bg-fg/5 px-3 py-2.5">
          <div className="flex items-center justify-between text-xs text-fg/50">
            <span>{t("characters.templateEditor.messages")}</span>
            <span className="font-medium text-fg/70">{messages.length}</span>
          </div>
          {messages.length > 0 && (
            <div className="mt-1.5 flex items-center justify-between text-xs text-fg/50">
              <span>{t("characters.templateEditor.roles")}</span>
              <span className="font-medium text-fg/70">
                {messages.filter((m) => m.role === "assistant").length} {character.name},{" "}
                {messages.filter((m) => m.role === "user").length} You
              </span>
            </div>
          )}
        </div>
      )}

      {/* Tips */}
      <div className="space-y-2 text-[11px] leading-relaxed text-fg/30">
        <p>{t("characters.templateEditor.hoverTip")}</p>
        <p>{t("characters.templateEditor.footerTip")}</p>
      </div>
    </div>
  );

  /* ── Layout ────────────────────────────────────────────────────────── */

  return (
    <>
      <div
        className="flex overflow-hidden"
        style={{ height: "calc(100dvh - 72px - env(safe-area-inset-top))" }}
      >
        {/* Left panel (settings) — desktop only */}
        <div className="hidden lg:flex lg:w-70 lg:shrink-0 lg:flex-col lg:overflow-y-auto lg:border-r lg:border-fg/10">
          {settingsPanel}
        </div>

        {/* Right panel / full page */}
        <div className="flex h-full min-h-0 min-w-0 flex-1 flex-col">
          {/* Mobile-only controls */}
          <div className="border-b border-fg/10 px-4 py-3 lg:hidden">
            <div className="flex items-center gap-2">
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={t("characters.templateEditor.templateNamePlaceholder")}
                className="min-w-0 flex-1 rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-sm text-fg outline-none placeholder:text-fg/30 focus:border-accent/40"
              />
              <button
                type="button"
                onClick={() => setShowMobileOptionsMenu(true)}
                className="flex h-9 shrink-0 items-center gap-1.5 rounded-lg border border-fg/10 bg-fg/5 px-3 text-xs font-medium text-fg/70 transition active:bg-fg/10"
              >
                <SlidersHorizontal className="h-3.5 w-3.5" />
                Options
              </button>
            </div>
          </div>

          {chatPreview}
        </div>
      </div>

      {/* Scene selector bottom menu */}
      <BottomMenu
        isOpen={showSceneMenu}
        onClose={() => setShowSceneMenu(false)}
        title={t("characters.templateEditor.selectScene")}
      >
        <MenuButtonGroup>
          <MenuButton
            icon={<X className="h-4 w-4" />}
            title={t("characters.templateEditor.noScene")}
            description={t("characters.templateEditor.startWithoutScene")}
            color="from-blue-500 to-cyan-600"
            rightElement={
              selectedSceneId === null ? <Check className="h-4 w-4 text-emerald-300" /> : undefined
            }
            onClick={() => {
              setSelectedSceneId(null);
              setShowSceneMenu(false);
            }}
          />
          {scenes.map((scene) => {
            const sceneLabel = formatSceneLabel(
              scene,
              t("characters.templateEditor.noScene"),
              t("characters.templateEditor.untitled"),
            );
            return (
              <MenuButton
                key={scene.id}
                icon={<MessageSquare className="h-4 w-4" />}
                title={sceneLabel.slice(0, 60)}
                description={sceneLabel.length > 60 ? sceneLabel.slice(60, 120) : undefined}
                color="from-blue-500 to-cyan-600"
                rightElement={
                  selectedSceneId === scene.id ? (
                    <Check className="h-4 w-4 text-emerald-300" />
                  ) : undefined
                }
                onClick={() => {
                  setSelectedSceneId(scene.id);
                  setShowSceneMenu(false);
                }}
              />
            );
          })}
        </MenuButtonGroup>
      </BottomMenu>

      <BottomMenu
        isOpen={showPromptTemplateMenu}
        onClose={() => setShowPromptTemplateMenu(false)}
        title={t("characters.templateEditor.selectSystemPrompt")}
      >
        <MenuButtonGroup>
          <MenuButton
            icon={<X className="h-4 w-4" />}
            title={t("characters.templateEditor.characterDefault")}
            description={t("characters.templateEditor.useCharacterSystemPrompt")}
            color="from-blue-500 to-cyan-600"
            rightElement={
              selectedPromptTemplateId === null ? (
                <Check className="h-4 w-4 text-emerald-300" />
              ) : undefined
            }
            onClick={() => {
              setSelectedPromptTemplateId(null);
              setShowPromptTemplateMenu(false);
            }}
          />
          {promptTemplates.map((template) => (
            <MenuButton
              key={template.id}
              icon={<MessageSquare className="h-4 w-4" />}
              title={template.name}
              description={template.scope}
              color="from-blue-500 to-cyan-600"
              rightElement={
                selectedPromptTemplateId === template.id ? (
                  <Check className="h-4 w-4 text-emerald-300" />
                ) : undefined
              }
              onClick={() => {
                setSelectedPromptTemplateId(template.id);
                setShowPromptTemplateMenu(false);
              }}
            />
          ))}
        </MenuButtonGroup>
      </BottomMenu>
    </>
  );
}
