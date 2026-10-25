import { describe, expect, it } from "vitest";
import type { LorebookDraftView, LorebookProjectView_Serialize } from "@/api/generated/bindings";
import {
  briefSources,
  category,
  draftCounts,
  emptyBrief,
  mergeReviewEdit,
  planInput,
  projectOperation,
  projectStep,
  sourceBytes,
  sourceLabels,
} from "./data/lorebook-projects";

const draft = (patch: Partial<LorebookDraftView> = {}): LorebookDraftView => ({
  plan_id: "plan",
  title: "Gate",
  content: "",
  keywords: [],
  always_active: false,
  status: "pending",
  revisions: [],
  ...patch,
});
const project = (
  patch: Partial<LorebookProjectView_Serialize> = {},
): LorebookProjectView_Serialize => ({
  project_id: "project",
  brief: "A kingdom",
  lorebook_name: "World",
  target_count: 5,
  stage: "drafting",
  sources: [],
  outline: [],
  drafts: [],
  coherence_changes: [],
  commit: null,
  active_job_ids: [],
  active_phase: null,
  plan_failure: null,
  revision: 1,
  created_at: 1,
  updated_at: 1,
  ...patch,
});
describe("lorebook creation accounting and request recovery", () => {
  it("preserves concurrent edits to fields the user did not change", () => {
    const original = { title: "Gate", content: "Old", keywords: ["gate"] };
    const edited = { ...original, title: "Northern gate" };
    const latest = { ...original, content: "Updated elsewhere" };
    expect(mergeReviewEdit(original, edited, latest)).toEqual({
      ...latest,
      title: "Northern gate",
    });
  });
  it("refuses to overwrite a concurrent edit to the same reviewed field", () => {
    const original = { title: "Gate" };
    expect(() => mergeReviewEdit(original, { title: "North" }, { title: "South" })).toThrow(
      "reviewed field changed",
    );
    expect(mergeReviewEdit(original, { title: "North" }, { title: "North" })).toEqual({
      title: "North",
    });
  });
  it("keeps file sources as asset identities and text sources as named text", () => {
    const value = {
      ...emptyBrief(),
      texts: [{ id: "note", label: "Harbour", text: "The docks" }],
      documents: [{ id: "asset", label: "notes.pdf", size: 20 }],
    };
    expect(briefSources(value)).toEqual([
      { type: "text", label: "Harbour", text: "The docks" },
      { type: "document", asset_id: "asset" },
    ]);
  });
  it("measures UTF-8 source bytes rather than JavaScript string length", () => {
    expect(
      sourceBytes(
        [{ id: "note", label: "World", text: "海岸🙂" }],
        [{ id: "asset", label: "notes.pdf", size: 512 }],
      ),
    ).toBe(522);
  });
  it("matches native title-or-content save semantics even for failed entries", () => {
    const value = project({
      drafts: [
        draft({ status: "approved", content: "Text" }),
        draft({ plan_id: "failed", status: "failed" }),
        draft({ plan_id: "blank", title: "", status: "failed" }),
        draft({ plan_id: "content", title: "", content: "Content only", status: "drafted" }),
      ],
    });
    expect(draftCounts(value)).toMatchObject({
      total: 4,
      written: 2,
      approved: 1,
      failed: 2,
      saved: 3,
    });
  });
  it("does not infer running jobs from a paused native drafting stage", () => {
    const value = project({
      stage: "drafting",
      drafts: [draft({ status: "pending" }), draft({ plan_id: "failed", status: "failed" })],
      active_job_ids: [],
    });
    expect(projectStep(value)).toBe(2);
    expect(draftCounts(value)).toMatchObject({ pending: 1, failed: 1 });
    expect(value.active_job_ids).toHaveLength(0);
  });
  it("preserves source identities and historical labels without looking up live characters", () => {
    const value = project({
      sources: [
        {
          source_id: "src_01",
          label: "Character: Deleted character",
          document: null,
          text: "Frozen definition",
        },
      ],
    });
    expect(sourceLabels(value, ["src_01", "old-source"])).toEqual([
      "Character: Deleted character",
      "old-source",
    ]);
  });
  it("keeps a plan id and source references when editing an existing outline", () => {
    const input = {
      plan_id: "plan",
      title: "The gate",
      category: "location",
      proposed_keys: ["gate"],
      rationale: "The entrance",
      source_refs: ["src_01"],
    };
    expect(planInput(input)).toEqual(input);
    expect(category("location")).toBe("location");
    expect(category("unrecognized")).toBe("other");
  });
  it("replays the exact request after an uncertain response, including its old revision", () => {
    const records = new Map<string, object>();
    const first = projectOperation(records, "commit", {
      project_id: "project",
      expected_revision: 4,
      target: { type: "existing", revision: 9 },
    });
    const retry = projectOperation(records, "commit", {
      project_id: "project",
      expected_revision: 5,
      target: { type: "existing", revision: 10 },
    });
    expect(retry).toEqual(first);
    expect(retry.client_operation_id).toMatch(/^[0-9a-f-]{36}$/);
  });
  it("freezes nested request data and creates fresh identities after a conflict or a new action", () => {
    const records = new Map<string, object>();
    const input = { keywords: ["gate"] };
    const first = projectOperation(records, "edit", input);
    input.keywords.push("harbour");
    expect(first.keywords).toEqual(["gate"]);
    const other = projectOperation(records, "approve", { approved: true });
    expect(other.client_operation_id).not.toBe(first.client_operation_id);
    records.clear();
    const retried = projectOperation(records, "edit", input);
    expect(retried.client_operation_id).not.toBe(first.client_operation_id);
    expect(retried.keywords).toEqual(["gate", "harbour"]);
  });
});
