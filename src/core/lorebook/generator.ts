import { invoke } from "@tauri-apps/api/core";

export type LorebookGeneratorJobStage =
  | "created"
  | "planning"
  | "awaitingOutlineApproval"
  | "drafting"
  | "draftsReady"
  | "coherenceReview"
  | "committed"
  | "cancelled"
  | "failed";

export type LorebookGeneratorDraftStatus =
  | "pending"
  | "drafting"
  | "drafted"
  | "approved"
  | "failed";

export type LorebookGeneratorSourceFileKind = "txt" | "md" | "pdf";

export type LorebookGeneratorSourceInput =
  | { type: "text"; label: string; body: string }
  | {
      type: "file";
      name: string;
      dataBase64: string;
      kind: LorebookGeneratorSourceFileKind;
    };

export interface LorebookGeneratorOverrides {
  modelId?: string;
  plannerPromptTemplateId?: string;
  writerPromptTemplateId?: string;
  refinePromptTemplateId?: string;
  coherencePromptTemplateId?: string;
}

export interface LorebookGeneratorEntryPlan {
  idx: number;
  title: string;
  category: string;
  proposedKeys: string[];
  rationale: string;
  sourceRefs: string[];
}

export interface LorebookGeneratorDraftRevision {
  feedback: string;
  content: string;
  timestampMs: number;
}

export interface LorebookGeneratorEntryDraft {
  planIdx: number;
  title: string;
  keywords: string[];
  content: string;
  alwaysActive: boolean;
  status: LorebookGeneratorDraftStatus;
  revisions: LorebookGeneratorDraftRevision[];
}

export type LorebookGeneratorCoherenceChange =
  | {
      kind: "mergeKeys";
      id: string;
      entryIdx: number;
      removeKeys: string[];
      reason: string;
    }
  | {
      kind: "renameTerm";
      id: string;
      oldTerm: string;
      newTerm: string;
      affectedEntryIdxs: number[];
      reason: string;
    }
  | {
      kind: "flagContradiction";
      id: string;
      entryIdxs: number[];
      description: string;
    }
  | {
      kind: "toggleAlwaysActive";
      id: string;
      entryIdx: number;
      newValue: boolean;
      reason: string;
    };

export interface LorebookGeneratorWorldDigestExcerpt {
  id: string;
  label: string;
  content: string;
}

export interface LorebookGeneratorJobState {
  id: string;
  createdAtMs: number;
  updatedAtMs: number;
  stage: LorebookGeneratorJobStage;
  brief: string;
  initialLorebookName: string | null;
  targetCount: number;
  digest: { excerpts: LorebookGeneratorWorldDigestExcerpt[] };
  overrides: LorebookGeneratorOverrides;
  outline: LorebookGeneratorEntryPlan[];
  drafts: LorebookGeneratorEntryDraft[];
  coherenceProposals: LorebookGeneratorCoherenceChange[];
  lastError: string | null;
}

export interface LorebookGeneratorCommitResult {
  lorebookId: string;
  createdEntryIds: string[];
}

export async function readFileAsBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result;
      if (typeof result !== "string") {
        reject(new Error("FileReader did not return a string"));
        return;
      }
      const comma = result.indexOf(",");
      resolve(comma >= 0 ? result.slice(comma + 1) : result);
    };
    reader.onerror = () => reject(reader.error ?? new Error("read failed"));
    reader.readAsDataURL(file);
  });
}

export function detectFileKind(name: string): LorebookGeneratorSourceFileKind | null {
  const lower = name.toLowerCase();
  if (lower.endsWith(".pdf")) return "pdf";
  if (lower.endsWith(".md") || lower.endsWith(".markdown")) return "md";
  if (lower.endsWith(".txt") || lower.endsWith(".text")) return "txt";
  return null;
}

export async function lorebookGenCreate(args: {
  brief: string;
  sources: LorebookGeneratorSourceInput[];
  targetCount: number;
  initialLorebookName?: string | null;
  overrides?: LorebookGeneratorOverrides;
}): Promise<LorebookGeneratorJobState> {
  return invoke<LorebookGeneratorJobState>("lorebook_gen_create", {
    args: {
      brief: args.brief,
      sources: args.sources,
      targetCount: args.targetCount,
      initialLorebookName: args.initialLorebookName ?? null,
      overrides: args.overrides ?? null,
    },
  });
}

export async function lorebookGenGet(jobId: string): Promise<LorebookGeneratorJobState> {
  return invoke<LorebookGeneratorJobState>("lorebook_gen_get", { jobId });
}

export async function lorebookGenDefaultTargetCount(): Promise<number> {
  return invoke<number>("lorebook_gen_default_target_count");
}

export async function lorebookGenRunPlanner(jobId: string): Promise<LorebookGeneratorJobState> {
  return invoke<LorebookGeneratorJobState>("lorebook_gen_run_planner", { jobId });
}

export async function lorebookGenUpdateOutline(args: {
  jobId: string;
  outline: LorebookGeneratorEntryPlan[];
}): Promise<LorebookGeneratorJobState> {
  return invoke<LorebookGeneratorJobState>("lorebook_gen_update_outline", { args });
}

export async function lorebookGenApproveOutline(
  jobId: string,
): Promise<LorebookGeneratorJobState> {
  return invoke<LorebookGeneratorJobState>("lorebook_gen_approve_outline", { jobId });
}

export async function lorebookGenDraftNext(jobId: string): Promise<LorebookGeneratorJobState> {
  return invoke<LorebookGeneratorJobState>("lorebook_gen_draft_next", { jobId });
}

export async function lorebookGenRefineEntry(args: {
  jobId: string;
  entryIdx: number;
  feedback: string;
}): Promise<LorebookGeneratorJobState> {
  return invoke<LorebookGeneratorJobState>("lorebook_gen_refine_entry", { args });
}

export async function lorebookGenSetDraftApproved(args: {
  jobId: string;
  entryIdx: number;
  approved: boolean;
}): Promise<LorebookGeneratorJobState> {
  return invoke<LorebookGeneratorJobState>("lorebook_gen_set_draft_approved", { args });
}

export async function lorebookGenEditDraft(args: {
  jobId: string;
  entryIdx: number;
  title: string;
  keywords: string[];
  content: string;
  alwaysActive: boolean;
}): Promise<LorebookGeneratorJobState> {
  return invoke<LorebookGeneratorJobState>("lorebook_gen_edit_draft", { args });
}

export async function lorebookGenRunCoherence(
  jobId: string,
): Promise<LorebookGeneratorJobState> {
  return invoke<LorebookGeneratorJobState>("lorebook_gen_run_coherence", { jobId });
}

export async function lorebookGenApplyCoherence(args: {
  jobId: string;
  acceptedChangeIds: string[];
}): Promise<LorebookGeneratorJobState> {
  return invoke<LorebookGeneratorJobState>("lorebook_gen_apply_coherence", { args });
}

export async function lorebookGenCommit(args: {
  jobId: string;
  targetLorebookId?: string | null;
  newName?: string | null;
}): Promise<LorebookGeneratorCommitResult> {
  return invoke<LorebookGeneratorCommitResult>("lorebook_gen_commit", { args });
}

export async function lorebookGenCancel(jobId: string): Promise<void> {
  return invoke<void>("lorebook_gen_cancel", { jobId });
}
