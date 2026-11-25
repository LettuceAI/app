import { describe, expect, it } from "vitest";
import type { ParticipantView, TimelineMessage } from "@/api/generated/bindings";
import { displayTextOf, resolveScenePlaceholders } from "./messages";

const directParticipants: ParticipantView[] = [
  { id: "pc", role: "character", name: "Cassian", character_id: "ch1", avatar: null },
  { id: "pu", role: "user", name: "Morgan", character_id: null, avatar: null },
];

const groupParticipants: ParticipantView[] = [
  { id: "p1", role: "character", name: "Aria", character_id: "ch1", avatar: null },
  { id: "p2", role: "character", name: "Bram", character_id: "ch2", avatar: null },
  { id: "pu", role: "user", name: "Morgan", character_id: null, avatar: null },
];

const msg = (role: TimelineMessage["role"], text: string): TimelineMessage => ({
  id: "m1",
  role,
  author_participant_id: null,
  parts: [{ type: "text", text }],
  reasoning: null,
  created_at: 1,
  effective_time: 1,
  candidate_index: null,
  candidate_count: 1,
  pinned: false,
  scene_image: null,
});

describe("resolveScenePlaceholders", () => {
  it("replaces character, user, and persona placeholders in direct chats", () => {
    expect(
      resolveScenePlaceholders(
        "{{char}}, {{ char.name }}, {{user}}, {{ user.name }}, {{user_name}}, {{persona}}, {{persona.name}}, {{persona_name}}",
        directParticipants,
      ),
    ).toBe("Cassian, Cassian, Morgan, Morgan, Morgan, Morgan, Morgan, Morgan");
  });

  it("handles special replacement characters like $& in names literally", () => {
    const special: ParticipantView[] = [
      { id: "pc", role: "character", name: "A$&B", character_id: "ch1", avatar: null },
      { id: "pu", role: "user", name: "U$1", character_id: null, avatar: null },
    ];
    expect(resolveScenePlaceholders("{{char}} meets {{user}}", special)).toBe("A$&B meets U$1");
  });

  it("resolves group member mentions and user/persona tokens without replacing {{char}}", () => {
    expect(
      resolveScenePlaceholders(
        '{{@"Aria"}} and {{@"Bram"}} greet {{user}} ({{persona}}), leaving {{char}} and {{@"Ghost"}}.',
        groupParticipants,
      ),
    ).toBe('Aria and Bram greet Morgan (Morgan), leaving {{char}} and {{@"Ghost"}}.');
  });

  it("only resolves placeholders on scene messages in displayTextOf", () => {
    expect(displayTextOf(msg("scene", "{{char}} waits for {{user}}."), directParticipants)).toBe(
      "Cassian waits for Morgan.",
    );
    expect(displayTextOf(msg("assistant", "Use {{char}} and {{user}}."), directParticipants)).toBe(
      "Use {{char}} and {{user}}.",
    );
  });
});
