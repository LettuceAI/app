import { useState, useEffect, useRef } from "react";
import { AnimatePresence, Reorder, motion, useDragControls, useMotionValue } from "framer-motion";
import { useParams } from "react-router-dom";
import {
  RotateCcw,
  Eye,
  Code2,
  Check,
  AlertTriangle,
  Sparkles,
  Copy,
  Lock,
  ChevronDown,
  ChevronUp,
  GripVertical,
  Plus,
  Trash2,
  Layers,
} from "lucide-react";
import { cn, radius, interactive } from "../../design-tokens";
import { MessageStructurePreview } from "./components/MessageStructurePreview";
import { BottomMenu } from "../../components";
import { confirmBottomMenu } from "../../components/ConfirmBottomMenu";
import { useNavigationManager } from "../../navigation";
import {
  createPromptTemplate,
  updatePromptTemplate,
  getPromptTemplate,
  getAppDefaultTemplateId,
  resetAppDefaultTemplate,
  resetDynamicSummaryTemplate,
  resetDynamicMemoryTemplate,
  getDefaultSystemPromptTemplate,
  renderPromptPreview,
  getRequiredTemplateVariables,
} from "../../../core/prompts/service";
import { listCharacters, listPersonas } from "../../../core/storage";
import type { Character, Persona, SystemPromptEntry } from "../../../core/storage/schemas";
import {
  APP_DYNAMIC_SUMMARY_TEMPLATE_ID,
  APP_DYNAMIC_MEMORY_TEMPLATE_ID,
  APP_HELP_ME_REPLY_TEMPLATE_ID,
  APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_ID,
  APP_GROUP_CHAT_TEMPLATE_ID,
  APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID,
  isProtectedPromptTemplate,
} from "../../../core/prompts/constants";

type PromptType =
  | "system"
  | "summary"
  | "memory"
  | "reply"
  | "group_chat"
  | "group_chat_roleplay"
  | null;

type Variable = {
  var: string;
  label: string;
  desc: string;
};

const VARIABLES_BY_TYPE: Record<string, Variable[]> = {
  system: [
    { var: "{{char.name}}", label: "Character Name", desc: "The character's display name" },
    { var: "{{char.desc}}", label: "Character Definition", desc: "Full character definition" },
    { var: "{{scene}}", label: "Scene", desc: "Starting scene or scenario" },
    { var: "{{rules}}", label: "Rules", desc: "Character behavioral rules" },
    { var: "{{persona.name}}", label: "User Name", desc: "The user's persona name" },
    { var: "{{persona.desc}}", label: "User Description", desc: "User persona description" },
    { var: "{{context_summary}}", label: "Context Summary", desc: "Dynamic conversation summary" },
    { var: "{{key_memories}}", label: "Key Memories", desc: "List of relevant memories" },
  ],
  summary: [
    { var: "{{prev_summary}}", label: "Previous Summary", desc: "The cumulative summary" },
    { var: "{{character}}", label: "Character", desc: "Character placeholder" },
    { var: "{{persona}}", label: "Persona", desc: "Persona placeholder" },
  ],
  memory: [
    { var: "{{max_entries}}", label: "Max Entries", desc: "Maximum memory entries allowed" },
  ],
  reply: [
    { var: "{{char.name}}", label: "Character Name", desc: "The character's display name" },
    { var: "{{char.desc}}", label: "Character Definition", desc: "Full character definition" },
    { var: "{{persona.name}}", label: "User Name", desc: "The user's persona name" },
    { var: "{{persona.desc}}", label: "User Description", desc: "User persona description" },
    { var: "{{current_draft}}", label: "Current Draft", desc: "Content user started writing" },
  ],
  group_chat: [
    { var: "{{char.name}}", label: "Character Name", desc: "The character's display name" },
    { var: "{{char.desc}}", label: "Character Definition", desc: "Full character definition" },
    { var: "{{persona.name}}", label: "User Name", desc: "The user's persona name" },
    { var: "{{persona.desc}}", label: "User Description", desc: "User persona description" },
    { var: "{{group_characters}}", label: "Group Characters", desc: "List of group characters" },
  ],
  group_chat_roleplay: [
    { var: "{{scene}}", label: "Scene", desc: "Starting scene or scenario" },
    { var: "{{scene_direction}}", label: "Scene Direction", desc: "Optional scene direction" },
    { var: "{{char.name}}", label: "Character Name", desc: "The character's display name" },
    { var: "{{char.desc}}", label: "Character Definition", desc: "Full character definition" },
    { var: "{{persona.name}}", label: "User Name", desc: "The user's persona name" },
    { var: "{{persona.desc}}", label: "User Description", desc: "User persona description" },
    { var: "{{group_characters}}", label: "Group Characters", desc: "List of group characters" },
    { var: "{{context_summary}}", label: "Context Summary", desc: "Dynamic conversation summary" },
    { var: "{{key_memories}}", label: "Key Memories", desc: "List of relevant memories" },
  ],
  default: [
    { var: "{{char.name}}", label: "Character Name", desc: "The character's display name" },
    { var: "{{char.desc}}", label: "Character Definition", desc: "Full character definition" },
    { var: "{{scene}}", label: "Scene", desc: "Starting scene or scenario" },
    { var: "{{rules}}", label: "Rules", desc: "Character behavioral rules" },
    { var: "{{persona.name}}", label: "User Name", desc: "The user's persona name" },
    { var: "{{persona.desc}}", label: "User Description", desc: "User persona description" },
    { var: "{{context_summary}}", label: "Context Summary", desc: "Dynamic conversation summary" },
    { var: "{{key_memories}}", label: "Key Memories", desc: "List of relevant memories" },
  ],
};

const ENTRY_ROLE_OPTIONS = [
  { value: "system", label: "System" },
  { value: "user", label: "User" },
  { value: "assistant", label: "Assistant" },
] as const;

const ENTRY_POSITION_OPTIONS = [
  { value: "relative", label: "Relative" },
  { value: "inChat", label: "In Chat" },
  { value: "conditional", label: "Conditional" },
  { value: "interval", label: "Interval" },
] as const;

const DRAG_HOLD_MS = 450;

const createEntryId = () =>
  typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : `entry_${Date.now()}_${Math.random().toString(16).slice(2)}`;

const DEFAULT_ENTRY_ROLE: SystemPromptEntry["role"] = "system";
const DEFAULT_ENTRY_POSITION: SystemPromptEntry["injectionPosition"] = "relative";
const DEFAULT_CONDITIONAL_MIN_MESSAGES = 6;
const DEFAULT_INTERVAL_TURNS = 3;

const createDefaultEntry = (
  content: string,
  overrides?: Partial<SystemPromptEntry>,
): SystemPromptEntry => ({
  id: createEntryId(),
  name: "System Prompt",
  role: DEFAULT_ENTRY_ROLE,
  content,
  enabled: true,
  injectionPosition: DEFAULT_ENTRY_POSITION,
  injectionDepth: 0,
  conditionalMinMessages: null,
  intervalTurns: null,
  systemPrompt: true,
  ...overrides,
});

const createExtraEntry = (overrides?: Partial<SystemPromptEntry>) =>
  createDefaultEntry("", { name: "Prompt Entry", systemPrompt: false, ...overrides });

function getInjectionModeHint(position: SystemPromptEntry["injectionPosition"]) {
  switch (position) {
    case "relative":
      return "Before chat history (system context).";
    case "inChat":
      return "Always inject inside chat history.";
    case "conditional":
      return "Inject only after a minimum number of chat messages.";
    case "interval":
      return "Inject every N chat messages.";
    default:
      return "";
  }
}

const entriesToContent = (entries: SystemPromptEntry[]) =>
  entries
    .map((entry) => entry.content.trim())
    .filter(Boolean)
    .join("\n\n");

const ensureSystemEntry = (entries: SystemPromptEntry[]) => {
  if (entries.length === 0) return [createDefaultEntry("")];
  if (entries.some((entry) => entry.systemPrompt)) return entries;
  return [{ ...entries[0], systemPrompt: true, enabled: true }, ...entries.slice(1)];
};

