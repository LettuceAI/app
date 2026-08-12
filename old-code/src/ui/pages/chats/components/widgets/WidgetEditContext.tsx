import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import type { WidgetNode } from "../../../../../core/storage/schemas";

export type WidgetSide = "left" | "right";

export interface WidgetSlots {
  left: WidgetNode[];
  right: WidgetNode[];
}

interface WidgetEditContextValue {
  editing: boolean;
  saving: boolean;
  enterEdit: () => void;
  done: () => void;
  revert: () => void;
  getNodes: (side: WidgetSide) => WidgetNode[];
  setNodes: (side: WidgetSide, nodes: WidgetNode[]) => void;
  addNode: (side: WidgetSide, node: WidgetNode) => void;
  updateNode: (side: WidgetSide, node: WidgetNode) => void;
  removeNode: (side: WidgetSide, id: string) => void;
  moveToOtherSlot: (fromSide: WidgetSide, id: string) => void;
  chooseLibraryImage: (node: WidgetNode) => void;
  pendingOpenNodeId: string | null;
  clearPendingOpen: () => void;
}

const WidgetEditCtx = createContext<WidgetEditContextValue | null>(null);

export interface WidgetEditRestore {
  slots: WidgetSlots;
  openNodeId: string;
}

interface WidgetEditProviderProps {
  slots: WidgetSlots;
  onPersist: (slots: WidgetSlots) => Promise<void> | void;
  onChooseLibraryImage?: (nodeId: string) => void;
  restore?: WidgetEditRestore | null;
  onRestoreConsumed?: () => void;
  editRequestNonce?: number;
  children: ReactNode;
}

export function WidgetEditProvider({
  slots,
  onPersist,
  onChooseLibraryImage,
  restore,
  onRestoreConsumed,
  editRequestNonce,
  children,
}: WidgetEditProviderProps) {
  const [editing, setEditing] = useState(false);
  const [saving, setSaving] = useState(false);
  const [draft, setDraft] = useState<WidgetSlots>(slots);
  const [pendingOpenNodeId, setPendingOpenNodeId] = useState<string | null>(null);
  const draftRef = useRef(draft);
  draftRef.current = draft;
  const lastEditNonceRef = useRef(editRequestNonce ?? 0);

  useEffect(() => {
    if (!editing) setDraft(slots);
  }, [slots, editing]);

  useEffect(() => {
    if (!restore) return;
    setDraft(restore.slots);
    setEditing(true);
    setPendingOpenNodeId(restore.openNodeId);
    onRestoreConsumed?.();
  }, [restore, onRestoreConsumed]);

  const clearPendingOpen = useCallback(() => setPendingOpenNodeId(null), []);

  const enterEdit = useCallback(() => {
    setDraft(slots);
    setEditing(true);
  }, [slots]);

  useEffect(() => {
    if (editRequestNonce === undefined) return;
    if (editRequestNonce === lastEditNonceRef.current) return;
    lastEditNonceRef.current = editRequestNonce;
    if (editRequestNonce > 0) enterEdit();
  }, [editRequestNonce, enterEdit]);

  const revert = useCallback(() => {
    setDraft(slots);
    setEditing(false);
  }, [slots]);

  const done = useCallback(async () => {
    setSaving(true);
    try {
      await onPersist(draft);
      setEditing(false);
    } finally {
      setSaving(false);
    }
  }, [draft, onPersist]);

  const getNodes = useCallback(
    (side: WidgetSide) => (editing ? draft[side] : slots[side]),
    [editing, draft, slots],
  );

  const setNodes = useCallback((side: WidgetSide, nodes: WidgetNode[]) => {
    setDraft((prev) => ({ ...prev, [side]: nodes }));
  }, []);

  const addNode = useCallback((side: WidgetSide, node: WidgetNode) => {
    setDraft((prev) => ({ ...prev, [side]: [...prev[side], node] }));
  }, []);

  const updateNode = useCallback((side: WidgetSide, node: WidgetNode) => {
    setDraft((prev) => ({
      ...prev,
      [side]: prev[side].map((n) => (n.id === node.id ? node : n)),
    }));
  }, []);

  const removeNode = useCallback((side: WidgetSide, id: string) => {
    setDraft((prev) => ({
      ...prev,
      [side]: prev[side].filter((n) => n.id !== id),
    }));
  }, []);

  const moveToOtherSlot = useCallback((fromSide: WidgetSide, id: string) => {
    setDraft((prev) => {
      const node = prev[fromSide].find((n) => n.id === id);
      if (!node) return prev;
      const otherSide: WidgetSide = fromSide === "left" ? "right" : "left";
      return {
        ...prev,
        [fromSide]: prev[fromSide].filter((n) => n.id !== id),
        [otherSide]: [...prev[otherSide], node],
      };
    });
  }, []);

  const chooseLibraryImage = useCallback(
    (node: WidgetNode) => {
      const merge = (nodes: WidgetNode[]): WidgetNode[] =>
        nodes.map((n) =>
          n.id === node.id
            ? node
            : n.type === "box"
              ? { ...n, children: merge(n.children) }
              : n,
        );
      const next: WidgetSlots = {
        left: merge(draftRef.current.left),
        right: merge(draftRef.current.right),
      };
      setDraft(next);
      setEditing(false);
      void onPersist(next);
      onChooseLibraryImage?.(node.id);
    },
    [onPersist, onChooseLibraryImage],
  );

  const value = useMemo<WidgetEditContextValue>(
    () => ({
      editing,
      saving,
      enterEdit,
      done: () => void done(),
      revert,
      getNodes,
      setNodes,
      addNode,
      updateNode,
      removeNode,
      moveToOtherSlot,
      chooseLibraryImage,
      pendingOpenNodeId,
      clearPendingOpen,
    }),
    [
      editing,
      saving,
      pendingOpenNodeId,
      clearPendingOpen,
      moveToOtherSlot,
      enterEdit,
      done,
      revert,
      getNodes,
      setNodes,
      addNode,
      updateNode,
      removeNode,
      chooseLibraryImage,
    ],
  );

  return <WidgetEditCtx.Provider value={value}>{children}</WidgetEditCtx.Provider>;
}

export function useWidgetEdit(): WidgetEditContextValue {
  const ctx = useContext(WidgetEditCtx);
  if (!ctx) {
    throw new Error("useWidgetEdit used outside WidgetEditProvider");
  }
  return ctx;
}
