import { useEffect, useRef, useState } from "react";
import { ApiFailure } from "@/api/client";
import { useQueryClient } from "@tanstack/react-query";
import { useLocation, useNavigate, useParams, useRouter, useSearch } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import {
  ArrowRight,
  Check,
  PenLine,
  RefreshCw,
  Sparkles,
  Square,
  Trash2,
  Wand2,
} from "lucide-react";
import {
  ActionBar,
  Alert,
  Button,
  confirm,
  CrispStringsProvider,
  EmptyState,
  icon,
  IconButton,
  Page,
  Skeleton,
  Spinner,
  Stepper,
  Text,
  toast,
} from "@lettuceai/crisp";
import type {
  LorebookDraftView,
  LorebookPlanInput,
  LorebookPlanView,
  LorebookProjectCommitTarget,
} from "@/api/generated/bindings";
import { usePageLayout } from "@/shared/hooks/usePageLayout";
import { FailureState } from "@/shared/ui/FailureState";
import { refreshLibrary } from "../data/actions";
import {
  briefDraftKey,
  draftCounts,
  emptyBrief,
  mergeReviewEdit,
  planInput,
  projectStep,
  type BriefDraft,
} from "../data/lorebook-projects";
import { CreationError } from "./CreationError";
import { DraftEditor, PlanEditor, RefineEditor } from "./Editors";
import { Entries, Outline, SaveProject } from "./ProjectViews";
import { useProject } from "./use-project";

