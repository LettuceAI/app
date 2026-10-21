import { useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCanGoBack, useNavigate, useParams, useRouteContext, useRouter } from "@tanstack/react-router";
import { GitBranch, MessageSquare, MoreHorizontal, Pencil, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge, confirm, ConnectedTree, icon, IconButton, ListGroup, ListRowButton, LoadingState, Page, Sheet, toast } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import type { ConnectedTreeNode } from "@lettuceai/crisp";
import type { ConversationBranchList, ConversationBranchView } from "@/api/generated/bindings";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { FailureState } from "@/shared/ui/FailureState";
import { deleteBranch, renameBranch, selectBranch } from "../data/chat-actions";
import { allChatsQuery, branchesQuery } from "../data/queries";
import { RenameRow } from "../history/ChatHistoryList";
import { buildTree, type TreeItem } from "./tree-model";

export function BranchesPage() {
  const { conversationId } = useParams({ strict: false });
  return conversationId ? <BranchesScreen key={conversationId} conversationId={conversationId} /> : null;
}

function BranchesScreen({ conversationId }: { conversationId: string }) {
  const { t, i18n } = useTranslation("conversations");
  const { t: common } = useTranslation("common");
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const router = useRouter();
  const canGoBack = useCanGoBack();
  const branches = useQuery(branchesQuery(api, conversationId));
  const chats = useQuery(allChatsQuery(api));
  const [busy, setBusy] = useState(false);
  const running = useRef(false);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [target, setTarget] = useState<ConversationBranchView | null>(null);
  const { shown } = useClosingValue(target);

  const run = async <T,>(task: () => Promise<T>): Promise<T | undefined> => {
    if (running.current) return;
    running.current = true;
    setBusy(true);
    try {
      return await task();
    } catch (error) {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      running.current = false;
      setBusy(false);
    }
  };
  const back = () => {
    if (canGoBack) router.history.back();
    else void navigate({ to: "/chat/$conversationId", params: { conversationId } });
  };
  const nameOf = (branch: ConversationBranchView) => branch.parent_branch_id === null ? t("branches.original") : branch.label || t("branches.untitled");
  const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" });

  const failed = branches.isError ? branches : chats.isError ? chats : null;
  return (
    <Page title={t("branches.title")} backLabel={t("header.back")} onBack={back}>
      {failed ? (
        <FailureState error={failed.error} onRetry={() => void failed.refetch()} />
      ) : !branches.isSuccess || !chats.isSuccess ? (
        <LoadingState />
      ) : (
        <ConnectedTree aria-label={t("branches.title")} nodes={buildTree(chats.data, conversationId, branches.data).map((item) => toNode(item, branches.data))} />
      )}
      <Sheet open={target !== null} onClose={() => setTarget(null)} size="sm" title={shown ? t("branches.actions", { title: nameOf(shown) }) : undefined} dismissible={!busy}>
        {shown && branches.data && (
          <ListGroup>
            <ListRowButton
              icon={<Pencil size={icon.lg} />}
              title={t("branches.rename")}
              chevron={false}
              disabled={busy}
              onClick={() => {
                setTarget(null);
                setRenaming(shown.id);
              }}
            />
            <ListRowButton
              icon={<Trash2 size={icon.lg} />}
              tone="danger"
              title={<span className="text-danger">{t("branches.delete")}</span>}
              {...(shown.parent_branch_id === null ? { description: t("branches.deleteRoot") } : shown.active ? { description: t("branches.deleteActive") } : {})}
              chevron={false}
              disabled={busy || shown.parent_branch_id === null || shown.active}
              onClick={async () => {
                setTarget(null);
                const sure = await confirm({
                  title: t("branches.deleteTitle"),
                  message: t("branches.deleteBody", { name: nameOf(shown) }),
                  confirmLabel: t("branches.delete"),
                  cancelLabel: t("confirm.cancel"),
                  destructive: true,
                });
                if (!sure) return;
                const done = await run(() => deleteBranch(api, queryClient, conversationId, shown.id, branches.data.revision).then(() => true));
                if (done) toast.success(t("branches.deleted"));
              }}
            />
          </ListGroup>
        )}
      </Sheet>
    </Page>
  );

  function toNode(item: TreeItem, list: ConversationBranchList): ConnectedTreeNode {
    const children = item.children.map((child) => toNode(child, list));
    const meta = (count: number, updatedAt: number) => t("branches.meta", { count, date: date.format(updatedAt) });
    if (item.kind === "chat") {
      return {
        id: item.id,
        title: item.chat.title || t("branches.untitledChat"),
        description: meta(item.chat.message_count, item.chat.updated_at),
        icon: <MessageSquare size={icon.md} />,
        ...(item.current ? { badge: <Badge tone="accent">{t("branches.thisChat")}</Badge> } : {}),
        current: item.current,
        onSelect: () => (item.current ? back() : void navigate({ to: "/chat/$conversationId", params: { conversationId: item.chat.id } })),
        children,
      };
    }
    const { branch } = item;
    if (renaming === branch.id) {
      return {
        id: item.id,
        title: nameOf(branch),
        content: (
          <div className="rounded-xl border border-line bg-surface-1">
            <RenameRow
              initial={branch.label}
              busy={busy}
              label={t("branches.renameField")}
              saveLabel={t("branches.renameSave")}
              onCancel={() => setRenaming(null)}
              onSave={(label) => void run(() => renameBranch(api, queryClient, conversationId, branch.id, list.revision, label).then(() => true)).then((ok) => ok && setRenaming(null))}
            />
          </div>
        ),
        children,
      };
    }
    return {
      id: item.id,
      title: nameOf(branch),
      description: meta(branch.message_count, branch.updated_at),
      icon: <GitBranch size={icon.md} />,
      ...(branch.active ? { badge: <Badge tone="accent">{t("branches.current")}</Badge> } : {}),
      current: branch.active,
      disabled: busy,
      onSelect: () => {
        if (branch.active) return back();
        void run(() => selectBranch(api, queryClient, conversationId, branch.id, list.revision).then(() => true)).then((ok) => ok && back());
      },
      trailing: (
        <IconButton label={t("branches.actions", { title: nameOf(branch) })} className="mr-2 shrink-0" disabled={busy} onClick={() => setTarget(branch)}>
          <MoreHorizontal size={icon.md} />
        </IconButton>
      ),
      children,
    };
  }
}
