import { queryOptions } from "@tanstack/react-query";
import { ApiFailure, type ApiClient } from "@/api/client";
import type {
  LorebookPlanView,
  LorebookPlanInput,
  LorebookProjectSourceInput,
  LorebookProjectSummary,
  LorebookProjectView_Serialize,
} from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

export const projectKey = (id: string) =>
  [...queryKeys.lorebookProjects.all, "detail", id] as const;
export const projectQuery = (api: ApiClient, id: string) =>
  queryOptions({
    queryKey: projectKey(id),
    queryFn: () => api.call("lorebookProjectGet", { project_id: id }),
  });
export const projectsQuery = (api: ApiClient) =>
  queryOptions({
    queryKey: [...queryKeys.lorebookProjects.all, "list"],
    queryFn: async () => {
      const projects: LorebookProjectSummary[] = [];
      let cursor: string | null = null;
      do {
        const page: { items: LorebookProjectSummary[]; next_cursor: string | null } =
          await api.call("lorebookProjectsList", { cursor, limit: 100 });
        projects.push(...page.items);
        cursor = page.next_cursor;
      } while (cursor !== null);
      return projects.filter(
        (project) => project.stage !== "committed" && project.stage !== "cancelled",
      );
    },
  });
export const categories = [
  "character",
  "location",
  "faction",
  "item",
  "event",
  "concept",
  "rule",
  "other",
] as const;
export type Category = (typeof categories)[number];
export function category(value: string): Category {
  return categories.includes(value as Category) ? (value as Category) : "other";
}
export function projectStep(project: LorebookProjectView_Serialize) {
  if (["created", "planning", "plan_failed"].includes(project.stage)) return 0;
  if (project.stage === "awaiting_outline_approval") return 1;
  return 2;
}
export function draftCounts(project: LorebookProjectView_Serialize) {
  const drafts = project.drafts;
  return {
    total: drafts.length,
    written: drafts.filter((draft) => draft.status === "drafted" || draft.status === "approved")
      .length,
    approved: drafts.filter((draft) => draft.status === "approved").length,
    failed: drafts.filter((draft) => draft.status === "failed").length,
    pending: drafts.filter((draft) => draft.status === "pending" || draft.status === "drafting")
      .length,
    saved: drafts.filter((draft) => draft.title.trim() || draft.content.trim()).length,
  };
}
export function planInput(plan: LorebookPlanView): LorebookPlanInput {
  return { ...plan };
}
export function sourceLabels(project: LorebookProjectView_Serialize, refs: string[]) {
  return refs.map((id) => project.sources.find((source) => source.source_id === id)?.label ?? id);
}
export interface PastedSource {
  id: string;
  label: string;
  text: string;
}
export interface DocumentSource {
  id: string;
  label: string;
  size: number | null;
}
export interface BriefDraft {
  name: string;
  brief: string;
  target: number | null;
  characterId: string | null;
  personaId: string | null;
  texts: PastedSource[];
  documents: DocumentSource[];
  mode: "empty" | "ai" | "import";
  step: "start" | "brief";
}
export const emptyBrief = (): BriefDraft => ({
  name: "",
  brief: "",
  target: null,
  characterId: null,
  personaId: null,
  texts: [],
  documents: [],
  mode: "ai",
  step: "start",
});
export const briefDraftKey = ["lorebook-brief"] as const;
export const fileLimit = 50 * 1024 * 1024;
export const totalSourceLimit = 200 * 1024 * 1024;
export function sourceBytes(texts: PastedSource[], documents: DocumentSource[]) {
  return (
    texts.reduce((sum, value) => sum + new TextEncoder().encode(value.text).byteLength, 0) +
    documents.reduce((sum, value) => sum + (value.size ?? 0), 0)
  );
}
export function briefSources(draft: BriefDraft): LorebookProjectSourceInput[] {
  return [
    ...draft.texts.map(({ label, text }) => ({ type: "text" as const, label, text })),
    ...draft.documents.map(({ id }) => ({ type: "document" as const, asset_id: id })),
  ];
}
export function projectOperation<P extends object>(
  records: Map<string, object>,
  command: string,
  payload: P,
): P & { client_operation_id: string } {
  let record = records.get(command);
  if (!record) {
    record = { ...structuredClone(payload), client_operation_id: crypto.randomUUID() };
    records.set(command, record);
  }
  return record as P & { client_operation_id: string };
}
export function mergeReviewEdit<T extends object>(initial: T, edited: T, current: T): T {
  const next = { ...current };
  for (const key of Object.keys(edited) as (keyof T)[]) {
    if (JSON.stringify(edited[key]) === JSON.stringify(initial[key])) continue;
    if (
      JSON.stringify(current[key]) !== JSON.stringify(initial[key]) &&
      JSON.stringify(current[key]) !== JSON.stringify(edited[key])
    ) {
      throw new ApiFailure("conflict", "reviewed field changed", null);
    }
    next[key] = edited[key];
  }
  return next;
}
