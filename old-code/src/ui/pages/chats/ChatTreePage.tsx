import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import {
  ArrowLeft,
  Check,
  Download,
  Edit3,
  GitBranch,
  GitCompare,
  MoreVertical,
  Trash2,
  X,
} from "lucide-react";
import { useLocation, useParams } from "react-router-dom";

import type { Character, Session, SessionPreview } from "../../../core/storage";
import {
  createBranchedSession,
  deleteSession,
  getSession,
  listBranchTree,
  listCharacters,
  updateSessionTitle,
} from "../../../core/storage";
import { storageBridge } from "../../../core/storage/files";
import { typography, radius, cn, colors, interactive } from "../../design-tokens";
import { BottomMenu } from "../../components";
import { Routes, useNavigationManager } from "../../navigation";
import { useI18n } from "../../../core/i18n/context";

interface TreeNode extends SessionPreview {
  children: TreeNode[];
  depth: number;
}

interface TreeRow {
  node: TreeNode;
  depth: number;
  rails: boolean[];
  railPath: boolean[];
  elbowHorizPath: boolean;
  elbowDownPath: boolean;
  isLast: boolean;
}

function buildTree(nodes: SessionPreview[]): TreeNode[] {
  const byId = new Map<string, TreeNode>();
  nodes.forEach((node) => byId.set(node.id, { ...node, children: [], depth: 0 }));

  const roots: TreeNode[] = [];
  byId.forEach((node) => {
    const parent = node.parentSessionId ? byId.get(node.parentSessionId) : undefined;
    if (parent) parent.children.push(node);
    else roots.push(node);
  });

  const order = (a: TreeNode, b: TreeNode) => a.updatedAt - b.updatedAt;
  const sortKids = (node: TreeNode) => {
    node.children.sort(order);
    node.children.forEach(sortKids);
  };
  roots.sort(order);
  roots.forEach(sortKids);
  return roots;
}

function flattenRows(roots: TreeNode[], pathIds: Set<string>): TreeRow[] {
  const out: TreeRow[] = [];
  const walk = (
    node: TreeNode,
    depth: number,
    parentRails: boolean[],
    parentRailPath: boolean[],
    isLast: boolean,
    elbow: { vert: boolean; horiz: boolean; down: boolean },
  ) => {
    const rails = depth === 0 ? [] : [...parentRails, !isLast];
    const railPath = depth === 0 ? [] : [...parentRailPath, elbow.vert];
    out.push({
      node,
      depth,
      rails,
      railPath,
      elbowHorizPath: elbow.horiz,
      elbowDownPath: elbow.down,
      isLast,
    });

    const childParentRailPath = depth === 0 ? [] : [...parentRailPath, elbow.down];
    const onPathChildIdx = node.children.findIndex((child) => pathIds.has(child.id));

    node.children.forEach((child, i) => {
      walk(child, depth + 1, rails, childParentRailPath, i === node.children.length - 1, {
        vert: onPathChildIdx !== -1 && i <= onPathChildIdx,
        horiz: pathIds.has(child.id),
        down: onPathChildIdx !== -1 && i < onPathChildIdx,
      });
    });
  };
  roots.forEach((root, i) =>
    walk(root, 0, [], [], i === roots.length - 1, { vert: false, horiz: false, down: false }),
  );
  return out;
}

function commonPrefixLength(a: Session["messages"], b: Session["messages"]): number {
  let i = 0;
  while (
    i < a.length &&
    i < b.length &&
    a[i].role === b[i].role &&
    a[i].content === b[i].content
  ) {
    i += 1;
  }
  return i;
}