export function LorebookProjectPage() {
  const { projectId } = useParams({ from: "/library/lorebooks/projects/$projectId" });
  return <ProjectContent key={projectId} projectId={projectId} />;
}
function ProjectContent({ projectId }: { projectId: string }) {
  const { t, i18n } = useTranslation("lorebook-create");
  const state = useProject(projectId);
  const { project, api, busy, perform } = state;
  const { phone } = usePageLayout();
  const search = useSearch({ from: "/library/lorebooks/projects/$projectId" });
  const navigate = useNavigate();
  const router = useRouter();
  const client = useQueryClient();
  const flowPushed = useLocation({ select: (location) => location.state.lorebookFlowPushed });
  const routeKey = useLocation({ select: (location) => location.state.__TSR_key });
  const pushed = useLocation({ select: (location) => location.state.lorebookPanelPushed });
  const [target, setTarget] = useState("new");
  const undoToasts = useRef<(string | number)[]>([]);
  useEffect(
    () => () => {
      for (const id of undoToasts.current) toast.dismiss(id);
    },
    [],
  );
  const [selection, setSelection] = useState<Set<string> | null>(null);
  const [skipCoherence, setSkipCoherence] = useState(false);
  const natural = project ? projectStep(project) : 0;
  const step =
    search.step === "save" && natural >= 2
      ? 3
      : search.step === "outline" && natural >= 1
        ? 1
        : search.step === "entries" && natural >= 2
          ? 2
          : natural;
  const change = (patch: typeof search) =>
    void navigate({
      to: "/library/lorebooks/projects/$projectId",
      params: { projectId },
      search: patch,
      state: { lorebookFlowPushed: flowPushed ?? false },
      replace: true,
      resetScroll: false,
    });
  const open = (panel: "plan" | "draft" | "refine", id?: string) => {
    state.clearError();
    void navigate({
      to: "/library/lorebooks/projects/$projectId",
      params: { projectId },
      search: { ...search, panel, ...(id ? { entry: id } : {}) },
      state: { lorebookPanelPushed: true, lorebookFlowPushed: flowPushed ?? false },
      resetScroll: false,
    });
  };
  const close = () => {
    if (router.state.location.state.__TSR_key !== routeKey) return;
    if (pushed) router.history.back();
    else change(search.step ? { step: search.step } : {});
  };
  const discard = async () => {
    if (!project || busy) return;
    if (
      !(await confirm({
        title: t("discardTitle"),
        message: t("discardBody"),
        confirmLabel: t("discard"),
        cancelLabel: t("cancel"),
        destructive: true,
      }))
    )
      return;
    const result = await perform((view, operation) => {
      const request = { project_id: projectId, expected_revision: view.revision };
      return api.call("lorebookProjectCancel", operation("lorebookProjectCancel", request));
    });
    if (result) {
      await refreshLibrary(client);
      if (!state.isActive()) return;
      void navigate({ to: "/library", state: { libraryKind: "lorebook" }, replace: true });
    }
  };
  const stop = () =>
    perform((view, operation) => {
      const request = { project_id: projectId, expected_revision: view.revision };
      return api.call("lorebookProjectStop", operation("lorebookProjectStop", request));
    });
  const plan = () =>
    perform((view, operation) => {
      const request = { project_id: projectId, expected_revision: view.revision };
      return api.call("lorebookProjectPlan", operation("lorebookProjectPlan", request));
    });
  const write = () =>
    perform(async (view, operation) => {
      let current = view;
      if (view.stage === "awaiting_outline_approval") {
        const request = { project_id: projectId, expected_revision: view.revision };
        current = await api.call(
          "lorebookProjectOutlineApprove",
          operation("lorebookProjectOutlineApprove", request),
        );
      }
      const request = { project_id: projectId, expected_revision: current.revision };
      return api.call("lorebookProjectWriteAll", operation("lorebookProjectWriteAll", request));
    });
  const approve = (draft: LorebookDraftView) =>
    perform((view, operation) => {
      const request = {
        project_id: projectId,
        expected_revision: view.revision,
        plan_id: draft.plan_id,
        approved: draft.status !== "approved",
      };
      return api.call(
        "lorebookProjectDraftSetApproved",
        operation("lorebookProjectDraftSetApproved", request),
      );
    }).then(Boolean);
  const check = () => {
    setSkipCoherence(false);
    setSelection(null);
    return perform((view, operation) => {
      const request = { project_id: projectId, expected_revision: view.revision };
      return api.call("lorebookProjectCoherence", operation("lorebookProjectCoherence", request));
    });
  };
  const applicable =
    project?.coherence_changes
      .filter((change) => change.kind !== "flag_contradiction")
      .map((change) => change.id) ?? [];
  const selected = new Set([...(selection ?? applicable)].filter((id) => applicable.includes(id)));
  const apply = async () => {
    const result = await perform((view, operation) => {
      const request = {
        project_id: projectId,
        expected_revision: view.revision,
        accepted_change_ids: [...selected],
      };
      return api.call(
        "lorebookProjectCoherenceApply",
        operation("lorebookProjectCoherenceApply", request),
      );
    });
    if (result) {
      setSkipCoherence(true);
      toast.success(t("changesApplied", { count: selected.size }));
    }
  };
  const changeBrief = async () => {
    if (
      !project ||
      !(await confirm({
        title: t("changeBrief"),
        message: t("changeBriefBody"),
        confirmLabel: t("changeBrief"),
        cancelLabel: t("cancel"),
      }))
    )
      return;
    const result = await perform((view, operation) => {
      const request = { project_id: projectId, expected_revision: view.revision };
      return api.call("lorebookProjectCancel", operation("lorebookProjectCancel", request));
    });
    if (!result) return;
    const draft: BriefDraft = {
      ...emptyBrief(),
      step: "brief",
      name: project.lorebook_name ?? "",
      brief: project.brief,
      target: project.target_count,
      texts: project.sources
        .filter((source) => !source.document)
        .map((source) => ({ id: source.source_id, label: source.label, text: source.text ?? "" })),
      documents: project.sources.flatMap((source) =>
        source.document ? [{ id: source.document.asset_id, label: source.label, size: null }] : [],
      ),
    };
    client.removeQueries({ queryKey: ["lorebook-create-operation"] });
    client.setQueryData(briefDraftKey, draft);
    void navigate({
      to: "/library/lorebooks/new",
      state: { lorebookFlowPushed: flowPushed ?? false },
      replace: true,
    });
  };
  const save = async () => {
    const result = await perform(async (view, operation) => {
      let destination: LorebookProjectCommitTarget = {
        type: "new_lorebook",
        name: view.lorebook_name,
      };
      if (target !== "new") {
        const existing = await api.call("lorebookGet", { lorebook_id: target });
        destination = {
          type: "existing_lorebook",
          lorebook_id: target,
          expected_revision: existing.lorebook.revision,
        };
      }
      const request = {
        project_id: projectId,
        expected_revision: view.revision,
        target: destination,
      };
      return api.call("lorebookProjectCommit", operation("lorebookProjectCommit", request));
    });
    if (result) {
      await refreshLibrary(client);
      if (!state.isActive()) return;
      void navigate({
        to: "/library",
        search: { kind: "lorebook", item: result.lorebook_id },
        state: { libraryKind: "lorebook" },
        replace: true,
      });
    }
  };
  const editOutline = async (
    value: LorebookPlanInput | null,
    removeId?: string,
    initial?: LorebookPlanView | null,
  ) => {
    const result = await perform((view, operation) => {
      const outline = view.outline.filter((plan) => plan.plan_id !== removeId).map(planInput);
      if (value) {
        const at = outline.findIndex((plan) => plan.plan_id === value.plan_id);
        if (at < 0) {
          if (value.plan_id !== null)
            throw new ApiFailure("conflict", "outline entry removed", null);
          outline.push({ ...value, plan_id: null });
        } else
          outline[at] = initial ? mergeReviewEdit(planInput(initial), value, outline[at]!) : value;
      }
      const request = { project_id: projectId, expected_revision: view.revision, outline };
      return api.call(
        "lorebookProjectOutlineUpdate",
        operation("lorebookProjectOutlineUpdate", request),
      );
    });
    if (result) close();
    return result;
  };
  const remove = async (plan: LorebookPlanView) => {
    const at = project?.outline.findIndex((item) => item.plan_id === plan.plan_id) ?? 0;
    const result = await editOutline(null, plan.plan_id);
    if (result)
      undoToasts.current.push(
        toast.success(t("removedPlan", { name: plan.title }), undefined, {
          action: {
            label: t("undo"),
            onClick: () =>
              void perform((view, operation) => {
                const outline = view.outline.map(planInput);
                outline.splice(Math.min(at, outline.length), 0, { ...plan, plan_id: null });
                const request = {
                  project_id: projectId,
                  expected_revision: view.revision,
                  outline,
                };
                return api.call(
                  "lorebookProjectOutlineUpdate",
                  operation("lorebookProjectOutlineUpdate", request),
                );
              }),
          },
        }),
      );
  };
  const draft = project?.drafts.find((item) => item.plan_id === search.entry);
  const editingPlan = project?.outline.find((item) => item.plan_id === search.entry) ?? null;
  const counts = project ? draftCounts(project) : null;
  const active = Boolean(project?.active_job_ids.length);
  const checking = active && project?.active_phase === "coherence";
  const coherence = Boolean(project?.coherence_changes.length) && !skipCoherence;
  const ready = Boolean(
    project &&
      ["drafts_ready", "coherence_review"].includes(project.stage) &&
      !active &&
      counts?.saved,
  );
  let primary;
  let secondary;
  if (project && natural === 0 && !active)
    primary = (
      <Button
        variant="primary"
        loading={busy}
        leading={<Wand2 size={icon.md} />}
        onClick={() => void plan()}
      >
        {t(project.stage === "created" ? "planEntries" : "tryAgain")}
      </Button>
    );
  else if (project && step === 1 && project.stage === "awaiting_outline_approval")
    primary = (
      <Button
        variant="primary"
        loading={busy}
        disabled={!project.outline.length || active}
        leading={<PenLine size={icon.md} />}
        onClick={() => void write()}
      >
        {t("writeEntries", { count: project.outline.length })}
      </Button>
    );
  else if (step === 1)
    primary = (
      <Button variant="primary" onClick={() => change({ step: "entries" })}>
        {t("continue")}
      </Button>
    );
  else if (step === 3)
    primary = (
      <Button
        variant="primary"
        loading={busy}
        disabled={!ready || !target}
        leading={<Check size={icon.md} />}
        onClick={() => void save()}
      >
        {t("saveLorebook")}
      </Button>
    );
  else if (step === 2 && coherence && !active)
    primary = (
      <Button
        variant="primary"
        loading={busy}
        disabled={!selected.size}
        onClick={() => void apply()}
      >
        {t("applyChanges", { count: selected.size })}
      </Button>
    );
  else if (step === 2)
    primary = (
      <Button
        variant="primary"
        disabled={!ready || busy}
        trailing={<ArrowRight size={icon.md} />}
        onClick={() => change({ step: "save" })}
      >
        {t("continueSave")}
      </Button>
    );
  if (active)
    secondary = (
      <Button
        variant="ghost"
        disabled={busy}
        leading={<Square size={icon.sm} />}
        onClick={() => void stop()}
      >
        {t("stop")}
      </Button>
    );
  else if (step === 2 && coherence)
    secondary = (
      <Button variant="ghost" disabled={busy} onClick={() => setSkipCoherence(true)}>
        {t("skip")}
      </Button>
    );
  else if (step === 2 && counts && (counts.failed || counts.pending))
    secondary = (
      <Button
        variant="ghost"
        disabled={busy}
        leading={<RefreshCw size={icon.sm} />}
        onClick={() => void write()}
      >
        {t(counts.pending ? "writeRemaining" : "retryFailed", {
          count: counts.failed + counts.pending,
        })}
      </Button>
    );
  else if (step === 2)
    secondary = (
      <Button
        variant="secondary"
        disabled={!ready || busy}
        leading={<Sparkles size={icon.sm} />}
        onClick={() => void check()}
      >
        {t("checkTogether")}
      </Button>
    );
  const done = project?.stage === "committed" || project?.stage === "cancelled";
  const jobFailure = Object.values(state.jobs)
    .filter((job) => job.state === "failed" || job.state === "interrupted")
    .sort((left, right) => right.updated_at - left.updated_at)[0];
  const actions = !done ? (
    <ActionBar
      pinned={phone}
      className={phone ? "" : "px-8 pb-6 pt-2"}
      primary={primary}
      secondary={secondary}
    />
  ) : undefined;
  return (
    <CrispStringsProvider
      locale={i18n.language}
      strings={{ sheetClose: t("close"), tagInputRemove: (tag) => t("removeNamed", { name: tag }) }}
    >
      <Page
        title={project?.lorebook_name ?? t("newLorebook")}
        meta={phone ? undefined : t("writtenWithAi")}
        onBack={() => {
          if (flowPushed) router.history.back();
          else void navigate({ to: "/library", state: { libraryKind: "lorebook" }, replace: true });
        }}
        backLabel={t("library")}
        contentClassName="max-w-3xl space-y-6"
        actions={
          project && !done ? (
            <IconButton label={t("discard")} disabled={busy} onClick={() => void discard()}>
              <Trash2 size={icon.lg} />
            </IconButton>
          ) : undefined
        }
        footer={actions}
      >
        {state.query.isError ? (
          <FailureState error={state.query.error} onRetry={() => void state.query.refetch()} />
        ) : !project ? (
          <div className="space-y-4" role="status" aria-label={t("loadingProject")}>
            <Skeleton className="h-12 w-full" />
            <Skeleton className="h-48 w-full" />
          </div>
        ) : done ? (
          <EmptyState
            title={t(project.stage === "cancelled" ? "projectDiscarded" : "projectSaved")}
            action={
              <Button
                onClick={() =>
                  void navigate({
                    to: "/library",
                    state: { libraryKind: "lorebook" },
                    ...(project.commit
                      ? { search: { kind: "lorebook", item: project.commit.lorebook_id } }
                      : {}),
                  })
                }
              >
                {t("openLibrary")}
              </Button>
            }
          />
        ) : (
          <>
            <Stepper
              steps={(["brief", "outline", "entries", "save"] as const).map((id) => ({
                id,
                label: t(`steps.${id}`),
              }))}
              current={project.stage === "planning" || project.stage === "plan_failed" ? 1 : step}
              onStepClick={(next) => {
                if (next === 1 && natural >= 1) change({ step: "outline" });
                else if (next === 2 && natural >= 2) change({ step: "entries" });
              }}
            />
            {step === 0 ? (
              active ? (
                <div className="flex flex-col items-center gap-4 py-16 text-center">
                  <Spinner size="lg" label={t("planning")} />
                  <div className="space-y-1">
                    <Text variant="title">
                      {t("planningCount", { count: project.target_count })}
                    </Text>
                    <Text variant="hint">
                      {t("readingSources", { count: project.sources.length })}
                    </Text>
                  </div>
                  <Text variant="hint" className="max-w-sm">
                    {t("canLeave")}
                  </Text>
                </div>
              ) : (
                <div className="space-y-5">
                  {project.stage !== "created" && (
                    <Alert
                      tone="danger"
                      title={t("planningFailed")}
                      description={t("planningFailedBody")}
                      details={t(`jobErrors.${project.plan_failure?.code ?? "unknown"}`, {
                        defaultValue: t("jobErrors.unknown"),
                      })}
                    />
                  )}
                  <Text variant="secondary">{project.brief}</Text>
                  <Button variant="ghost" onClick={() => void changeBrief()}>
                    {t("changeBrief")}
                  </Button>
                </div>
              )
            ) : step === 1 ? (
              <Outline
                project={project}
                editable={project.stage === "awaiting_outline_approval" && !busy}
                onEdit={(plan) => open("plan", plan?.plan_id)}
              />
            ) : step === 3 ? (
              <SaveProject project={project} target={target} onTarget={setTarget} />
            ) : (
              <Entries
                project={project}
                busy={busy}
                onApprove={approve}
                onEdit={(draft) => open("draft", draft.plan_id)}
                onRefine={(draft) => open("refine", draft.plan_id)}
                checking={checking}
                coherence={coherence && !active}
                selected={selected}
                onSelected={setSelection}
                onFix={(draft) => open("draft", draft.plan_id)}
              />
            )}
          </>
        )}
        {jobFailure && (
          <Alert
            tone="danger"
            title={t("jobErrors.unknown")}
            description={t(`jobErrors.${jobFailure.failure?.code ?? "unknown"}`, {
              defaultValue: t("jobErrors.unknown"),
            })}
          />
        )}
        <CreationError error={state.error} busy={busy} retry={state.retry} />
        {state.watchError !== null && (
          <FailureState error={state.watchError} onRetry={state.retryWatch} size="sm" />
        )}
        {search.panel === "plan" && project && (
          <PlanEditor
            key={search.entry ?? "new"}
            plan={editingPlan}
            phone={phone}
            busy={busy}
            error={state.error}
            retry={state.retry}
            onClose={close}
            onSave={(plan, initial) => void editOutline(plan, undefined, initial)}
            {...(editingPlan ? { onRemove: () => void remove(editingPlan) } : {})}
          />
        )}
        {search.panel === "draft" && draft && (
          <DraftEditor
            key={draft.plan_id}
            draft={draft}
            phone={phone}
            busy={busy}
            error={state.error}
            retry={state.retry}
            onClose={close}
            onSave={(value, initial) =>
              void perform((view, operation) => {
                const latest = view.drafts.find((draft) => draft.plan_id === initial.plan_id);
                if (!latest) throw new ApiFailure("conflict", "draft removed", null);
                const fields = (draft: LorebookDraftView) => ({
                  title: draft.title,
                  content: draft.content,
                  keywords: draft.keywords,
                  always_active: draft.always_active,
                });
                const request = {
                  project_id: projectId,
                  expected_revision: view.revision,
                  plan_id: draft.plan_id,
                  ...mergeReviewEdit(fields(initial), value, fields(latest)),
                };
                return api.call(
                  "lorebookProjectDraftUpdate",
                  operation("lorebookProjectDraftUpdate", request),
                );
              }).then((result) => {
                if (result) close();
              })
            }
          />
        )}
        {search.panel === "refine" && draft && (
          <RefineEditor
            key={draft.plan_id}
            draft={draft}
            phone={phone}
            busy={busy}
            error={state.error}
            retry={state.retry}
            onClose={close}
            onSave={(feedback) =>
              void perform((view, operation) => {
                const request = {
                  project_id: projectId,
                  expected_revision: view.revision,
                  plan_id: draft.plan_id,
                  feedback,
                };
                return api.call(
                  "lorebookProjectRefine",
                  operation("lorebookProjectRefine", request),
                );
              }).then((result) => {
                if (result) close();
              })
            }
          />
        )}
      </Page>
    </CrispStringsProvider>
  );
}
