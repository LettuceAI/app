import { useState, useMemo } from "react";
import {
  ChevronDown,
  ChevronUp,
  MessageSquare,
  MoreHorizontal,
  Pencil,
  GripVertical,
  Trash2,
} from "lucide-react";
import { cn, radius, interactive } from "../../../design-tokens";
import { BottomMenu } from "../../../components";
import { confirmBottomMenu } from "../../../components/ConfirmBottomMenu";
import type { SystemPromptEntry } from "../../../../core/storage/schemas";

// ---------- Types ----------

interface PreviewMessage {
  id: string;
  role: "system" | "user" | "assistant";
  label: string;
  content: string;
  isMock: boolean;
  entryId?: string;
  injectionInfo?: string;
}

// ---------- Assembly algorithm (mirrors Rust backend) ----------

/** Mirrors `partition_prompt_entries` from commands.rs:110-124 */
function partitionEntries(
  entries: SystemPromptEntry[],
): [SystemPromptEntry[], SystemPromptEntry[]] {
  const relative: SystemPromptEntry[] = [];
  const inChat: SystemPromptEntry[] = [];
  for (const entry of entries) {
    if (!entry.enabled && !entry.systemPrompt) continue;
    if (entry.content.trim() === "") continue;
    switch (entry.injectionPosition) {
      case "relative":
        relative.push(entry);
        break;
      case "inChat":
      case "conditional":
      case "interval":
        inChat.push(entry);
        break;
    }
  }
  return [relative, inChat];
}

/** Mirrors `should_insert_in_chat_prompt_entry` from commands.rs:126-139 */
function shouldInsert(entry: SystemPromptEntry, turnCount: number): boolean {
  switch (entry.injectionPosition) {
    case "inChat":
      return true;
    case "conditional": {
      const min = entry.conditionalMinMessages ?? 1;
      return turnCount >= min;
    }
    case "interval": {
      const interval = entry.intervalTurns ?? 0;
      return interval > 0 && turnCount > 0 && turnCount % interval === 0;
    }
    default:
      return false;
  }
}

/** Mirrors `insert_in_chat_prompt_entries` from commands.rs:141-173 */
function insertInChatEntries(
  messages: PreviewMessage[],
  entries: SystemPromptEntry[],
): PreviewMessage[] {
  if (entries.length === 0) return messages;

  const result = [...messages];
  const baseLen = result.length;
  const turnCount = baseLen;

  const inserts: { pos: number; idx: number; entry: SystemPromptEntry }[] = [];
  entries.forEach((entry, idx) => {
    if (!shouldInsert(entry, turnCount)) return;
    if (entry.content.trim() === "") return;
    const pos = Math.max(0, baseLen - entry.injectionDepth);
    inserts.push({ pos, idx, entry });
  });

  inserts.sort((a, b) => a.pos - b.pos || a.idx - b.idx);

  let offset = 0;
  for (const { pos, entry } of inserts) {
    const insertAt = Math.min(pos + offset, result.length);
    result.splice(insertAt, 0, {
      id: `injected-${entry.id}`,
      role: entry.role,
      label: entry.name,
      content: entry.content,
      isMock: false,
      entryId: entry.id,
      injectionInfo: formatInjectionInfo(entry),
    });
    offset += 1;
  }

  return result;
}

/** Mirrors `condense_entries_into_single_system_message` from prompt_engine.rs:1161-1193 */
function condenseEntries(entries: SystemPromptEntry[]): SystemPromptEntry[] {
  const merged = entries
    .filter((e) => e.enabled || e.systemPrompt)
    .map((e) => e.content.trim())
    .filter(Boolean)
    .join("\n\n");

  if (!merged.trim()) return [];

  return [
    {
      id: "entry_condensed_system",
      name: "Condensed System Prompt",
      role: "system",
      content: merged,
      enabled: true,
      injectionPosition: "relative",
      injectionDepth: 0,
      conditionalMinMessages: null,
      intervalTurns: null,
      systemPrompt: true,
    } satisfies SystemPromptEntry,
  ];
}

function formatInjectionInfo(entry: SystemPromptEntry): string {
  switch (entry.injectionPosition) {
    case "relative":
      return "relative";
    case "inChat":
      return `inChat · depth ${entry.injectionDepth}`;
    case "conditional":
      return `conditional · min ${entry.conditionalMinMessages ?? 1}`;
    case "interval":
      return `interval · every ${entry.intervalTurns ?? 0}`;
    default:
      return "";
  }
}

function generateMockMessages(pairCount: number): PreviewMessage[] {
  const messages: PreviewMessage[] = [];
  for (let i = 1; i <= pairCount; i++) {
    messages.push({
      id: `mock-user-${i}`,
      role: "user",
      label: `User message #${i}`,
      content: `This is a mock user message #${i}`,
      isMock: true,
    });
    messages.push({
      id: `mock-assistant-${i}`,
      role: "assistant",
      label: `Assistant message #${i}`,
      content: `This is a mock assistant message #${i}`,
      isMock: true,
    });
  }
  return messages;
}

