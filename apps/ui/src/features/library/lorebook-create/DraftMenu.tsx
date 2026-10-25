import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Trash2, Wand2 } from "lucide-react";
import { confirm, icon, ListGroup, ListRowButton, Sheet, Skeleton } from "@lettuceai/crisp";
import { FailureState } from "@/shared/ui/FailureState";
import { CreationError } from "./CreationError";
import { useProject } from "./use-project";
import { draftCounts } from "../data/lorebook-projects";

export function DraftMenu({ id, onClose }: { id: string; onClose: () => void }) {
  const { t } = useTranslation("lorebook-create");
  const state = useProject(id);
  const { project, api, busy, perform } = state;
  const navigate = useNavigate();
  const counts = project ? draftCounts(project) : null;
  const step =
    project?.stage === "awaiting_outline_approval"
      ? t("draftOutlineReady")
      : project?.stage === "plan_failed"
        ? t("planningFailed")
        : project?.stage === "created"
          ? t("draftBriefReady")
          : project?.stage === "planning"
            ? t("planningCount", { count: project.target_count })
            : counts
              ? t("writtenProgress", { written: counts.written, count: counts.total })
              : "";
  const discard = async () => {
    if (
      !project ||
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
      const request = { project_id: id, expected_revision: view.revision };
      return api.call("lorebookProjectCancel", operation("lorebookProjectCancel", request));
    });
    if (result) onClose();
  };
  return (
    <Sheet
      open
      onClose={onClose}
      size="sm"
      title={project?.lorebook_name ?? t("untitled")}
      description={
        project
          ? t(project.active_job_ids.length ? "draftRunning" : "draftWaitingForYou")
          : undefined
      }
    >
      {state.query.isError ? (
        <FailureState error={state.query.error} onRetry={() => void state.query.refetch()} />
      ) : !project ? (
        <Skeleton className="h-32 w-full" />
      ) : (
        <div className="space-y-4 pb-1">
          <ListGroup>
            <ListRowButton
              icon={<Wand2 size={icon.lg} />}
              title={t("continue")}
              description={step}
              disabled={busy}
              onClick={() =>
                void navigate({
                  to: "/library/lorebooks/projects/$projectId",
                  params: { projectId: id },
                  state: { lorebookFlowPushed: true },
                  replace: true,
                })
              }
            />
          </ListGroup>
          <ListGroup>
            <ListRowButton
              icon={<Trash2 size={icon.lg} />}
              title={t("discardDraft")}
              tone="danger"
              chevron={false}
              disabled={busy}
              onClick={() => void discard()}
            />
          </ListGroup>
          <CreationError error={state.error} busy={busy} retry={state.retry} />
        </div>
      )}
    </Sheet>
  );
}
