import { useCallback, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useLocation, useNavigate, useParams, useRouter, useSearch } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { History } from "lucide-react";
import { Alert, AppBar, Button, icon, IconButton, LoadingState, Resizable, Tab, TabList, Tabs } from "@lettuceai/crisp";
import type { AssetRef, CreatorDraft, CreatorProposalView, CreatorView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { useAvatarCrop } from "@/entities/image-input";
import { ModelChoiceSheet } from "@/entities/model";
import { refreshLibrary } from "@/features/library";
import { startDirectChat } from "@/features/conversations";
import { FailureState } from "@/shared/ui/FailureState";
import { usePageLayout } from "@/shared/hooks/usePageLayout";
import { type AttachPanel, type CreatorAttachments, CreatorChat } from "./CreatorChat";
import { HistorySheet, ReviewSheet } from "./CreatorSheets";
import { DraftEditor } from "./DraftEditor";
import { useCreator } from "./data";

type Panel = "history" | "review" | "model" | AttachPanel;

export function CreatorPage() {
  const { workflowId } = useParams({ from: "/creator/$workflowId" });
  return <CreatorWorkspace key={workflowId} />;
}

function CreatorWorkspace() {
  const { workflowId } = useParams({ from: "/creator/$workflowId" });
  const search = useSearch({ from: "/creator/$workflowId" });
  const navigate = useNavigate();
  const router = useRouter();
  const panelPushed = useLocation({ select: (location) => location.state.creatorPanelPushed });
  const client = useQueryClient();
  const { t } = useTranslation("creator");
  const { phone } = usePageLayout();
  const { api, query, busy, error, perform, operation } = useCreator(workflowId);
  const work = query.data;
  const [picked, setModel] = useState<string | null | undefined>(undefined);
  const model = picked === undefined ? (work?.model_profile_id ?? null) : picked;
  const [dirty, setDirty] = useState(false);
  const pending = useRef<CreatorDraft | null>(null);
  const holdPending = useCallback((draft: CreatorDraft | null) => {
    pending.current = draft;
  }, []);
  const [selected, setSelected] = useState<CreatorProposalView | null>(null);
  const [chatFailure, setChatFailure] = useState<unknown>(null);
  const { refetch } = query;
  const settled = useCallback(() => refetch({ throwOnError: true }), [refetch]);
  const settings = useQuery({ queryKey: queryKeys.settings.detail(), queryFn: () => api.call("settingsGet") });
  const effectiveModel = model ?? settings.data?.global.creation_helper.model_profile_id ?? settings.data?.default_model_profile_id ?? null;

  const open = (panel: Panel) => void navigate({ to: "/creator/$workflowId", params: { workflowId }, search: { ...search, panel }, state: { creatorPanelPushed: true } });
  const close = () => {
    setSelected(null);
    if (panelPushed) router.history.back();
    else void navigate({ to: "/creator/$workflowId", params: { workflowId }, search: search.tab ? { tab: search.tab } : {}, replace: true });
  };
  const switchPanel = (panel: Panel) => void navigate({ to: "/creator/$workflowId", params: { workflowId }, search: { ...search, panel }, state: { creatorPanelPushed: panelPushed ?? false }, replace: true });
  const showTab = (tab: "chat" | "draft") => void navigate({ to: "/creator/$workflowId", params: { workflowId }, search: { ...search, tab }, replace: true });

  const revision = (value: CreatorView) => ({ workflow_id: workflowId, expected_revision: value.revision, proposal_id: value.proposal.id });
  const saveDraft = async (draft: CreatorDraft) =>
    (await perform((value) => {
      const payload = { workflow_id: workflowId, expected_revision: value.revision, draft };
      return api.call("creatorEdit", { ...payload, client_operation_id: operation("edit", payload) });
    })) !== null;
  const flush = async () => (pending.current ? saveDraft(pending.current) : true);
  const send = async (message: string, attachments: CreatorAttachments) =>
    (await flush()) &&
    (await perform((value) => {
      const payload = {
        workflow_id: workflowId,
        expected_revision: value.revision,
        message,
        references: attachments.references.map(({ kind, id }) => ({ kind, id })),
        images: attachments.images.map((image) => image.asset_id),
        model_profile_id: model,
      };
      return api.call("creatorSend", { ...payload, client_operation_id: operation("send", payload) });
    })) !== null;
  const regenerate = (turnId: string) =>
    void perform((value) => {
      const payload = { workflow_id: workflowId, expected_revision: value.revision, turn_id: turnId, model_profile_id: model };
      return api.call("creatorRegenerate", { ...payload, client_operation_id: operation("regenerate", payload) });
    });
  const crop = useAvatarCrop(api);
  const applyAvatar = async (asset: AssetRef) => {
    const baked = await crop.choose(asset);
    if (!baked || !(await flush())) return;
    await perform((value) => {
      const payload = { workflow_id: workflowId, expected_revision: value.revision, draft: { ...value.proposal.draft, avatar: baked } };
      return api.call("creatorEdit", { ...payload, client_operation_id: operation("avatar", payload) });
    });
  };
  const launchChat = async (id: string) => {
    try {
      const conversationId = await startDirectChat(api, client, id);
      await navigate({ to: "/chat/$conversationId", params: { conversationId }, replace: true });
    } catch (failure) {
      setChatFailure(failure);
    }
  };
  const carryOver = async () => {
    if (!(await flush())) return;
    const next = await perform((value) => {
      const payload = { kind: value.summary.kind, target_id: value.summary.target_id, draft: value.proposal.draft };
      return api.call("creatorStart", { ...payload, client_operation_id: operation("carry", payload) });
    });
    if (next) await navigate({ to: "/creator/$workflowId", params: { workflowId: next.summary.id }, replace: true });
  };
  const apply = async (chat: boolean) => {
    const saved = await perform(async (value) => {
      const reviewPayload = revision(value);
      const reviewed = value.reviewed ? value : await api.call("creatorReview", { ...reviewPayload, client_operation_id: operation("review", reviewPayload) });
      const payload = revision(reviewed);
      return api.call("creatorApply", { ...payload, client_operation_id: operation("apply", payload) });
    });
    if (!saved) return;
    await refreshLibrary(client);
    close();
    if (chat && saved.summary.applied_id) await launchChat(saved.summary.applied_id);
  };

  if (query.isPending) return <LoadingState />;
  if (query.isError || !work) return <FailureState error={query.error} onRetry={() => void query.refetch()} />;

  const applied = work.summary.applied_id !== null;
  const active = work.active_job_id !== null;
  const stale = work.target_changed && !applied;
  const locked = busy || active || applied;
  const complete = Boolean(work.proposal.draft.name.trim() && work.proposal.draft.definition.trim());
  const canReview = !locked && !dirty && complete;
  const review = async () => {
    if (await flush()) open("review");
  };
  const baseline = work.history[0]?.draft ?? work.proposal.draft;
  const tab = search.tab ?? "chat";
  const openSaved = () =>
    void navigate(
      work.summary.kind === "character"
        ? { to: "/characters/$characterId/edit", params: { characterId: work.summary.applied_id! } }
        : { to: "/personas/$personaId/edit", params: { personaId: work.summary.applied_id! } },
    );

  const chat = (
    <CreatorChat
      api={api}
      work={work}
      phone={phone}
      busy={busy || applied}
      hasModel={effectiveModel !== null}
      onModel={() => open("model")}
      onChanges={(proposalId) => {
        setSelected(work.history.find((proposal) => proposal.id === proposalId) ?? null);
        open("history");
      }}
      send={send}
      panel={search.panel === "attach" || search.panel === "character" || search.panel === "persona" ? search.panel : null}
      onUseAvatar={(asset) => void applyAvatar(asset)}
      onPanel={(panel) => (panel === null ? close() : search.panel ? switchPanel(panel) : open(panel))}
      regenerate={regenerate}
      stop={() => { if (work.active_job_id) void perform(() => api.call("jobCancel", { job_id: work.active_job_id! })); }}
      settled={settled}
    />
  );
  const draft = <DraftEditor key={work.proposal.id} api={api} images={work.images} initial={work.proposal.draft} kind={work.summary.kind} disabled={locked} phone={phone} save={saveDraft} onDirty={setDirty} onPending={holdPending} />;

  return (
    <div className="@container flex h-full min-h-0 flex-col overflow-clip">
      <AppBar
        title={work.summary.name || t("untitled")}
        meta={phone ? t(applied ? "saved" : "notSaved") : `${t(`kind.${work.summary.kind}`)} · ${t(applied ? "saved" : "notSaved")}`}
        onBack={() => void navigate({ to: "/creator" })}
        actions={
          <div className="flex items-center gap-1">
            <IconButton label={t("history")} variant="ghost" onClick={() => open("history")}>
              <History size={icon.lg} />
            </IconButton>
            {!applied && (
              <Button size="sm" variant="primary" aria-label={t("review")} disabled={locked || stale} onClick={() => void review()}>
                {t(phone ? "reviewShort" : "review")}
              </Button>
            )}
          </div>
        }
        filters={
          phone ? (
            <Tabs value={tab} onChange={showTab}>
              <TabList aria-label={t("views")}>
                <Tab value="chat">{t("chat")}</Tab>
                <Tab value="draft">{t("draft")}</Tab>
              </TabList>
            </Tabs>
          ) : undefined
        }
      />
      {(error !== null || settings.isError || chatFailure !== null || applied || stale) && (
        <div className="shrink-0 space-y-2 px-4 pb-2 @xl:px-6">
          {stale && (
            <Alert
              tone="warning"
              title={t(`targetChanged.${work.summary.kind}`)}
              action={<Button size="sm" variant="ghost" disabled={locked} onClick={() => void carryOver()}>{t("targetChanged.carry")}</Button>}
            />
          )}
          {applied && <Alert tone="success" title={t("savedBody")} action={<Button size="sm" variant="ghost" onClick={openSaved}>{t("openSaved")}</Button>} />}
          {error !== null && <FailureState size="sm" error={error} />}
          {settings.isError && <FailureState size="sm" error={settings.error} onRetry={() => void settings.refetch()} />}
          {chatFailure !== null && <FailureState size="sm" error={chatFailure} onRetry={() => { if (work.summary.applied_id) void launchChat(work.summary.applied_id); }} />}
        </div>
      )}
      {phone ? (
        <div className="min-h-0 flex-1">
          <div className={tab === "chat" ? "h-full" : "hidden"}>{chat}</div>
          <div className={tab === "draft" ? "h-full" : "hidden"}>{draft}</div>
        </div>
      ) : (
        <Resizable side="right" defaultSize={560} minSize={380} maxSize={960} storageKey="creator-draft-width" panel={draft} className="min-h-0 flex-1">
          {chat}
        </Resizable>
      )}
      {crop.editor}
      <ModelChoiceSheet
        api={api}
        open={search.panel === "model"}
        onClose={close}
        title={t("model")}
        value={model}
        onChange={setModel}
        nullChoice={{ title: t("defaultModel") }}
        requirement={{ kind: "chat", input: ["text"], output: ["text"] }}
        onAddModel={() => void navigate({ to: "/settings/$section", params: { section: "models" } })}
      />
      <ReviewSheet open={search.panel === "review"} phone={phone} work={work} baseline={baseline} complete={complete} canSave={canReview && !stale} busy={busy} onClose={close} onApply={(chat) => void apply(chat)} />
      <HistorySheet
        open={search.panel === "history"}
        phone={phone}
        work={work}
        baseline={baseline}
        selected={selected}
        canRestore={!locked && !dirty}
        onSelect={setSelected}
        onClose={close}
        onRestore={(proposal) => void saveDraft(proposal.draft).then((saved) => { if (saved) close(); })}
      />
    </div>
  );
}