/** Main assembly: builds the full message list the LLM would see. */
function assembleStructure(
  entries: SystemPromptEntry[],
  mockPairCount: number,
  condense: boolean,
): PreviewMessage[] {
  const working = condense
    ? condenseEntries(entries)
    : entries.filter((e) => e.enabled || e.systemPrompt);

  const [relativeEntries, inChatEntries] = partitionEntries(working);

  const result: PreviewMessage[] = [];

  for (const entry of relativeEntries) {
    if (entry.content.trim() === "") continue;
    result.push({
      id: `relative-${entry.id}`,
      role: entry.role,
      label: entry.name,
      content: entry.content,
      isMock: false,
      entryId: entry.id,
      injectionInfo: "relative",
    });
  }

  const mockMessages = generateMockMessages(mockPairCount);
  const chatWithInjections = insertInChatEntries(mockMessages, inChatEntries);
  result.push(...chatWithInjections);

  return result;
}

// ---------- Role config ----------

const ROLE_CONFIG = {
  system: { accent: "bg-info", text: "text-info/80", badge: "bg-info/15 text-info/80" },
  user: { accent: "bg-accent", text: "text-accent/80", badge: "bg-accent/15 text-accent/80" },
  assistant: { accent: "bg-secondary", text: "text-secondary/80", badge: "bg-secondary/15 text-secondary/80" },
} as const;

// ---------- Sub-components ----------

