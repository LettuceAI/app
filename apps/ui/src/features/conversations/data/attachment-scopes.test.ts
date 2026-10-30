import { describe, expect, it } from "vitest";
import type { ConversationSettingsView, ModelsView, ParticipantSettings } from "@/api/generated/bindings";
import { attachmentScopes } from "./attachment-scopes";

const catalog = { default_model_id: "vision", models: [{ id: "vision", input_scopes: ["text", "image"] }, { id: "text", input_scopes: ["text"] }] } as ModelsView;
const member = (id: string | null, patch: Partial<ParticipantSettings> = {}): ParticipantSettings => ({ participant_id: "p", character_id: "c", name: "Ada", enabled: true, muted: false, model: { id, source: "character" }, ...patch });
const group = (members: ParticipantSettings[], model: string | null = null) => ({ kind: "group", model: { id: model, source: "group" }, members: { participants: members } }) as ConversationSettingsView;

describe("group attachment scopes", () => {
  it("uses member models and the app default without a group-wide model", () => {
    expect(attachmentScopes(group([member("vision"), member(null)]), catalog)).toContain("image");
  });
  it("requires every enabled member to support the media, including muted members as in legacy", () => {
    expect(attachmentScopes(group([member("vision"), member("text", { muted: true })]), catalog)).not.toContain("image");
    expect(attachmentScopes(group([member("vision"), member("text", { enabled: false })]), catalog)).toContain("image");
  });
  it("honors conversation and member model overrides and missing models", () => {
    expect(attachmentScopes(group([member("text")], "vision"), catalog)).toContain("image");
    expect(attachmentScopes(group([member("missing")]), catalog)).toEqual([]);
    expect(attachmentScopes(group([]), catalog)).toEqual([]);
  });
});
