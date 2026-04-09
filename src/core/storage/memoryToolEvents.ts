import type { GroupSession, Session } from "./schemas";

type DirectMemory = NonNullable<Session["memoryEmbeddings"]>[number];
type GroupMemory = NonNullable<GroupSession["memoryEmbeddings"]>[number];
type AnyMemory = DirectMemory | GroupMemory;
type AnyMemoryToolEvent =
  | NonNullable<Session["memoryToolEvents"]>[number]
  | NonNullable<GroupSession["memoryToolEvents"]>[number];

function cloneMemory<T extends AnyMemory>(memory: T): T {
  return {
    ...memory,
    embedding: [...memory.embedding],
  };
}

function toMemorySnapshot(value: unknown): AnyMemory | null {
  if (!value || typeof value !== "object") return null;
  const snapshot = value as Record<string, unknown>;
  if (typeof snapshot.id !== "string" || typeof snapshot.text !== "string") return null;

  return {
    id: snapshot.id,
    text: snapshot.text,
    embedding: Array.isArray(snapshot.embedding)
      ? snapshot.embedding.filter((item): item is number => typeof item === "number")
      : [],
    createdAt: typeof snapshot.createdAt === "number" ? snapshot.createdAt : 0,
    tokenCount: typeof snapshot.tokenCount === "number" ? snapshot.tokenCount : 0,
    isCold: Boolean(snapshot.isCold),
    importanceScore: typeof snapshot.importanceScore === "number" ? snapshot.importanceScore : 1,
    lastAccessedAt: typeof snapshot.lastAccessedAt === "number" ? snapshot.lastAccessedAt : 0,
    isPinned: Boolean(snapshot.isPinned),
    category: typeof snapshot.category === "string" ? snapshot.category : null,
  };
}

function resolveMemoryId(action: Record<string, unknown>): string | null {
  const args = (action.arguments as Record<string, unknown> | undefined) ?? {};
  const argId = typeof args.id === "string" ? args.id : null;
  const actionId = typeof action.memoryId === "string" ? action.memoryId : null;
  const deletedId = typeof action.deletedMemoryId === "string" ? action.deletedMemoryId : null;
  const argText = typeof args.text === "string" ? args.text : null;

  return argId ?? actionId ?? deletedId ?? argText;
}

function findMemoryIndex<T extends AnyMemory>(memories: T[], action: Record<string, unknown>): number {
  const memoryId = resolveMemoryId(action);
  if (memoryId) {
    const byId = memories.findIndex((memory) => memory.id === memoryId);
    if (byId >= 0) return byId;
  }

  const args = (action.arguments as Record<string, unknown> | undefined) ?? {};
  const text =
    (typeof action.deletedText === "string" ? action.deletedText : null) ??
    (typeof args.text === "string" ? args.text : null);

  return text ? memories.findIndex((memory) => memory.text === text) : -1;
}

export function revertMemoryToolEvent<T extends AnyMemory>(
  memories: T[],
  event: AnyMemoryToolEvent,
): T[] {
  const next = memories.map((memory) => cloneMemory(memory as T));
  const actions = [...(event.actions ?? [])]
    .filter((action) => action.name !== "done")
    .reverse() as Array<Record<string, unknown> & { name: string }>;

  for (const action of actions) {
    if (action.name === "create_memory") {
      const idx = findMemoryIndex(next, action);
      if (idx >= 0) {
        next.splice(idx, 1);
      }
      continue;
    }

    if (action.name === "delete_memory") {
      const snapshot = toMemorySnapshot(action.memorySnapshot);
      if (!snapshot) continue;

      const idx = findMemoryIndex(next, action);
      if (idx >= 0) {
        next[idx] = cloneMemory(snapshot as T);
      } else {
        next.push(cloneMemory(snapshot as T));
      }
      continue;
    }

    if (action.name === "pin_memory" || action.name === "unpin_memory") {
      const idx = findMemoryIndex(next, action);
      if (idx >= 0) {
        next[idx] = {
          ...next[idx],
          isPinned: action.name === "unpin_memory",
        };
      }
    }
  }

  return next;
}

export function markMemoryToolEventReverted<
  T extends { id?: string; revertedAt?: number } & Record<string, unknown>,
>(events: T[], eventId: string, revertedAt: number): T[] {
  return events.map((event) =>
    event.id === eventId
      ? {
          ...event,
          revertedAt,
        }
      : event,
  );
}