function PromptEntryMessage({
  message,
  index,
  onMenuOpen,
}: {
  message: PreviewMessage;
  index: number;
  onMenuOpen: (entryId: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const role = ROLE_CONFIG[message.role] ?? ROLE_CONFIG.system;
  const isLong = message.content.length > 160;

  return (
    <div className="flex gap-0 overflow-hidden rounded-lg border border-fg/10 bg-fg/[0.03]">
      {/* Accent bar */}
      <div className={cn("w-1 shrink-0", role.accent)} />

      <div className="flex-1 min-w-0 px-3.5 py-2.5">
        {/* Header */}
        <div className="flex items-center gap-2">
          <span className="text-[10px] font-mono text-fg/20 w-4 shrink-0">{index + 1}</span>
          <span
            className={cn(
              "px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wider rounded",
              role.badge,
            )}
          >
            {message.role}
          </span>
          <span className="text-[13px] font-medium text-fg/80 truncate">{message.label}</span>
          {message.injectionInfo && (
            <span className="ml-auto text-[10px] text-fg/25 font-mono shrink-0">
              {message.injectionInfo}
            </span>
          )}
        </div>

        {/* Content + actions row */}
        <div className="mt-1.5 ml-6 flex gap-2">
          <div className="flex-1 min-w-0">
            <pre
              className={cn(
                "whitespace-pre-wrap text-xs leading-relaxed font-mono text-fg/55",
                !expanded && isLong && "line-clamp-2",
              )}
            >
              {message.content}
            </pre>
            {isLong && (
              <button
                onClick={() => setExpanded(!expanded)}
                className={cn(
                  "flex items-center gap-1 mt-1 text-[10px]",
                  role.text,
                  "opacity-50",
                  interactive.transition.fast,
                  "hover:opacity-80",
                )}
              >
                {expanded ? (
                  <>
                    <ChevronUp className="h-3 w-3" /> Show less
                  </>
                ) : (
                  <>
                    <ChevronDown className="h-3 w-3" /> Show more
                  </>
                )}
              </button>
            )}
          </div>

          {/* Three-dot menu trigger */}
          {message.entryId && (
            <button
              onClick={() => onMenuOpen(message.entryId!)}
              className={cn(
                "self-end p-1 rounded",
                "text-fg/20",
                interactive.transition.fast,
                "hover:text-fg/50 hover:bg-fg/5",
              )}
            >
              <MoreHorizontal className="h-4 w-4" />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

function MockMessage({ message, index }: { message: PreviewMessage; index: number }) {
  const role = ROLE_CONFIG[message.role] ?? ROLE_CONFIG.system;

  return (
    <div className="flex items-center gap-2.5 px-3.5 py-2.5 rounded-md">
      <span className="text-[10px] font-mono text-fg/20 w-4 shrink-0">{index + 1}</span>
      <div className={cn("w-1.5 h-1.5 rounded-full opacity-50", role.accent)} />
      <span className="text-xs text-fg/30 italic">{message.label}</span>
    </div>
  );
}

// ---------- Main component ----------

interface MessageStructurePreviewProps {
  entries: SystemPromptEntry[];
  condensePromptEntries: boolean;
  onEditEntry?: (entryId: string) => void;
  onDeleteEntry?: (entryId: string) => void;
  onReorderEntry?: (entryId: string) => void;
}

export function MessageStructurePreview({
  entries,
  condensePromptEntries,
  onEditEntry,
  onDeleteEntry,
  onReorderEntry,
}: MessageStructurePreviewProps) {
  const [mockPairCount, setMockPairCount] = useState(3);
  const [menuOpenId, setMenuOpenId] = useState<string | null>(null);

  const messages = useMemo(
    () => assembleStructure(entries, mockPairCount, condensePromptEntries),
    [entries, mockPairCount, condensePromptEntries],
  );

  const menuEntry = menuOpenId ? entries.find((e) => e.id === menuOpenId) : null;

  const promptCount = messages.filter((m) => !m.isMock).length;
  const mockCount = messages.filter((m) => m.isMock).length;

  const handleDelete = async () => {
    if (!menuOpenId) return;
    const entryId = menuOpenId;
    setMenuOpenId(null);
    const confirmed = await confirmBottomMenu({
      title: "Delete entry?",
      message: `Remove "${menuEntry?.name ?? "this entry"}" from the prompt template? This cannot be undone.`,
      confirmLabel: "Delete",
      destructive: true,
    });
    if (confirmed) onDeleteEntry?.(entryId);
  };

  return (
    <div className="space-y-3">
      {/* Header */}
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2 text-fg/40">
          <MessageSquare className="h-3.5 w-3.5" />
          <span className="text-xs">What the LLM sees</span>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <label htmlFor="mock-pair-count" className="text-[11px] text-fg/30">
            Turns
          </label>
          <input
            id="mock-pair-count"
            type="number"
            min={0}
            max={50}
            value={mockPairCount}
            onChange={(e) => {
              const v = parseInt(e.target.value, 10);
              if (!isNaN(v) && v >= 0 && v <= 50) setMockPairCount(v);
            }}
            className={cn(
              "w-12 h-6 text-center text-[11px] font-mono",
              radius.md,
              "border border-fg/10 bg-surface-el/30 text-fg",
              "focus:border-fg/20 focus:outline-none",
            )}
          />
        </div>
      </div>

      {/* Message list */}
      <div className="space-y-1">
        {messages.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-10 text-center">
            <MessageSquare className="h-6 w-6 text-fg/15 mb-2" />
            <p className="text-xs text-fg/30">No messages</p>
            <p className="text-[11px] text-fg/20 mt-0.5">Add entries or increase turns</p>
          </div>
        ) : (
          messages.map((msg, i) =>
            msg.isMock ? (
              <MockMessage key={msg.id} message={msg} index={i} />
            ) : (
              <PromptEntryMessage
                key={msg.id}
                message={msg}
                index={i}
                onMenuOpen={setMenuOpenId}
              />
            ),
          )
        )}
      </div>

      {/* Summary */}
      <div className="flex items-center gap-4 pt-2 border-t border-fg/5 text-[11px] text-fg/25">
        <span>
          <span className="font-mono text-fg/40">{messages.length}</span> messages
        </span>
        <span>
          <span className="font-mono text-info/40">{promptCount}</span> injected
        </span>
        <span>
          <span className="font-mono text-fg/30">{mockCount}</span> mock
        </span>
      </div>

      {/* Entry action menu */}
      <BottomMenu
        isOpen={!!menuOpenId}
        onClose={() => setMenuOpenId(null)}
        title={menuEntry?.name ?? "Entry"}
      >
        <div className="space-y-1 py-1">
          <button
            onClick={() => {
              const id = menuOpenId;
              setMenuOpenId(null);
              if (id) onEditEntry?.(id);
            }}
            className={cn(
              "w-full flex items-center gap-3 px-4 py-3 text-left",
              radius.md,
              interactive.transition.fast,
              "hover:bg-fg/5",
            )}
          >
            <Pencil className="h-4 w-4 text-fg/50" />
            <span className="text-sm text-fg">Edit entry</span>
          </button>

          <button
            onClick={() => {
              const id = menuOpenId;
              setMenuOpenId(null);
              if (id) onReorderEntry?.(id);
            }}
            className={cn(
              "w-full flex items-center gap-3 px-4 py-3 text-left",
              radius.md,
              interactive.transition.fast,
              "hover:bg-fg/5",
            )}
          >
            <GripVertical className="h-4 w-4 text-fg/50" />
            <span className="text-sm text-fg">Reorder</span>
          </button>

          <button
            onClick={handleDelete}
            className={cn(
              "w-full flex items-center gap-3 px-4 py-3 text-left",
              radius.md,
              interactive.transition.fast,
              "hover:bg-danger/10",
            )}
          >
            <Trash2 className="h-4 w-4 text-danger" />
            <span className="text-sm text-danger">Delete</span>
          </button>
        </div>
      </BottomMenu>
    </div>
  );
}