function PromptEntryCard({
  entry,
  onUpdate,
  onDelete,
  onToggle,
  onToggleCollapse,
  collapsed,
  highlighted,
  onTextareaRef,
  onTextareaFocus,
}: {
  entry: SystemPromptEntry;
  onUpdate: (id: string, updates: Partial<SystemPromptEntry>) => void;
  onDelete: (id: string) => void;
  onToggle: (id: string) => void;
  onToggleCollapse: (id: string) => void;
  collapsed: boolean;
  highlighted?: boolean;
  onTextareaRef: (id: string, el: HTMLTextAreaElement | null) => void;
  onTextareaFocus: (id: string) => void;
}) {
  const controls = useDragControls();
  const toggleId = `prompt-entry-${entry.id}`;

  return (
    <Reorder.Item
      id={`prompt-entry-row-${entry.id}`}
      value={entry}
      layout
      dragListener={false}
      dragControls={controls}
      whileDrag={{
        zIndex: 50,
        scale: 1.025,
        rotate: -0.4,
        boxShadow:
          "0 24px 48px rgba(0,0,0,0.45), 0 8px 16px rgba(0,0,0,0.3), 0 0 0 1px rgba(255,255,255,0.08)",
      }}
      transition={{ layout: { duration: 0.2, ease: "easeOut" } }}
      style={{ position: "relative", zIndex: 0 }}
      className={cn(
        "rounded-xl border bg-fg/5 p-4 space-y-3 cursor-default",
        highlighted
          ? "border-accent/50 ring-2 ring-accent/30 ring-offset-1 ring-offset-black"
          : "border-fg/10",
      )}
    >
      <div className="flex flex-wrap items-center gap-2">
        <button
          onPointerDown={(event) => controls.start(event)}
          className={cn(
            "flex h-8 w-8 items-center justify-center rounded-lg cursor-grab active:cursor-grabbing",
            "border border-fg/10 bg-fg/5 text-fg/40",
          )}
          style={{ touchAction: "none" }}
          title="Drag to reorder"
        >
          <GripVertical className="h-4 w-4" />
        </button>

        <button
          onClick={() => onToggleCollapse(entry.id)}
          className={cn(
            "flex h-8 w-8 items-center justify-center rounded-lg",
            "border border-fg/10 bg-fg/5 text-fg/40",
          )}
          title={collapsed ? "Expand entry" : "Collapse entry"}
        >
          {collapsed ? <ChevronDown className="h-4 w-4" /> : <ChevronUp className="h-4 w-4" />}
        </button>

        <input
          value={entry.name}
          onChange={(event) => onUpdate(entry.id, { name: event.target.value })}
          className="flex-1 rounded-lg border border-fg/10 bg-surface-el/30 px-3 py-2 text-sm text-fg"
          placeholder="Entry name"
        />

        <div className="flex items-center gap-2">
          <div className="flex items-center gap-3">
            <input
              id={toggleId}
              type="checkbox"
              checked={entry.enabled || entry.systemPrompt}
              onChange={() => onToggle(entry.id)}
              onClick={(event) => event.stopPropagation()}
              disabled={entry.systemPrompt}
              className="peer sr-only"
            />
            <label
              htmlFor={toggleId}
              onClick={(event) => event.stopPropagation()}
              className={cn(
                "relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full",
                "border-2 border-transparent transition-all duration-200 ease-in-out",
                "focus:outline-none focus:ring-2 focus:ring-fg/20",
                entry.enabled || entry.systemPrompt ? "bg-accent" : "bg-fg/20",
                entry.systemPrompt && "cursor-not-allowed opacity-60",
              )}
              title={entry.systemPrompt ? "System prompt entries are always enabled" : "Toggle"}
            >
              <span
                className={cn(
                  "inline-block h-4 w-4 transform rounded-full bg-fg shadow-sm",
                  "ring-0 transition duration-200 ease-in-out",
                  entry.enabled || entry.systemPrompt ? "translate-x-4" : "translate-x-0",
                )}
              />
            </label>
            <span className="text-xs text-fg/50">
              {entry.systemPrompt ? "Required" : entry.enabled ? "Enabled" : "Disabled"}
            </span>
          </div>

          {!entry.systemPrompt && (
            <button
              onClick={() => onDelete(entry.id)}
              className={cn(
                "rounded-lg border border-fg/10 p-2 text-fg/40",
                "hover:border-danger/40 hover:bg-danger/10 hover:text-danger/80",
              )}
              title="Delete entry"
            >
              <Trash2 className="h-4 w-4" />
            </button>
          )}
        </div>
      </div>

      {!collapsed && (
        <div className="space-y-3">
          <div className="grid gap-2 md:grid-cols-3">
            <select
              value={entry.role}
              onChange={(event) => onUpdate(entry.id, { role: event.target.value as any })}
              className="h-9 w-full rounded-lg border border-fg/10 bg-surface-el/30 px-2.5 text-xs text-fg"
            >
              {ENTRY_ROLE_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>

            <select
              value={entry.injectionPosition}
              onChange={(event) => {
                const nextPosition = event.target.value as SystemPromptEntry["injectionPosition"];
                onUpdate(entry.id, {
                  injectionPosition: nextPosition,
                  conditionalMinMessages:
                    nextPosition === "conditional"
                      ? (entry.conditionalMinMessages ?? DEFAULT_CONDITIONAL_MIN_MESSAGES)
                      : (entry.conditionalMinMessages ?? null),
                  intervalTurns:
                    nextPosition === "interval"
                      ? (entry.intervalTurns ?? DEFAULT_INTERVAL_TURNS)
                      : (entry.intervalTurns ?? null),
                });
              }}
              className="h-9 w-full rounded-lg border border-fg/10 bg-surface-el/30 px-2.5 text-xs text-fg"
            >
              {ENTRY_POSITION_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>

            {entry.injectionPosition !== "relative" && (
              <div className="flex items-center gap-2">
                <span className="shrink-0 text-[11px] text-fg/50">Depth</span>
                <input
                  type="number"
                  min={0}
                  value={entry.injectionDepth}
                  onChange={(event) =>
                    onUpdate(entry.id, { injectionDepth: Number(event.target.value) })
                  }
                  className="h-9 w-full rounded-lg border border-fg/10 bg-surface-el/30 px-2.5 text-xs text-fg"
                  placeholder="0"
                  title="Insertion Depth"
                  aria-label="Insertion Depth"
                />
              </div>
            )}
          </div>
          <p className="text-[11px] text-fg/50">
            {getInjectionModeHint(entry.injectionPosition)}
          </p>

          {entry.injectionPosition === "conditional" && (
            <div className="space-y-1">
              <p className="text-[11px] text-fg/50">Min Messages</p>
              <input
                type="number"
                min={1}
                value={entry.conditionalMinMessages ?? DEFAULT_CONDITIONAL_MIN_MESSAGES}
                onChange={(event) =>
                  onUpdate(entry.id, {
                    conditionalMinMessages: Math.max(1, Number(event.target.value) || 1),
                  })
                }
                className="h-9 w-full rounded-lg border border-fg/10 bg-surface-el/30 px-2.5 text-xs text-fg"
                placeholder="Inject after at least N messages"
              />
            </div>
          )}

          {entry.injectionPosition === "interval" && (
            <div className="space-y-1">
              <p className="text-[11px] text-fg/50">Every N Messages</p>
              <input
                type="number"
                min={1}
                value={entry.intervalTurns ?? DEFAULT_INTERVAL_TURNS}
                onChange={(event) =>
                  onUpdate(entry.id, {
                    intervalTurns: Math.max(1, Number(event.target.value) || 1),
                  })
                }
                className="h-9 w-full rounded-lg border border-fg/10 bg-surface-el/30 px-2.5 text-xs text-fg"
                placeholder="Inject every N messages"
              />
            </div>
          )}

          <textarea
            ref={(el) => {
              onTextareaRef(entry.id, el);
            }}
            value={entry.content}
            onChange={(event) => onUpdate(entry.id, { content: event.target.value })}
            onFocus={() => onTextareaFocus(entry.id)}
            rows={6}
            className="w-full resize-none rounded-xl border border-fg/10 bg-surface-el/30 px-3.5 py-2.5 font-mono text-sm leading-relaxed text-fg placeholder-fg/30"
            placeholder="Write the prompt entry..."
          />
        </div>
      )}
    </Reorder.Item>
  );
}

function PromptEntryListItem({
  entry,
  onToggle,
  onDelete,
  onEdit,
}: {
  entry: SystemPromptEntry;
  onToggle: (id: string) => void;
  onDelete: (id: string) => void;
  onEdit: (id: string) => void;
}) {
  const controls = useDragControls();
  const dragTimeoutRef = useRef<number | null>(null);
  const draggingRef = useRef(false);
  const pendingEventRef = useRef<PointerEvent | null>(null);
  const scrollLockRef = useRef<{
    el: HTMLElement;
    overflow: string;
    touchAction: string;
    scrollTop: number;
  } | null>(null);
  const toggleId = `prompt-entry-mobile-${entry.id}`;

  const scheduleDragStart = (event: React.PointerEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();
    pendingEventRef.current = event.nativeEvent;
    if (dragTimeoutRef.current) {
      window.clearTimeout(dragTimeoutRef.current);
    }
    dragTimeoutRef.current = window.setTimeout(() => {
      dragTimeoutRef.current = null;
      const pendingEvent = pendingEventRef.current;
      if (pendingEvent) {
        draggingRef.current = true;
        controls.start(pendingEvent);
      }
    }, DRAG_HOLD_MS);
  };

  const cancelDragStart = () => {
    if (dragTimeoutRef.current) {
      window.clearTimeout(dragTimeoutRef.current);
      dragTimeoutRef.current = null;
    }
  };

  const cancelDragStartWithRelease = () => {
    cancelDragStart();
    draggingRef.current = false;
    pendingEventRef.current = null;
  };

  const lockScrollContainer = () => {
    const scrollEl = document.querySelector("main") as HTMLElement | null;
    if (!scrollEl || scrollLockRef.current) return;
    scrollLockRef.current = {
      el: scrollEl,
      overflow: scrollEl.style.overflow,
      touchAction: scrollEl.style.touchAction,
      scrollTop: scrollEl.scrollTop,
    };
    scrollEl.style.overflow = "hidden";
    scrollEl.style.touchAction = "none";
  };

  const unlockScrollContainer = () => {
    if (!scrollLockRef.current) return;
    const { el, overflow, touchAction, scrollTop } = scrollLockRef.current;
    el.style.overflow = overflow;
    el.style.touchAction = touchAction;
    el.scrollTop = scrollTop;
    scrollLockRef.current = null;
  };

  useEffect(() => {
    return () => {
      unlockScrollContainer();
      if (draggingRef.current) {
        document.body.style.overflow = "";
        document.body.style.touchAction = "";
        draggingRef.current = false;
      }
    };
  }, []);

  return (
    <Reorder.Item
      id={`prompt-entry-row-mobile-${entry.id}`}
      value={entry}
      layout
      dragListener={false}
      dragControls={controls}
      whileDrag={{
        zIndex: 50,
        scale: 1.025,
        rotate: -0.4,
        boxShadow:
          "0 24px 48px rgba(0,0,0,0.45), 0 8px 16px rgba(0,0,0,0.3), 0 0 0 1px rgba(255,255,255,0.08)",
      }}
      transition={{ layout: { duration: 0.2, ease: "easeOut" } }}
      style={{ position: "relative", zIndex: 0 }}
      onDragStart={() => {
        draggingRef.current = true;
        document.body.style.overflow = "hidden";
        document.body.style.touchAction = "none";
        lockScrollContainer();
      }}
      onDragEnd={() => {
        draggingRef.current = false;
        document.body.style.overflow = "";
        document.body.style.touchAction = "";
        unlockScrollContainer();
      }}
      onPointerMove={(event) => {
        if (dragTimeoutRef.current) {
          pendingEventRef.current = event.nativeEvent;
        }
        if (draggingRef.current) {
          event.preventDefault();
        }
      }}
      onPointerUp={() => {
        draggingRef.current = false;
        pendingEventRef.current = null;
        unlockScrollContainer();
      }}
      onPointerCancel={() => {
        draggingRef.current = false;
        pendingEventRef.current = null;
        unlockScrollContainer();
      }}
      className={cn("rounded-xl border border-fg/10 bg-fg/5 p-3 select-none", "space-y-2")}
    >
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2 min-w-0">
          <button
            onPointerDown={scheduleDragStart}
            onPointerUp={cancelDragStartWithRelease}
            onPointerLeave={cancelDragStartWithRelease}
            onPointerCancel={cancelDragStartWithRelease}
            onContextMenu={(event) => event.preventDefault()}
            className={cn(
              "flex h-8 w-8 items-center justify-center rounded-lg",
              "border border-fg/10 bg-fg/5 text-fg/40",
            )}
            style={{ touchAction: "none" }}
            title="Drag to reorder"
          >
            <GripVertical className="h-4 w-4" />
          </button>
          <div className="min-w-0">
            <p className="text-sm font-medium text-fg truncate">{entry.name}</p>
            <p className="text-[11px] text-fg/40 uppercase tracking-wide">
              {entry.role} · {entry.injectionPosition}
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <div className="flex items-center gap-2">
            <input
              id={toggleId}
              type="checkbox"
              checked={entry.enabled || entry.systemPrompt}
              onChange={() => onToggle(entry.id)}
              onClick={(event) => event.stopPropagation()}
              disabled={entry.systemPrompt}
              className="peer sr-only"
            />
            <label
              htmlFor={toggleId}
              onClick={(event) => event.stopPropagation()}
              className={cn(
                "relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full",
                "border-2 border-transparent transition-all duration-200 ease-in-out",
                entry.enabled || entry.systemPrompt ? "bg-accent" : "bg-fg/20",
                entry.systemPrompt && "cursor-not-allowed opacity-60",
              )}
              title={entry.systemPrompt ? "System prompt entries are always enabled" : "Toggle"}
            >
              <span
                className={cn(
                  "inline-block h-4 w-4 transform rounded-full bg-fg shadow-sm",
                  "ring-0 transition duration-200 ease-in-out",
                  entry.enabled || entry.systemPrompt ? "translate-x-4" : "translate-x-0",
                )}
              />
            </label>
          </div>

          <button
            onClick={() => onEdit(entry.id)}
            className={cn(
              "rounded-lg border border-fg/10 px-3 py-1.5 text-xs font-medium text-fg/70",
              "hover:bg-fg/10 hover:text-fg",
            )}
          >
            Edit
          </button>

          {!entry.systemPrompt && (
            <button
              onClick={() => onDelete(entry.id)}
              className={cn(
                "rounded-lg border border-fg/10 p-2 text-fg/40",
                "hover:border-danger/40 hover:bg-danger/10 hover:text-danger/80",
              )}
              title="Delete entry"
            >
              <Trash2 className="h-4 w-4" />
            </button>
          )}
        </div>
      </div>

      <p className="text-xs text-fg/50 line-clamp-2">
        {entry.content?.trim() || "No content yet"}
      </p>
    </Reorder.Item>
  );
}

function getPromptTypeName(type: PromptType): string {
  switch (type) {
    case "system":
      return "System Prompt";
    case "summary":
      return "Dynamic Summary";
    case "memory":
      return "Dynamic Memory";
    case "reply":
      return "Reply Helper";
    case "group_chat":
      return "Group Chat";
    case "group_chat_roleplay":
      return "Group Chat RP";
    default:
      return "Custom Prompt";
  }
}

function LoadingSkeleton() {
  return (
    <div className="flex h-full flex-col pb-16">
      <main className="flex-1 overflow-y-auto px-4 pt-4">
        <div className="mx-auto w-full max-w-5xl space-y-4">
          <div className="h-12 w-full animate-pulse rounded-xl bg-fg/10" />
          <div className="h-80 w-full animate-pulse rounded-xl bg-fg/10" />
        </div>
      </main>
    </div>
  );
}

export function EditPromptTemplate() {
  const { backOrReplace } = useNavigationManager();
  const { id } = useParams<{ id: string }>();
  const isEditing = !!id;
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const sidebarRef = useRef<HTMLDivElement>(null);
  const entryTextareaRefs = useRef<Record<string, HTMLTextAreaElement | null>>({});
  const activeEntryIdRef = useRef<string | null>(null);
  const entriesRef = useRef<SystemPromptEntry[]>([]);
  const nameRef = useRef("");
  const contentRef = useRef("");
  const savingRef = useRef(false);
  const initialRef = useRef<{
    name: string;
    content: string;
    entries: string;
    condensePromptEntries: boolean;
  } | null>(null);

  // Form state
  const [name, setName] = useState("");
  const [content, setContent] = useState("");
  const [entries, setEntries] = useState<SystemPromptEntry[]>([]);
  const [condensePromptEntries, setCondensePromptEntries] = useState(false);

  // Preview state
  const [characters, setCharacters] = useState<Character[]>([]);
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [previewCharacterId, setPreviewCharacterId] = useState<string | null>(null);
  const [previewPersonaId, setPreviewPersonaId] = useState<string | null>(null);
  const [preview, setPreview] = useState<string>("");
  const [previewEntries, setPreviewEntries] = useState<SystemPromptEntry[]>([]);
  const [previewing, setPreviewing] = useState(false);
  const [previewMode, setPreviewMode] = useState<"rendered" | "raw">("rendered");
  const [previewExpanded, setPreviewExpanded] = useState(false);
  const [collapsedEntries, setCollapsedEntries] = useState<Record<string, boolean>>({});
  const [mobileEntryEditorId, setMobileEntryEditorId] = useState<string | null>(null);

  // UI state
  const [loading, setLoading] = useState(isEditing);
  const [saving, setSaving] = useState(false);
  const [showVariables, setShowVariables] = useState(false);
  const [showMobilePreview, setShowMobilePreview] = useState(false);
  const [editorView, setEditorView] = useState<"entries" | "structure">("entries");
  const [mobilePreviewTab, setMobilePreviewTab] = useState<"content" | "structure">("content");
  const [copiedVar, setCopiedVar] = useState<string | null>(null);
  const [highlightedEntryId, setHighlightedEntryId] = useState<string | null>(null);

  // Template metadata
  const [isAppDefault, setIsAppDefault] = useState(false);
  const [promptType, setPromptType] = useState<PromptType>(null);
  const [resetting, setResetting] = useState(false);
  const [requiredVariables, setRequiredVariables] = useState<string[]>([]);
  const [missingVariables, setMissingVariables] = useState<string[]>([]);

  const canReset =
    isAppDefault &&
    (promptType === "system" ||
      promptType === "summary" ||
      promptType === "memory" ||
      promptType === "reply");

  const usesEntryEditor = true;
  const quickInsertY = useMotionValue(0);
  const [scrollListenerMounted, setScrollListenerMounted] = useState(false);

  // Trigger scroll listener setup after component mounts
  useEffect(() => {
    setScrollListenerMounted(true);
  }, []);

  useEffect(() => {
    if (!scrollListenerMounted) return;

    const getScrollParent = (node: HTMLElement | null): HTMLElement | null => {
      let current = node?.parentElement ?? null;
      while (current) {
        const style = getComputedStyle(current);
        const overflowY = style.overflowY;
        if (
          (overflowY === "auto" || overflowY === "scroll" || overflowY === "overlay") &&
          current.scrollHeight > current.clientHeight
        ) {
          return current;
        }
        current = current.parentElement;
      }
      return null;
    };

    const target = sidebarRef.current;
    if (!target) return;

    const scrollParent = getScrollParent(target);

    const handleScroll = () => {
      const scrollTop = scrollParent ? scrollParent.scrollTop : window.scrollY;
      quickInsertY.set(scrollTop);
    };

    const options: AddEventListenerOptions = { passive: true };
    (scrollParent ?? window).addEventListener("scroll", handleScroll, options);
    handleScroll();
    return () => (scrollParent ?? window).removeEventListener("scroll", handleScroll, options);
  }, [scrollListenerMounted, quickInsertY]);

  const variables = VARIABLES_BY_TYPE[promptType || "default"] || VARIABLES_BY_TYPE.default;

  const contentValue = usesEntryEditor ? entriesToContent(entries) : content;
  const charCount = contentValue.length;
  const charCountColor =
    charCount > 8000 ? "text-danger/80" : charCount > 5000 ? "text-warning/80" : "text-fg/40";

  const hasEntryContent = entries.some((entry) => entry.content.trim().length > 0);
  const hasContent = content.trim().length > 0;
  const serializeEntries = (items: SystemPromptEntry[]) =>
    JSON.stringify(
      items.map((entry) => ({
        id: entry.id,
        name: entry.name,
        role: entry.role,
        content: entry.content,
        enabled: entry.enabled,
        injectionPosition: entry.injectionPosition,
        injectionDepth: entry.injectionDepth,
        conditionalMinMessages: entry.conditionalMinMessages ?? null,
        intervalTurns: entry.intervalTurns ?? null,
        systemPrompt: entry.systemPrompt,
      })),
    );
  const isDirty =
    !loading &&
    initialRef.current !== null &&
    (name.trim() !== initialRef.current.name ||
      content !== initialRef.current.content ||
      serializeEntries(entries) !== initialRef.current.entries ||
      condensePromptEntries !== initialRef.current.condensePromptEntries);
  const canSave = isDirty && name.trim().length > 0 && (hasEntryContent || hasContent);

  // Expose save state to TopNav via window globals
  useEffect(() => {
    const globalWindow = window as any;
    globalWindow.__savePromptCanSave = canSave && !saving;
    globalWindow.__savePromptSaving = saving;

    return () => {
      delete globalWindow.__savePromptCanSave;
      delete globalWindow.__savePromptSaving;
    };
  }, [canSave, saving]);

  useEffect(() => {
    const globalWindow = window as any;
    const handleDiscard = () => resetToInitial();
    globalWindow.__discardChanges = handleDiscard;
    window.addEventListener("unsaved:discard", handleDiscard);
    return () => {
      if (globalWindow.__discardChanges === handleDiscard) {
        delete globalWindow.__discardChanges;
      }
      window.removeEventListener("unsaved:discard", handleDiscard);
    };
  }, [id]);

  useEffect(() => {
    initialRef.current = null;
  }, [id]);

  useEffect(() => {
    entriesRef.current = entries;
  }, [entries]);

  useEffect(() => {
    nameRef.current = name;
  }, [name]);

  useEffect(() => {
    contentRef.current = content;
  }, [content]);

  useEffect(() => {
    savingRef.current = saving;
  }, [saving]);

  // Listen for save event from TopNav
  useEffect(() => {
    const handleSave = () => {
      if (canSave && !savingRef.current) {
        handleSave_internal();
      }
    };

    window.addEventListener("prompt:save", handleSave);
    return () => window.removeEventListener("prompt:save", handleSave);
  }, [canSave]);

  useEffect(() => {
    loadData();
  }, []);

  useEffect(() => {
    if (isAppDefault && requiredVariables.length > 0) {
      const source = usesEntryEditor ? entriesToContent(entries) : content;
      const missing = requiredVariables.filter((v) => !source.includes(v));
      setMissingVariables(missing);
    }
  }, [content, entries, requiredVariables, isAppDefault, usesEntryEditor]);

  async function loadData() {
    try {
      const [chars, pers] = await Promise.all([listCharacters(), listPersonas()]);
      setCharacters(chars);
      setPersonas(pers);
      setPreviewCharacterId(chars[0]?.id ?? null);
      setPreviewPersonaId(pers.find((p) => p.isDefault)?.id ?? null);

      if (isEditing && id) {
        const [template, appDefaultId] = await Promise.all([
          getPromptTemplate(id),
          getAppDefaultTemplateId(),
        ]);

        if (template) {
          setName(template.name);
          setContent(template.content);
          const isProtected =
            template.id === appDefaultId || isProtectedPromptTemplate(template.id);
          setIsAppDefault(isProtected);

          let detectedType: PromptType = null;
          if (template.id === appDefaultId) {
            detectedType = "system";
          } else if (template.id === APP_DYNAMIC_SUMMARY_TEMPLATE_ID) {
            detectedType = "summary";
          } else if (template.id === APP_DYNAMIC_MEMORY_TEMPLATE_ID) {
            detectedType = "memory";
          } else if (template.id === APP_HELP_ME_REPLY_TEMPLATE_ID) {
            detectedType = "reply";
          } else if (template.id === APP_HELP_ME_REPLY_CONVERSATIONAL_TEMPLATE_ID) {
            detectedType = "reply";
          } else if (template.id === APP_GROUP_CHAT_TEMPLATE_ID) {
            detectedType = "group_chat";
          } else if (template.id === APP_GROUP_CHAT_ROLEPLAY_TEMPLATE_ID) {
            detectedType = "group_chat_roleplay";
          }
          setPromptType(detectedType);

          const nextEntries =
            template.entries?.length > 0
              ? template.entries
              : [createDefaultEntry(template.content)];
          const normalizedEntries = ensureSystemEntry(nextEntries);
          setEntries(normalizedEntries);
          setCondensePromptEntries(Boolean(template.condensePromptEntries));
          setCollapsedEntries(
            Object.fromEntries(normalizedEntries.map((entry) => [entry.id, true])),
          );
          initialRef.current = {
            name: template.name,
            content: template.content,
            entries: serializeEntries(normalizedEntries),
            condensePromptEntries: Boolean(template.condensePromptEntries),
          };

          if (isProtected) {
            const required = await getRequiredTemplateVariables(template.id);
            setRequiredVariables(required);
          }
        }
      } else {
        const def = await getDefaultSystemPromptTemplate();
        setContent(def);
        const seedEntries = [createDefaultEntry(def)];
        setEntries(seedEntries);
        setCondensePromptEntries(false);
        setCollapsedEntries(Object.fromEntries(seedEntries.map((entry) => [entry.id, true])));
        initialRef.current = {
          name: "",
          content: def,
          entries: serializeEntries(seedEntries),
          condensePromptEntries: false,
        };
      }
    } catch (error) {
      console.error("Failed to load data:", error);
    } finally {
      setLoading(false);
    }
  }

  const handleEntryUpdate = (id: string, updates: Partial<SystemPromptEntry>) => {
    setEntries((prev) => prev.map((entry) => (entry.id === id ? { ...entry, ...updates } : entry)));
  };

  const handleToggleEntryCollapse = (id: string) => {
    setCollapsedEntries((prev) => ({ ...prev, [id]: !prev[id] }));
  };

  const handleEntryDelete = (id: string) => {
    setEntries((prev) => prev.filter((entry) => entry.id !== id));
  };

  const handleEntryToggle = (id: string) => {
    setEntries((prev) =>
      prev.map((entry) => {
        if (entry.id !== id || entry.systemPrompt) return entry;
        return { ...entry, enabled: !entry.enabled };
      }),
    );
  };

  const handleAddEntry = () => {
    const entry = createExtraEntry();
    setEntries((prev) => [...prev, entry]);
    setCollapsedEntries((prev) => ({ ...prev, [entry.id]: false }));
    window.setTimeout(() => {
      const isMobile = window.matchMedia("(max-width: 1023px)").matches;
      const targetId = isMobile
        ? `prompt-entry-row-mobile-${entry.id}`
        : `prompt-entry-row-${entry.id}`;
      const target = document.getElementById(targetId);
      if (target) {
        target.scrollIntoView({ behavior: "smooth", block: "center" });
      }
    }, 150);
  };

  const handleStructureEdit = (entryId: string) => {
    setEditorView("entries");
    setCollapsedEntries((prev) => ({ ...prev, [entryId]: false }));
    window.setTimeout(() => {
      const isMobile = window.matchMedia("(max-width: 1023px)").matches;
      const targetId = isMobile
        ? `prompt-entry-row-mobile-${entryId}`
        : `prompt-entry-row-${entryId}`;
      document.getElementById(targetId)?.scrollIntoView({ behavior: "smooth", block: "center" });
      window.setTimeout(() => {
        entryTextareaRefs.current[entryId]?.focus();
      }, 300);
    }, 200);
  };

  const handleStructureDelete = (entryId: string) => {
    handleEntryDelete(entryId);
  };

  const handleStructureReorder = (entryId: string) => {
    setEditorView("entries");
    setHighlightedEntryId(entryId);
    window.setTimeout(() => {
      const isMobile = window.matchMedia("(max-width: 1023px)").matches;
      const targetId = isMobile
        ? `prompt-entry-row-mobile-${entryId}`
        : `prompt-entry-row-${entryId}`;
      document.getElementById(targetId)?.scrollIntoView({ behavior: "smooth", block: "center" });
    }, 200);
    window.setTimeout(() => setHighlightedEntryId(null), 4000);
  };

  const selectedMobileEntry = mobileEntryEditorId
    ? (entries.find((entry) => entry.id === mobileEntryEditorId) ?? null)
    : null;

  async function handleSave_internal() {
    const entriesSnapshot = entriesRef.current;
    const nameSnapshot = nameRef.current.trim();
    const contentSnapshot = contentRef.current;
    const hasContent = usesEntryEditor
      ? entriesSnapshot.some((entry) => entry.content.trim().length > 0)
      : contentSnapshot.trim().length > 0;
    if (!nameSnapshot || !hasContent) return;

    if (isAppDefault && id && missingVariables.length > 0) {
      alert(`Cannot save: Missing required variables: ${missingVariables.join(", ")}`);
      return;
    }

    setSaving(true);
    try {
      const contentToSave = usesEntryEditor
        ? entriesToContent(entriesSnapshot)
        : contentSnapshot.trim();
      if (isEditing && id) {
        await updatePromptTemplate(id, {
          name: nameSnapshot,
          content: contentToSave,
          entries: usesEntryEditor ? entriesSnapshot : undefined,
          condensePromptEntries,
        });
      } else {
        await createPromptTemplate(
          nameSnapshot,
          "appWide" as any,
          [],
          contentToSave,
          usesEntryEditor ? entriesSnapshot : undefined,
          condensePromptEntries,
        );
      }
      backOrReplace("/settings/prompts");
    } catch (error) {
      console.error("Failed to save template:", error);
      alert("Failed to save template: " + String(error));
    } finally {
      setSaving(false);
    }
  }

  async function handleReset() {
    if (!isAppDefault || !promptType) return;
    if (!["system", "summary", "memory", "reply"].includes(promptType)) return;

    const promptTypeName = getPromptTypeName(promptType);
    const confirmed = await confirmBottomMenu({
      title: `Reset ${promptTypeName}?`,
      message: `Reset to the original default ${promptTypeName}? This cannot be undone.`,
      confirmLabel: "Reset",
      destructive: true,
    });
    if (!confirmed) return;

    setResetting(true);
    try {
      let updated;
      if (promptType === "system") {
        updated = await resetAppDefaultTemplate();
      } else if (promptType === "summary") {
        updated = await resetDynamicSummaryTemplate();
      } else if (promptType === "memory") {
        updated = await resetDynamicMemoryTemplate();
      } else {
        const { resetHelpMeReplyTemplate } = await import("../../../core/prompts/service");
        updated = await resetHelpMeReplyTemplate();
      }
      setContent(updated.content);
      setCondensePromptEntries(Boolean(updated.condensePromptEntries));
      if (usesEntryEditor) {
        const nextEntries =
          updated.entries?.length > 0 ? updated.entries : [createDefaultEntry(updated.content)];
        const normalizedEntries = ensureSystemEntry(nextEntries);
        setEntries(normalizedEntries);
        setCollapsedEntries(Object.fromEntries(normalizedEntries.map((entry) => [entry.id, true])));
      }
    } catch (error) {
      console.error("Failed to reset template:", error);
      alert("Failed to reset template");
    } finally {
      setResetting(false);
    }
  }

  const resetToInitial = () => {
    if (!initialRef.current) return;
    try {
      const nextEntries = JSON.parse(initialRef.current.entries) as SystemPromptEntry[];
      setName(initialRef.current.name);
      setContent(initialRef.current.content);
      setEntries(nextEntries);
      setCondensePromptEntries(initialRef.current.condensePromptEntries);
      setCollapsedEntries(Object.fromEntries(nextEntries.map((entry) => [entry.id, true])));
      setMobileEntryEditorId(null);
    } catch (error) {
      console.error("Failed to reset prompt editor:", error);
    }
  };

  async function handlePreview() {
    if (!previewCharacterId) return;
    setPreviewing(true);
    try {
      if (usesEntryEditor) {
        if (previewMode === "raw") {
          setPreviewEntries(entries);
        } else {
          const renderedEntries = await Promise.all(
            entries.map(async (entry) => {
              const rendered = await renderPromptPreview(entry.content, {
                characterId: previewCharacterId,
                personaId: previewPersonaId ?? undefined,
              });
              return { ...entry, content: rendered };
            }),
          );
          setPreviewEntries(renderedEntries);
        }
      } else {
        const rendered = await renderPromptPreview(content, {
          characterId: previewCharacterId,
          personaId: previewPersonaId ?? undefined,
        });
        setPreview(rendered);
      }
    } catch (e) {
      console.error("Preview failed", e);
      setPreview("<failed to render preview>");
      if (usesEntryEditor) {
        setPreviewEntries([]);
      }
    } finally {
      setPreviewing(false);
    }
  }

  async function copyVariable(variable: string) {
    await navigator.clipboard.writeText(variable);
    setCopiedVar(variable);
    setTimeout(() => setCopiedVar(null), 2000);
  }

  function insertVariable(variable: string) {
    if (usesEntryEditor) {
      const targetId = activeEntryIdRef.current;
      const targetEl = targetId ? entryTextareaRefs.current[targetId] : null;
      if (targetId && targetEl) {
        const start = targetEl.selectionStart ?? 0;
        const end = targetEl.selectionEnd ?? start;
        setEntries((prev) =>
          prev.map((entry) => {
            if (entry.id !== targetId) return entry;
            const nextContent =
              entry.content.substring(0, start) + variable + entry.content.substring(end);
            return { ...entry, content: nextContent };
          }),
        );
        setTimeout(() => {
          const el = entryTextareaRefs.current[targetId];
          if (!el) return;
          el.focus();
          const newPos = start + variable.length;
          el.setSelectionRange(newPos, newPos);
        }, 0);
        return;
      }
      setEntries((prev) => {
        if (prev.length === 0) return prev;
        const targetIndex = prev.findIndex((entry) => entry.systemPrompt);
        const index = targetIndex >= 0 ? targetIndex : 0;
        const next = [...prev];
        next[index] = {
          ...next[index],
          content: `${next[index].content}${next[index].content ? "\n" : ""}${variable}`,
        };
        return next;
      });
      return;
    }
    if (!textareaRef.current) return;

    const textarea = textareaRef.current;
    const start = textarea.selectionStart;
    const end = textarea.selectionEnd;

    const newContent = content.substring(0, start) + variable + content.substring(end);
    setContent(newContent);

    setTimeout(() => {
      textarea.focus();
      const newPos = start + variable.length;
      textarea.setSelectionRange(newPos, newPos);
    }, 0);
  }

  if (loading) {
    return <LoadingSkeleton />;
  }

  // Preview Panel Component (used in both desktop inline and mobile sheet)
  const PreviewPanel = ({ isMobile = false }: { isMobile?: boolean }) => (
    <div className={cn("space-y-3", isMobile ? "" : "")}>
      {/* Mode Toggle */}
      <div className="flex items-center gap-1 p-1 rounded-lg border border-fg/10 bg-fg/5">
        <button
          onClick={() => setPreviewMode("rendered")}
          className={cn(
            "flex-1 flex items-center justify-center gap-1.5 px-3 py-1.5",
            radius.md,
            "text-xs font-medium transition",
            previewMode === "rendered"
              ? "bg-accent/20 text-accent/80"
              : "text-fg/50 hover:text-fg/70",
          )}
        >
          <Sparkles className="h-3.5 w-3.5" />
          Rendered
        </button>
        <button
          onClick={() => setPreviewMode("raw")}
          className={cn(
            "flex-1 flex items-center justify-center gap-1.5 px-3 py-1.5",
            radius.md,
            "text-xs font-medium transition",
            previewMode === "raw"
              ? "bg-accent/20 text-accent/80"
              : "text-fg/50 hover:text-fg/70",
          )}
        >
          <Code2 className="h-3.5 w-3.5" />
          Raw
        </button>
      </div>

      {/* Character/Persona Selection */}
      {previewMode === "rendered" && (
        <>
          <div className="grid grid-cols-2 gap-2">
            <select
              value={previewCharacterId ?? ""}
              onChange={(e) => setPreviewCharacterId(e.target.value || null)}
              className={cn(
                "w-full px-3 py-2",
                radius.md,
                "border border-fg/10 bg-fg/5",
                "text-sm text-fg",
                "focus:border-fg/20 focus:outline-none",
              )}
            >
              <option value="">Select character…</option>
              {characters.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>

            <select
              value={previewPersonaId ?? ""}
              onChange={(e) => setPreviewPersonaId(e.target.value || null)}
              className={cn(
                "w-full px-3 py-2",
                radius.md,
                "border border-fg/10 bg-fg/5",
                "text-sm text-fg",
                "focus:border-fg/20 focus:outline-none",
              )}
            >
              <option value="">Select persona…</option>
              {personas.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.title}
                </option>
              ))}
            </select>
          </div>

          <button
            onClick={handlePreview}
            disabled={!previewCharacterId || previewing}
            className={cn(
              "w-full py-2",
              radius.md,
              "border text-sm font-medium transition",
              !previewCharacterId || previewing
                ? "border-fg/10 bg-fg/5 text-fg/30 cursor-not-allowed"
                : "border-accent/40 bg-accent/15 text-accent/80 hover:bg-accent/25",
            )}
          >
            {previewing ? "Rendering…" : "Generate Preview"}
          </button>
        </>
      )}

      {/* Preview Output */}
      <div
        className={cn(
          "overflow-auto",
          radius.lg,
          "border border-fg/10 bg-surface-el/30 p-4",
          isMobile ? "max-h-80" : "max-h-64",
        )}
      >
        {usesEntryEditor ? (
          (() => {
            const entriesToShow = previewMode === "rendered" ? previewEntries : entries;
            if (previewMode === "rendered" && entriesToShow.length === 0) {
              return (
                <div className="flex flex-col items-center justify-center h-full py-8 text-center">
                  <Eye className="h-8 w-8 text-fg/20 mb-2" />
                  <p className="text-sm text-fg/50">No preview yet</p>
                  <p className="text-xs text-fg/30">Select a character and generate</p>
                </div>
              );
            }
            if (entriesToShow.length === 0) {
              return <p className="text-xs text-fg/40">No entries to preview</p>;
            }
            return (
              <div className="space-y-4">
                {entriesToShow.map((entry) => (
                  <div key={entry.id} className="space-y-1">
                    <div className="text-[11px] uppercase tracking-wide text-fg/40">
                      {entry.role} · {entry.name}
                    </div>
                    <pre className="whitespace-pre-wrap text-xs leading-relaxed text-fg/80 font-mono">
                      {entry.content || "No content"}
                    </pre>
                  </div>
                ))}
              </div>
            );
          })()
        ) : previewMode === "rendered" ? (
          preview ? (
            <pre className="whitespace-pre-wrap text-xs leading-relaxed text-fg/80 font-mono">
              {preview}
            </pre>
          ) : (
            <div className="flex flex-col items-center justify-center h-full py-8 text-center">
              <Eye className="h-8 w-8 text-fg/20 mb-2" />
              <p className="text-sm text-fg/50">No preview yet</p>
              <p className="text-xs text-fg/30">Select a character and generate</p>
            </div>
          )
        ) : (
          <pre className="whitespace-pre-wrap text-xs leading-relaxed text-fg/80 font-mono">
            {content || "No content to preview"}
          </pre>
        )}
      </div>
    </div>
  );

  return (
    <div className="flex h-full flex-col pb-16">
      <main className="flex-1 overflow-y-auto px-4 pt-4">
        <div className="mx-auto w-full max-w-5xl">
          {/* Desktop: Two column layout */}
          <div className="flex flex-col lg:flex-row lg:gap-6">
            {/* Main Editor Column */}
            <div className="flex-1 space-y-4 min-w-0">
              {/* Protected Template Notice */}
              {isAppDefault && (
                <div className={cn(radius.lg, "border border-warning/30 bg-warning/10 p-3")}>
                  <div className="flex items-center justify-between gap-3">
                    <div className="flex items-center gap-2 min-w-0">
                      <Lock className="h-4 w-4 text-warning/80 shrink-0" />
                      <div className="min-w-0">
                        <span className="text-sm font-medium text-warning/80">Protected</span>
                        {promptType && (
                          <span className="text-xs text-warning/70 ml-2">
                            {getPromptTypeName(promptType)}
                          </span>
                        )}
                      </div>
                    </div>
                    {canReset && (
                      <button
                        onClick={handleReset}
                        disabled={resetting}
                        className={cn(
                          "flex items-center gap-1.5 px-3 py-1.5 shrink-0",
                          radius.md,
                          "text-xs font-medium text-warning/80",
                          "hover:bg-warning/20",
                          interactive.transition.fast,
                          "disabled:opacity-50",
                        )}
                      >
                        <RotateCcw className={cn("h-3.5 w-3.5", resetting && "animate-spin")} />
                        Reset
                      </button>
                    )}
                  </div>
                </div>
              )}

              {/* Missing Variables Warning */}
              <AnimatePresence>
                {isAppDefault && missingVariables.length > 0 && (
                  <motion.div
                    initial={{ opacity: 0, height: 0 }}
                    animate={{ opacity: 1, height: "auto" }}
                    exit={{ opacity: 0, height: 0 }}
                    className={cn(radius.lg, "border border-danger/30 bg-danger/10 p-3")}
                  >
                    <div className="flex items-start gap-2">
                      <AlertTriangle className="h-4 w-4 text-danger/80 shrink-0 mt-0.5" />
                      <div>
                        <p className="text-sm font-medium text-danger/80">
                          Missing Required Variables
                        </p>
                        <p className="text-xs text-danger/70 mt-0.5">
                          Include: <span className="font-mono">{missingVariables.join(", ")}</span>
                        </p>
                      </div>
                    </div>
                  </motion.div>
                )}
              </AnimatePresence>

              {/* Name Input */}
              <div className="space-y-2">
                <label className="text-xs font-medium uppercase tracking-wider text-fg/50">
                  Template Name
                </label>
                <input
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="e.g., Creative Roleplay"
                  className={cn(
                    "w-full px-4 py-3",
                    radius.lg,
                    "border border-fg/10 bg-fg/5",
                    "text-sm text-fg placeholder-fg/30",
                    interactive.transition.fast,
                    "focus:border-fg/20 focus:bg-fg/10 focus:outline-none",
                  )}
                />
              </div>

              {/* Content Editor */}
              <div className="space-y-2">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  {usesEntryEditor ? (
                    <div className="flex items-center gap-1 p-0.5 rounded-md border border-fg/10 bg-surface-el/20">
                      <button
                        onClick={() => setEditorView("entries")}
                        className={cn(
                          "px-2.5 py-1 text-xs font-medium",
                          radius.sm,
                          "transition",
                          editorView === "entries"
                            ? "bg-fg/10 text-fg"
                            : "text-fg/40 hover:text-fg/60",
                        )}
                      >
                        Entries
                      </button>
                      <button
                        onClick={() => setEditorView("structure")}
                        className={cn(
                          "flex items-center gap-1.5 px-2.5 py-1 text-xs font-medium",
                          radius.sm,
                          "transition",
                          editorView === "structure"
                            ? "bg-fg/10 text-fg"
                            : "text-fg/40 hover:text-fg/60",
                        )}
                      >
                        <Layers className="h-3 w-3" />
                        Structure
                      </button>
                    </div>
                  ) : (
                    <label className="text-xs font-medium uppercase tracking-wider text-fg/50">
                      Prompt Content
                    </label>
                  )}
                  {usesEntryEditor && (
                    <div className="flex items-center gap-3 rounded-lg border border-fg/10 bg-surface-el/20 px-2.5 py-1.5">
                      <input
                        id="condense-prompt-entries"
                        type="checkbox"
                        checked={condensePromptEntries}
                        onChange={() => setCondensePromptEntries((prev) => !prev)}
                        className="peer sr-only"
                      />
                      <label
                        htmlFor="condense-prompt-entries"
                        className={cn(
                          "relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full",
                          "border-2 border-transparent transition-all duration-200 ease-in-out",
                          condensePromptEntries ? "bg-accent" : "bg-fg/20",
                        )}
                      >
                        <span
                          className={cn(
                            "inline-block h-4 w-4 transform rounded-full bg-fg shadow-sm",
                            "ring-0 transition duration-200 ease-in-out",
                            condensePromptEntries ? "translate-x-4" : "translate-x-0",
                          )}
                        />
                      </label>
                      <span className="text-xs text-fg/70">
                        Send entries as one system message
                      </span>
                    </div>
                  )}
                  <div className="flex flex-wrap items-center gap-2">
                    {usesEntryEditor && (
                      <button
                        onClick={handleAddEntry}
                        className={cn(
                          "flex items-center gap-1.5 px-2.5 py-1.5",
                          radius.md,
                          "border border-accent/30 bg-accent/10",
                          "text-xs font-medium text-accent/80",
                          interactive.transition.fast,
                          "hover:bg-accent/20",
                        )}
                      >
                        <Plus className="h-3.5 w-3.5" />
                        Add Entry
                      </button>
                    )}
                    <button
                      onClick={() => setShowVariables(true)}
                      className={cn(
                        "flex items-center gap-1.5 px-2.5 py-1.5",
                        radius.md,
                        "border border-info/30 bg-info/10",
                        "text-xs font-medium text-info/80",
                        interactive.transition.fast,
                        "hover:bg-info/20",
                      )}
                    >
                      <Sparkles className="h-3.5 w-3.5" />
                      Variables
                    </button>
                    <button
                      onClick={() => setShowMobilePreview(true)}
                      className={cn(
                        "flex items-center gap-1.5 px-2.5 py-1.5 lg:hidden",
                        radius.md,
                        "border border-fg/10 bg-fg/5",
                        "text-xs font-medium text-fg/70",
                        interactive.transition.fast,
                        "hover:bg-fg/10",
                      )}
                    >
                      <Eye className="h-3.5 w-3.5" />
                      Preview
                    </button>
                  </div>
                </div>

                {usesEntryEditor ? (
                  <AnimatePresence mode="wait" initial={false}>
                    {editorView === "structure" ? (
                      <motion.div
                        key="structure-view"
                        initial={{ opacity: 0 }}
                        animate={{ opacity: 1 }}
                        exit={{ opacity: 0 }}
                        transition={{ duration: 0.15 }}
                      >
                        <MessageStructurePreview
                          entries={entries}
                          condensePromptEntries={condensePromptEntries}
                          onEditEntry={handleStructureEdit}
                          onDeleteEntry={handleStructureDelete}
                          onReorderEntry={handleStructureReorder}
                        />
                      </motion.div>
                    ) : (
                      <motion.div
                        key="entries-view"
                        initial={{ opacity: 0 }}
                        animate={{ opacity: 1 }}
                        exit={{ opacity: 0 }}
                        transition={{ duration: 0.15 }}
                        className="space-y-3"
                      >
                        <Reorder.Group
                          axis="y"
                          values={entries}
                          onReorder={setEntries}
                          className="hidden lg:flex lg:flex-col gap-3"
                        >
                          {entries.map((entry) => (
                            <PromptEntryCard
                              key={entry.id}
                              entry={entry}
                              onUpdate={handleEntryUpdate}
                              onDelete={handleEntryDelete}
                              onToggle={handleEntryToggle}
                              onToggleCollapse={handleToggleEntryCollapse}
                              collapsed={collapsedEntries[entry.id] ?? true}
                              highlighted={highlightedEntryId === entry.id}
                              onTextareaRef={(id, el) => {
                                entryTextareaRefs.current[id] = el;
                              }}
                              onTextareaFocus={(id) => {
                                activeEntryIdRef.current = id;
                              }}
                            />
                          ))}
                        </Reorder.Group>

                        <Reorder.Group
                          axis="y"
                          values={entries}
                          onReorder={setEntries}
                          className="flex flex-col gap-2 lg:hidden"
                        >
                          {entries.map((entry) => (
                            <PromptEntryListItem
                              key={entry.id}
                              entry={entry}
                              onToggle={handleEntryToggle}
                              onDelete={handleEntryDelete}
                              onEdit={(id) => setMobileEntryEditorId(id)}
                            />
                          ))}
                        </Reorder.Group>

                        <div className="flex items-center justify-end">
                          <span
                            className={cn(
                              "px-2 py-1 rounded-md bg-surface-el/60",
                              "text-xs font-medium",
                              charCountColor,
                            )}
                          >
                            {charCount.toLocaleString()}
                          </span>
                        </div>
                      </motion.div>
                    )}
                  </AnimatePresence>
                ) : (
                  <div className="relative">
                    <textarea
                      ref={textareaRef}
                      value={content}
                      onChange={(e) => setContent(e.target.value)}
                      placeholder="You are a creative and engaging AI assistant..."
                      rows={20}
                      className={cn(
                        "w-full px-4 py-3 resize-none",
                        radius.lg,
                        "border border-fg/10 bg-fg/5",
                        "font-mono text-sm leading-relaxed text-fg placeholder-fg/30",
                        interactive.transition.fast,
                        "focus:border-fg/20 focus:bg-fg/10 focus:outline-none",
                      )}
                    />
                    <div className="absolute bottom-3 right-3 pointer-events-none">
                      <span
                        className={cn(
                          "px-2 py-1 rounded-md bg-surface-el/60",
                          "text-xs font-medium",
                          charCountColor,
                        )}
                      >
                        {charCount.toLocaleString()}
                      </span>
                    </div>
                  </div>
                )}
              </div>

              {/* Collapsible Preview Panel (Desktop - below content) */}
              <div className={cn(radius.lg, "border border-fg/10 bg-fg/5 hidden lg:block")}>
                {/* Collapsed Header / Toggle */}
                <button
                  onClick={() => setPreviewExpanded(!previewExpanded)}
                  className={cn(
                    "w-full flex items-center justify-between px-4 py-3",
                    "text-left",
                    interactive.transition.fast,
                    "hover:bg-fg/5",
                    previewExpanded ? "border-b border-fg/10" : "",
                  )}
                >
                  <div className="flex items-center gap-2">
                    <Eye className="h-4 w-4 text-fg/50" />
                    <span className="text-sm font-medium text-fg">Preview</span>
                    {!previewExpanded && preview && (
                      <span className="text-xs text-fg/40 ml-2">(has generated preview)</span>
                    )}
                  </div>
                  <div className="flex items-center gap-2">
                    {previewExpanded ? (
                      <ChevronUp className="h-4 w-4 text-fg/50" />
                    ) : (
                      <ChevronDown className="h-4 w-4 text-fg/50" />
                    )}
                  </div>
                </button>

                {/* Expanded Content */}
                <AnimatePresence>
                  {previewExpanded && (
                    <motion.div
                      initial={{ height: 0, opacity: 0 }}
                      animate={{ height: "auto", opacity: 1 }}
                      exit={{ height: 0, opacity: 0 }}
                      transition={{ duration: 0.2 }}
                      className="overflow-hidden"
                    >
                      <div className="p-4">
                        <PreviewPanel />
                      </div>
                    </motion.div>
                  )}
                </AnimatePresence>
              </div>

            </div>

            {/* Desktop Sidebar - Quick Insert */}
            <motion.div
              ref={sidebarRef}
              style={{ y: quickInsertY }}
              className="hidden lg:block w-80 shrink-0 space-y-4 self-start relative z-20"
            >
              <div className={cn(radius.lg, "border border-fg/10 bg-fg/5 p-4")}>
                <h3 className="text-sm font-medium text-fg mb-1">Quick Insert</h3>
                <p className="text-xs text-fg/40 mb-3">Click to insert at cursor</p>

                <div className="space-y-1.5 max-h-[60vh] overflow-y-auto">
                  {variables.map((v) => {
                    const isRequired = requiredVariables.includes(v.var);
                    const isMissing = missingVariables.includes(v.var);
                    return (
                      <button
                        key={v.var}
                        onClick={() => insertVariable(v.var)}
                        className={cn(
                          "w-full text-left p-2",
                          radius.md,
                          "border",
                          isMissing
                            ? "border-danger/30 bg-danger/10"
                            : isRequired
                              ? "border-warning/30 bg-warning/10"
                              : "border-fg/10 bg-fg/5",
                          interactive.transition.fast,
                          "hover:bg-fg/10",
                        )}
                      >
                        <div className="flex items-center gap-2">
                          {isRequired && (
                            <span
                              className={cn(
                                "text-xs",
                                isMissing ? "text-danger/80" : "text-warning/80",
                              )}
                            >
                              ★
                            </span>
                          )}
                          <code
                            className={cn(
                              "text-xs font-medium",
                              isMissing ? "text-danger/80" : "text-accent/80",
                            )}
                          >
                            {v.var}
                          </code>
                        </div>
                        <p className="text-[10px] text-fg/40 mt-0.5">{v.desc}</p>
                      </button>
                    );
                  })}
                </div>

                <div className="flex items-start gap-2 mt-3 pt-3 border-t border-fg/10">
                  <span className="text-fg/30 text-xs mt-0.5">ⓘ</span>
                  <p className="text-xs text-fg/40 leading-relaxed">
                    Variables are replaced with actual values when the prompt is used.
                  </p>
                </div>
              </div>
            </motion.div>
          </div>
        </div>
      </main>

      {/* Variables Bottom Sheet (Mobile) */}
      <BottomMenu
        isOpen={showVariables}
        onClose={() => setShowVariables(false)}
        title="Template Variables"
      >
        <div className="space-y-4">
          <p className="text-xs text-fg/50">Tap to insert a variable into your prompt</p>

          {isAppDefault && requiredVariables.length > 0 && (
            <div className={cn(radius.lg, "border border-warning/30 bg-warning/10 p-3")}>
              <p className="text-xs text-warning/80">
                <span className="font-semibold">Required:</span> Variables marked with ★ must be
                included
              </p>
            </div>
          )}

          <div className="max-h-[50vh] overflow-y-auto space-y-2">
            {variables.map((item) => {
              const isRequired = requiredVariables.includes(item.var);
              const isMissing = missingVariables.includes(item.var);
              return (
                <div
                  key={item.var}
                  className={cn(
                    radius.lg,
                    "border p-3",
                    isMissing
                      ? "border-danger/40 bg-danger/10"
                      : isRequired
                        ? "border-warning/30 bg-warning/10"
                        : "border-fg/10 bg-fg/5",
                  )}
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 mb-1">
                        {isRequired && (
                          <span className={isMissing ? "text-danger/80" : "text-warning/80"}>★</span>
                        )}
                        <code
                          className={cn(
                            "text-sm font-semibold",
                            isMissing ? "text-danger/80" : "text-accent/80",
                          )}
                        >
                          {item.var}
                        </code>
                        {copiedVar === item.var && (
                          <span className="flex items-center gap-1 text-xs text-accent/80">
                            <Check className="h-3 w-3" />
                            Copied
                          </span>
                        )}
                      </div>
                      <p className="text-sm text-fg/80">{item.label}</p>
                      <p className="text-xs text-fg/50">{item.desc}</p>
                    </div>

                    <div className="flex items-center gap-2 shrink-0">
                      <button
                        onClick={() => copyVariable(item.var)}
                        className={cn(
                          "flex items-center justify-center h-8 w-8",
                          radius.md,
                          "border border-fg/10 bg-fg/5",
                          "text-fg/50",
                          interactive.transition.fast,
                          "hover:bg-fg/10 hover:text-fg",
                        )}
                        title="Copy"
                      >
                        <Copy className="h-4 w-4" />
                      </button>
                      <button
                        onClick={() => {
                          insertVariable(item.var);
                          setShowVariables(false);
                        }}
                        className={cn(
                          "flex items-center gap-1.5 px-3 py-1.5",
                          radius.md,
                          "border border-accent/30 bg-accent/15",
                          "text-xs font-medium text-accent/80",
                          interactive.transition.fast,
                          "hover:bg-accent/25",
                        )}
                      >
                        Insert
                      </button>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </BottomMenu>

      {/* Preview Bottom Sheet (Mobile only) */}
      <BottomMenu
        isOpen={showMobilePreview}
        onClose={() => setShowMobilePreview(false)}
        title="Preview"
      >
        {usesEntryEditor && (
          <div className="flex items-center gap-1 p-1 rounded-lg border border-fg/10 bg-fg/5 mb-3">
            <button
              onClick={() => setMobilePreviewTab("content")}
              className={cn(
                "flex-1 flex items-center justify-center px-3 py-1.5",
                radius.md,
                "text-xs font-medium transition",
                mobilePreviewTab === "content"
                  ? "bg-accent/20 text-accent/80"
                  : "text-fg/50 hover:text-fg/70",
              )}
            >
              Content
            </button>
            <button
              onClick={() => setMobilePreviewTab("structure")}
              className={cn(
                "flex-1 flex items-center justify-center px-3 py-1.5",
                radius.md,
                "text-xs font-medium transition",
                mobilePreviewTab === "structure"
                  ? "bg-accent/20 text-accent/80"
                  : "text-fg/50 hover:text-fg/70",
              )}
            >
              Structure
            </button>
          </div>
        )}
        {mobilePreviewTab === "content" || !usesEntryEditor ? (
          <PreviewPanel isMobile />
        ) : (
          <MessageStructurePreview
            entries={entries}
            condensePromptEntries={condensePromptEntries}
            onEditEntry={handleStructureEdit}
            onDeleteEntry={handleStructureDelete}
            onReorderEntry={handleStructureReorder}
          />
        )}
      </BottomMenu>

      {/* Entry Editor Bottom Sheet (Mobile only) */}
      <BottomMenu
        isOpen={!!mobileEntryEditorId}
        onClose={() => setMobileEntryEditorId(null)}
        title="Edit Entry"
      >
        {selectedMobileEntry ? (
          <div className="space-y-3">
            <div className="grid gap-2">
              <div className="space-y-1">
                <input
                  value={selectedMobileEntry.name}
                  onChange={(event) =>
                    handleEntryUpdate(selectedMobileEntry.id, { name: event.target.value })
                  }
                  className="w-full rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-sm text-fg"
                  placeholder="Entry name"
                />
                <p className="text-[11px] text-fg/50">Name used for organization and preview.</p>
              </div>

              <div className="space-y-1">
                <select
                  value={selectedMobileEntry.role}
                  onChange={(event) =>
                    handleEntryUpdate(selectedMobileEntry.id, {
                      role: event.target.value as any,
                    })
                  }
                  className="h-9 w-full rounded-lg border border-fg/10 bg-fg/5 px-2.5 text-xs text-fg"
                >
                  {ENTRY_ROLE_OPTIONS.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
                <p className="text-[11px] text-fg/50">
                  Select which role the model receives for this entry.
                </p>
              </div>

              <div className="space-y-1">
                <select
                  value={selectedMobileEntry.injectionPosition}
                  onChange={(event) => {
                    const nextPosition = event.target
                      .value as SystemPromptEntry["injectionPosition"];
                    handleEntryUpdate(selectedMobileEntry.id, {
                      injectionPosition: nextPosition,
                      conditionalMinMessages:
                        nextPosition === "conditional"
                          ? (selectedMobileEntry.conditionalMinMessages ??
                            DEFAULT_CONDITIONAL_MIN_MESSAGES)
                          : (selectedMobileEntry.conditionalMinMessages ?? null),
                      intervalTurns:
                        nextPosition === "interval"
                          ? (selectedMobileEntry.intervalTurns ?? DEFAULT_INTERVAL_TURNS)
                          : (selectedMobileEntry.intervalTurns ?? null),
                    });
                  }}
                  className="h-9 w-full rounded-lg border border-fg/10 bg-fg/5 px-2.5 text-xs text-fg"
                >
                  {ENTRY_POSITION_OPTIONS.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
                <p className="text-[11px] text-fg/50">
                  {getInjectionModeHint(selectedMobileEntry.injectionPosition)}
                </p>
              </div>

              {selectedMobileEntry.injectionPosition !== "relative" && (
                <div className="space-y-1">
                  <p className="text-[11px] text-fg/50">Insertion Depth</p>
                  <input
                    type="number"
                    min={0}
                    value={selectedMobileEntry.injectionDepth}
                    onChange={(event) =>
                      handleEntryUpdate(selectedMobileEntry.id, {
                        injectionDepth: Number(event.target.value),
                      })
                    }
                    className="h-9 w-full rounded-lg border border-fg/10 bg-fg/5 px-2.5 text-xs text-fg"
                    placeholder="0 = newest context"
                  />
                  <p className="text-[11px] text-fg/50">
                    Depth 0 is newest; higher numbers insert earlier.
                  </p>
                </div>
              )}

              {selectedMobileEntry.injectionPosition === "conditional" && (
                <div className="space-y-1">
                  <p className="text-[11px] text-fg/50">Min Messages</p>
                  <input
                    type="number"
                    min={1}
                    value={
                      selectedMobileEntry.conditionalMinMessages ?? DEFAULT_CONDITIONAL_MIN_MESSAGES
                    }
                    onChange={(event) =>
                      handleEntryUpdate(selectedMobileEntry.id, {
                        conditionalMinMessages: Math.max(1, Number(event.target.value) || 1),
                      })
                    }
                    className="h-9 w-full rounded-lg border border-fg/10 bg-fg/5 px-2.5 text-xs text-fg"
                    placeholder="Inject after at least N messages"
                  />
                  <p className="text-[11px] text-fg/50">
                    Inject only when at least this many chat messages are in context.
                  </p>
                </div>
              )}

              {selectedMobileEntry.injectionPosition === "interval" && (
                <div className="space-y-1">
                  <p className="text-[11px] text-fg/50">Every N Messages</p>
                  <input
                    type="number"
                    min={1}
                    value={selectedMobileEntry.intervalTurns ?? DEFAULT_INTERVAL_TURNS}
                    onChange={(event) =>
                      handleEntryUpdate(selectedMobileEntry.id, {
                        intervalTurns: Math.max(1, Number(event.target.value) || 1),
                      })
                    }
                    className="h-9 w-full rounded-lg border border-fg/10 bg-fg/5 px-2.5 text-xs text-fg"
                    placeholder="Inject every N messages"
                  />
                  <p className="text-[11px] text-fg/50">Inject every N context turns.</p>
                </div>
              )}
            </div>

            <textarea
              ref={(el) => {
                entryTextareaRefs.current[selectedMobileEntry.id] = el;
              }}
              value={selectedMobileEntry.content}
              onChange={(event) =>
                handleEntryUpdate(selectedMobileEntry.id, { content: event.target.value })
              }
              onFocus={() => {
                activeEntryIdRef.current = selectedMobileEntry.id;
              }}
              rows={10}
              className="w-full resize-none rounded-xl border border-fg/10 bg-surface-el/30 px-3.5 py-2.5 font-mono text-sm leading-relaxed text-fg placeholder-fg/30"
              placeholder="Write the prompt entry..."
            />
          </div>
        ) : (
          <p className="text-sm text-fg/60">Select an entry to edit.</p>
        )}
      </BottomMenu>
    </div>
  );
}