export function ChatTreePage() {
  const { characterId } = useParams<{ characterId: string }>();
  const location = useLocation();
  const { go, backOrReplace } = useNavigationManager();
  const { t } = useI18n();

  const sessionId = useMemo(
    () => new URLSearchParams(location.search).get("sessionId"),
    [location.search],
  );

  const [nodes, setNodes] = useState<SessionPreview[]>([]);
  const [characterNames, setCharacterNames] = useState<Map<string, string>>(new Map());
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busyIds, setBusyIds] = useState<Set<string>>(new Set());

  const [actionTarget, setActionTarget] = useState<SessionPreview | null>(null);
  const [renameTarget, setRenameTarget] = useState<SessionPreview | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [deleteTarget, setDeleteTarget] = useState<SessionPreview | null>(null);

  const [compareMode, setCompareMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [comparison, setComparison] = useState<{
    a: SessionPreview;
    b: SessionPreview;
    shared: number;
    tailA: Session["messages"];
    tailB: Session["messages"];
  } | null>(null);
  const [comparing, setComparing] = useState(false);

  useEffect(() => {
    const load = async () => {
      if (!sessionId) {
        setIsLoading(false);
        return;
      }
      try {
        setIsLoading(true);
        setError(null);
        const [tree, characters] = await Promise.all([listBranchTree(sessionId), listCharacters()]);
        setNodes(tree);
        setCharacterNames(new Map(characters.map((c: Character) => [c.id, c.name])));
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setIsLoading(false);
      }
    };
    void load();
  }, [sessionId]);

  useEffect(() => {
    setRenameDraft(renameTarget?.title ?? "");
  }, [renameTarget]);

  const pathIds = useMemo(() => {
    const byId = new Map(nodes.map((n) => [n.id, n]));
    const ids = new Set<string>();
    let cursor = sessionId ? byId.get(sessionId) : undefined;
    while (cursor) {
      ids.add(cursor.id);
      cursor = cursor.parentSessionId ? byId.get(cursor.parentSessionId) : undefined;
    }
    return ids;
  }, [nodes, sessionId]);

  const rows = useMemo(() => flattenRows(buildTree(nodes), pathIds), [nodes, pathIds]);

  const summary = useMemo(() => {
    const branches = nodes.length;
    const messages = nodes.reduce((sum, n) => sum + n.messageCount, 0);
    const depth = rows.reduce((max, r) => Math.max(max, r.depth), 0);
    return { branches, messages, depth };
  }, [nodes, rows]);

  const withBusy = useCallback(async (id: string, fn: () => Promise<void>) => {
    setBusyIds((prev) => new Set(prev).add(id));
    try {
      await fn();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyIds((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    }
  }, []);

  const handleRename = useCallback(
    async (id: string, title: string) => {
      await withBusy(id, async () => {
        await updateSessionTitle(id, title);
        setNodes((prev) => prev.map((n) => (n.id === id ? { ...n, title } : n)));
      });
    },
    [withBusy],
  );

  const handleDelete = useCallback(
    async (id: string) => {
      await withBusy(id, async () => {
        await deleteSession(id);
        setNodes((prev) => prev.filter((n) => n.id !== id));
      });
    },
    [withBusy],
  );

  const handleExport = useCallback(
    async (id: string) => {
      try {
        const path = await storageBridge.jsonlExportSingleChat(id);
        alert(t("chats.history.chatPackageExportedTo", { path }));
      } catch (err) {
        alert(typeof err === "string" ? err : String(err));
      }
    },
    [t],
  );

  const handleBranchFromHere = useCallback(
    async (node: SessionPreview) => {
      await withBusy(node.id, async () => {
        const full = await getSession(node.id);
        if (!full || full.messages.length === 0) return;
        const lastMessage = full.messages[full.messages.length - 1];
        const branched = await createBranchedSession(full, lastMessage.id);
        go(Routes.chatSession(branched.characterId, branched.id));
      });
    },
    [withBusy, go],
  );

  const toggleSelection = useCallback((id: string) => {
    setSelectedIds((prev) => {
      if (prev.includes(id)) return prev.filter((x) => x !== id);
      if (prev.length >= 2) return [prev[1], id];
      return [...prev, id];
    });
  }, []);

  useEffect(() => {
    if (selectedIds.length !== 2) return;
    const a = nodes.find((n) => n.id === selectedIds[0]);
    const b = nodes.find((n) => n.id === selectedIds[1]);
    if (!a || !b) return;
    let cancelled = false;
    void (async () => {
      setComparing(true);
      try {
        const [sa, sb] = await Promise.all([getSession(a.id), getSession(b.id)]);
        if (cancelled || !sa || !sb) return;
        const shared = commonPrefixLength(sa.messages, sb.messages);
        setComparison({
          a,
          b,
          shared,
          tailA: sa.messages.slice(shared),
          tailB: sb.messages.slice(shared),
        });
      } finally {
        if (!cancelled) setComparing(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [selectedIds, nodes]);

  const exitCompare = useCallback(() => {
    setCompareMode(false);
    setSelectedIds([]);
    setComparison(null);
  }, []);

  const handleBack = useCallback(() => {
    backOrReplace(characterId ? Routes.chatSession(characterId, sessionId) : Routes.chat);
  }, [backOrReplace, characterId, sessionId]);

  const onNodeSelect = useCallback(
    (node: SessionPreview) => {
      if (compareMode) toggleSelection(node.id);
      else go(Routes.chatSession(node.characterId, node.id));
    },
    [compareMode, toggleSelection, go],
  );

  return (
    <div className={cn("flex h-full flex-col", colors.surface.base, colors.text.primary)}>
      <header
        className={cn("z-20 shrink-0 border-b border-fg/10 bg-surface pl-4 pb-3 pr-4")}
        style={{ paddingTop: "calc(env(safe-area-inset-top) + 12px)" }}
      >
        <div className="flex items-center justify-between gap-3">
          <div className="flex min-w-0 items-center">
            <button
              onClick={compareMode ? exitCompare : handleBack}
              className={cn(
                "flex shrink-0 items-center justify-center -ml-2 px-[0.6em] py-[0.3em]",
                colors.text.primary,
                interactive.transition.fast,
                "hover:text-fg/80",
              )}
              aria-label={t("chats.header.back")}
            >
              {compareMode ? <X size={18} strokeWidth={2.5} /> : <ArrowLeft size={18} strokeWidth={2.5} />}
            </button>
            <div className="min-w-0 text-left">
              <p className="truncate text-xl font-bold text-fg/90">{t("chats.branchTree.title")}</p>
              <p className="mt-0.5 truncate text-xs text-fg/50">
                {compareMode ? t("chats.branchTree.compareSelect") : t("chats.branchTree.subtitle")}
              </p>
            </div>
          </div>
          {!isLoading && rows.length > 1 ? (
            <button
              onClick={() => (compareMode ? exitCompare() : setCompareMode(true))}
              className={cn(
                "flex shrink-0 items-center gap-1.5 border px-3 py-1.5",
                radius.lg,
                typography.caption.size,
                interactive.transition.fast,
                compareMode
                  ? "border-accent/40 bg-accent/15 text-accent/90"
                  : "border-fg/10 bg-fg/4 text-fg/60 hover:bg-fg/8 hover:text-fg/85",
              )}
            >
              <GitCompare size={14} />
              {t("chats.branchTree.compare")}
            </button>
          ) : null}
        </div>
      </header>

      <main className="flex-1 overflow-y-auto px-3 pt-4">
        {comparing || comparison ? (
          <CompareView
            comparing={comparing}
            comparison={comparison}
            onClose={() => {
              setComparison(null);
              setSelectedIds([]);
            }}
          />
        ) : (
        <div className="mx-auto w-full max-w-3xl pb-24">
          {isLoading ? (
            <div className="flex justify-center py-20">
              <div className="h-8 w-8 animate-spin rounded-full border border-fg/10 border-t-white/60" />
            </div>
          ) : error ? (
            <div className={cn("border border-danger/30 bg-danger/10 p-4 text-center", radius.lg)}>
              <p className={cn(typography.bodySmall.size, "text-danger")}>{error}</p>
            </div>
          ) : rows.length <= 1 ? (
            <div className="py-20 text-center">
              <GitBranch className="mx-auto mb-4 h-12 w-12 text-fg/30" />
              <p className={cn(typography.bodySmall.size, "text-fg/45")}>
                {t("chats.branchTree.empty")}
              </p>
            </div>
          ) : (
            <>
              <div
                className={cn(
                  "mb-4 flex flex-wrap items-center gap-x-2 gap-y-1 border bg-fg/3 px-4 py-2.5",
                  colors.border.subtle,
                  radius.lg,
                  typography.caption.size,
                  "text-fg/50",
                )}
              >
                <span className="font-medium text-fg/75">
                  {t("chats.branchTree.statBranches", { count: summary.branches })}
                </span>
                <span className="text-fg/25">·</span>
                <span>{t("chats.branchTree.statMessages", { count: summary.messages })}</span>
                <span className="text-fg/25">·</span>
                <span>{t("chats.branchTree.statDepth", { count: summary.depth })}</span>
              </div>

              <div>
                {rows.map((row) => (
                  <BranchRow
                    key={row.node.id}
                    row={row}
                    isCurrent={row.node.id === sessionId}
                    onPath={pathIds.has(row.node.id)}
                    dimmed={pathIds.size > 0 && !pathIds.has(row.node.id) && !compareMode}
                    compareMode={compareMode}
                    selectedIndex={selectedIds.indexOf(row.node.id)}
                    isBusy={busyIds.has(row.node.id)}
                    crossCharacterName={
                      row.node.characterId !== characterId
                        ? characterNames.get(row.node.characterId)
                        : undefined
                    }
                    onSelect={() => onNodeSelect(row.node)}
                    onMenu={() => setActionTarget(row.node)}
                  />
                ))}
              </div>
            </>
          )}
        </div>
        )}
      </main>

      <BottomMenu
        isOpen={actionTarget != null}
        onClose={() => setActionTarget(null)}
        title={t("chats.branchTree.actionsTitle")}
      >
        <div className="space-y-1.5 text-fg">
          <SheetButton
            icon={<GitBranch size={16} />}
            label={t("chats.branchTree.openChat")}
            onClick={() => {
              if (!actionTarget) return;
              const node = actionTarget;
              setActionTarget(null);
              go(Routes.chatSession(node.characterId, node.id));
            }}
          />
          <SheetButton
            icon={<Edit3 size={16} />}
            label={t("common.buttons.rename")}
            onClick={() => {
              setRenameTarget(actionTarget);
              setActionTarget(null);
            }}
          />
          <SheetButton
            icon={<GitCompare size={16} />}
            label={t("chats.branchTree.branchFromHere")}
            onClick={() => {
              if (!actionTarget) return;
              const node = actionTarget;
              setActionTarget(null);
              void handleBranchFromHere(node);
            }}
          />
          <SheetButton
            icon={<Download size={16} />}
            label={t("common.buttons.export")}
            onClick={() => {
              if (!actionTarget) return;
              const node = actionTarget;
              setActionTarget(null);
              void handleExport(node.id);
            }}
          />
          <SheetButton
            icon={<Trash2 size={16} />}
            label={t("common.buttons.delete")}
            danger
            onClick={() => {
              setDeleteTarget(actionTarget);
              setActionTarget(null);
            }}
          />
        </div>
      </BottomMenu>

      <BottomMenu
        isOpen={renameTarget != null}
        onClose={() => setRenameTarget(null)}
        title={t("common.buttons.rename")}
      >
        <div className="space-y-4 text-fg">
          <input
            type="text"
            value={renameDraft}
            onChange={(e) => setRenameDraft(e.target.value)}
            className={cn(
              "w-full border bg-fg/5 px-3 py-2.5 text-fg",
              colors.border.subtle,
              radius.lg,
              typography.bodySmall.size,
              "focus:outline-none focus:ring-2 focus:ring-white/10 focus:border-fg/20",
            )}
            placeholder={t("chats.chatTitlePlaceholder")}
            autoFocus
          />
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => setRenameTarget(null)}
              className={cn(
                "flex-1 border border-fg/10 bg-fg/5 px-4 py-2.5 text-fg/65",
                radius.lg,
                typography.bodySmall.size,
                "font-medium transition-all hover:bg-fg/8 hover:text-fg/80",
                interactive.active.scale,
              )}
            >
              {t("common.buttons.cancel")}
            </button>
            <button
              type="button"
              disabled={
                !renameTarget ||
                !renameDraft.trim() ||
                renameDraft.trim() === renameTarget.title
              }
              onClick={() => {
                if (!renameTarget) return;
                const id = renameTarget.id;
                const title = renameDraft.trim();
                setRenameTarget(null);
                void handleRename(id, title);
              }}
              className={cn(
                "flex-1 border border-accent/30 bg-accent/15 px-4 py-2.5 text-accent/90",
                radius.lg,
                typography.bodySmall.size,
                "font-medium transition-all hover:bg-accent/25",
                interactive.active.scale,
                "disabled:pointer-events-none disabled:opacity-40",
              )}
            >
              {t("common.buttons.save")}
            </button>
          </div>
        </div>
      </BottomMenu>

      <BottomMenu
        isOpen={deleteTarget != null}
        onClose={() => setDeleteTarget(null)}
        title={t("chats.deleteChat")}
        includeExitIcon={false}
      >
        <div className="rounded-xl border border-danger/20 bg-danger/8 p-3">
          <p className={cn(typography.bodySmall.size, "truncate font-semibold text-fg/90")}>
            {deleteTarget?.title || t("chats.untitledChat")}
          </p>
          <p className={cn(typography.bodySmall.size, "mt-3 text-fg/55")}>
            {t("chats.branchTree.deleteConfirm")}
          </p>
        </div>
        <div className="flex gap-2 pt-4">
          <button
            type="button"
            onClick={() => setDeleteTarget(null)}
            className={cn(
              "flex-1 border border-fg/10 bg-fg/5 px-4 py-2.5 text-fg/65",
              radius.lg,
              typography.bodySmall.size,
              "font-medium transition-all hover:bg-fg/8 hover:text-fg/80",
              interactive.active.scale,
            )}
          >
            {t("common.buttons.cancel")}
          </button>
          <button
            type="button"
            onClick={() => {
              if (!deleteTarget) return;
              const id = deleteTarget.id;
              setDeleteTarget(null);
              void handleDelete(id);
            }}
            className={cn(
              "flex-1 border border-danger/30 bg-danger/15 px-4 py-2.5 text-danger",
              radius.lg,
              typography.bodySmall.size,
              "font-medium transition-all hover:bg-danger/25",
              interactive.active.scale,
            )}
          >
            {t("common.buttons.delete")}
          </button>
        </div>
      </BottomMenu>

    </div>
  );
}

function CompareView({
  comparing,
  comparison,
  onClose,
}: {
  comparing: boolean;
  comparison: {
    a: SessionPreview;
    b: SessionPreview;
    shared: number;
    tailA: Session["messages"];
    tailB: Session["messages"];
  } | null;
  onClose: () => void;
}) {
  const { t } = useI18n();

  return (
    <div className="mx-auto w-full max-w-5xl pb-24">
      <div className="mb-4 flex items-center justify-between gap-3">
        <button
          onClick={onClose}
          className={cn(
            "flex items-center gap-1.5 border px-3 py-1.5",
            radius.lg,
            typography.caption.size,
            colors.border.subtle,
            "bg-fg/4 text-fg/65 transition-colors hover:bg-fg/8 hover:text-fg/90",
          )}
        >
          <ArrowLeft size={14} />
          {t("chats.branchTree.title")}
        </button>
        {comparison ? (
          <span
            className={cn(
              "inline-flex items-center gap-1.5 rounded-full border border-accent/25 bg-accent/10 px-3 py-1",
              typography.caption.size,
              "text-accent/80",
            )}
          >
            <GitCompare size={13} />
            {t("chats.branchTree.compareShared", { count: comparison.shared })}
          </span>
        ) : null}
      </div>

      {comparing || !comparison ? (
        <div className="flex justify-center py-20">
          <div className="h-8 w-8 animate-spin rounded-full border border-fg/10 border-t-white/60" />
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
          <CompareColumn title={comparison.a.title} messages={comparison.tailA} />
          <CompareColumn title={comparison.b.title} messages={comparison.tailB} />
        </div>
      )}
    </div>
  );
}

const RAIL_W = 26;

function BranchRow({
  row,
  isCurrent,
  onPath,
  dimmed,
  compareMode,
  selectedIndex,
  isBusy,
  crossCharacterName,
  onSelect,
  onMenu,
}: {
  row: TreeRow;
  isCurrent: boolean;
  onPath: boolean;
  dimmed: boolean;
  compareMode: boolean;
  selectedIndex: number;
  isBusy: boolean;
  crossCharacterName?: string;
  onSelect: () => void;
  onMenu: () => void;
}) {
  const { t } = useI18n();
  const { node, depth, rails, railPath, elbowHorizPath, elbowDownPath } = row;
  const vLine = (on: boolean) =>
    cn("absolute left-1/2 -translate-x-1/2", on ? "w-0.5 bg-accent/80" : "w-px bg-fg/12");
  const hLine = (on: boolean) =>
    cn("absolute -translate-y-1/2", on ? "h-0.5 bg-accent/80" : "h-px bg-fg/12");
  const isSelected = selectedIndex >= 0;

  return (
    <div className="flex items-stretch">
      {rails.map((active, k) => {
        const isElbow = k === depth - 1;
        return (
          <div key={k} className="relative shrink-0 self-stretch" style={{ width: RAIL_W }}>
            {isElbow ? (
              <>
                <span className={cn("top-0 h-1/2", vLine(railPath[k]))} />
                <span className={cn("left-1/2 right-0 top-1/2", hLine(elbowHorizPath))} />
                {active ? (
                  <span className={cn("bottom-0 top-1/2", vLine(elbowDownPath))} />
                ) : null}
                <span
                  className={cn(
                    "absolute left-1/2 top-1/2 h-2 w-2 -translate-x-1/2 -translate-y-1/2 rounded-full border",
                    isCurrent
                      ? "border-accent/60 bg-accent"
                      : onPath
                        ? "border-accent/50 bg-accent/40"
                        : "border-fg/30 bg-surface",
                  )}
                />
              </>
            ) : active ? (
              <span className={cn("top-0 bottom-0", vLine(railPath[k]))} />
            ) : null}
          </div>
        );
      })}

      <div className={cn("flex min-w-0 flex-1 items-center gap-1 py-1.5", dimmed && "opacity-45")}>
        <button
          onClick={onSelect}
          disabled={isBusy}
          className={cn(
            "group block min-w-0 flex-1 overflow-hidden border px-4 py-2.5 text-left",
            radius.lg,
            interactive.transition.fast,
            isBusy && "opacity-50",
            isSelected
              ? "border-accent/50 bg-accent/12"
              : isCurrent
                ? "border-accent/40 bg-accent/10"
                : onPath
                  ? "border-fg/14 bg-fg/5 hover:border-fg/20"
                  : "border-fg/8 bg-fg/3 hover:border-fg/12 hover:bg-fg/4",
          )}
        >
          <div className="flex items-center gap-2">
            {compareMode ? (
              <span
                className={cn(
                  "flex h-4 w-4 shrink-0 items-center justify-center rounded border",
                  isSelected ? "border-accent bg-accent text-surface" : "border-fg/30",
                )}
              >
                {isSelected ? <Check size={11} strokeWidth={3} /> : null}
              </span>
            ) : depth === 0 ? (
              <GitBranch size={13} className="shrink-0 text-fg/45" />
            ) : null}
            <h3 className={cn(typography.bodySmall.size, "truncate font-semibold text-fg/92")}>
              {node.title?.trim() ? node.title : t("chats.untitledChat")}
            </h3>
            {isCurrent ? (
              <span className="inline-flex shrink-0 items-center rounded-md border border-accent/30 bg-accent/15 px-1.5 py-0.5 text-[10px] font-medium text-accent/90">
                {t("chats.branchTree.current")}
              </span>
            ) : null}
          </div>
          <div className="mt-1 flex items-center gap-2">
            <span className={cn(typography.caption.size, "text-fg/40")}>
              {t("chats.branchTree.messagesCount", { count: node.messageCount.toLocaleString() })}
            </span>
            {crossCharacterName ? (
              <span className={cn(typography.caption.size, "truncate text-accent/70")}>
                {t("chats.branchTree.otherCharacter", { name: crossCharacterName })}
              </span>
            ) : null}
          </div>
        </button>
        {!compareMode ? (
          <button
            onClick={onMenu}
            disabled={isBusy}
            aria-label={t("chats.branchTree.actionsTitle")}
            className={cn(
              "flex shrink-0 items-center justify-center rounded-lg p-2 text-fg/40",
              interactive.transition.fast,
              "hover:bg-fg/6 hover:text-fg/75",
            )}
          >
            <MoreVertical size={16} />
          </button>
        ) : null}
      </div>
    </div>
  );
}

function CompareColumn({ title, messages }: { title: string; messages: Session["messages"] }) {
  const { t } = useI18n();
  return (
    <div className={cn("min-w-0 overflow-hidden border bg-fg/2", colors.border.subtle, radius.lg)}>
      <div className="sticky top-0 z-10 border-b border-fg/8 bg-surface/95 px-4 py-2.5 backdrop-blur">
        <p className={cn(typography.bodySmall.size, "truncate font-semibold text-fg/85")}>
          {title?.trim() ? title : t("chats.untitledChat")}
        </p>
        <p className={cn(typography.caption.size, "mt-0.5 text-fg/40")}>
          {t("chats.branchTree.messagesCount", { count: messages.length.toLocaleString() })}
        </p>
      </div>
      <div className="space-y-2 p-3">
        {messages.length === 0 ? (
          <p className={cn(typography.caption.size, "px-1 py-6 text-center text-fg/30")}>—</p>
        ) : (
          messages.slice(0, 60).map((m, i) => {
            const isAssistant = m.role === "assistant";
            return (
              <div
                key={i}
                className={cn(
                  "border px-3 py-2",
                  radius.md,
                  isAssistant
                    ? "border-accent/15 bg-accent/5"
                    : cn("bg-fg/4", colors.border.subtle),
                )}
              >
                <span
                  className={cn(
                    "mb-1 inline-block rounded text-[10px] font-medium uppercase tracking-wide",
                    isAssistant ? "text-accent/70" : "text-fg/40",
                  )}
                >
                  {m.role}
                </span>
                <p className={cn(typography.caption.size, "whitespace-pre-wrap text-fg/70")}>
                  {m.content?.trim() || "—"}
                </p>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}

function SheetButton({
  icon,
  label,
  danger,
  onClick,
}: {
  icon: ReactNode;
  label: string;
  danger?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "flex w-full items-center gap-3 border px-4 py-3 text-left",
        radius.lg,
        typography.bodySmall.size,
        interactive.transition.fast,
        interactive.active.scale,
        danger
          ? "border-danger/20 bg-danger/8 text-danger hover:bg-danger/15"
          : "border-fg/8 bg-fg/3 text-fg/80 hover:bg-fg/6 hover:text-fg",
      )}
    >
      <span className="shrink-0">{icon}</span>
      <span className="font-medium">{label}</span>
    </button>
  );
}

export default ChatTreePage;
