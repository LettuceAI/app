import type {
  AssetRef,
  CharacterSummary,
  CompanionEmotionVector,
  CompanionRelationshipDefaults,
  CompanionRelationshipView,
  ConversationSettingsView,
  ConversationSummary,
  CreatorDraft,
  CreatorTurnView,
  GroupView_Serialize,
  LibraryChatAppearanceV1_Serialize,
  LorebookEntryView,
  MemoryItemView,
  ParticipantView,
  TimelineMessage,
  TimeView,
} from "@/api/generated/bindings";
import throneRoom from "@/shared/assets/customization/throne-room.webp";
import seraFace from "@/shared/assets/customization/sera-face.webp";
import cassianFace from "@/shared/assets/showcase/cassian-face.webp";
import mireilleFace from "@/shared/assets/showcase/mireille-face.webp";
import orenFace from "@/shared/assets/showcase/oren-face.webp";
import rellFace from "@/shared/assets/showcase/rell-face.webp";
import seraPortrait from "@/shared/assets/showcase/sera.webp";
import ysoldeFace from "@/shared/assets/showcase/ysolde-face.webp";

const minute = 60_000;
const hour = 60 * minute;
const day = 24 * hour;

const asset = (id: string, url: string): AssetRef => ({ asset_id: id, url });

interface Person {
  id: string;
  name: string;
  description: string;
  face: string | null;
}

const people: readonly Person[] = [
  { id: "cassian", name: "Cassian", description: "Ruthless young King of Welan. Rules with an iron fist wrapped in velvet.", face: cassianFace },
  { id: "mireille", name: "Mireille", description: "Court apothecary. Knows which cups are safe and never says so.", face: mireilleFace },
  { id: "rell", name: "Rell", description: "Gate mechanic. Talks to pulleys more than people.", face: rellFace },
  { id: "oren", name: "Oren", description: "The king's old tutor. Answers questions with better questions.", face: orenFace },
  { id: "ysolde", name: "Ysolde", description: "Exiled heir, quietly gathering the people Cassian wronged.", face: ysoldeFace },
  { id: "sera", name: "Sera Vance", description: "Smuggler captain with a debt in every harbour on the coast.", face: seraFace },
  { id: "anselm", name: "Brother Anselm", description: "Keeper of the chapel archive.", face: null },
  { id: "idris", name: "Captain Idris Vale", description: "Harbour watch. Owes Sera more than money.", face: null },
];

const person = (id: string): Person => people.find((entry) => entry.id === id) ?? people[0]!;

function characterOf(id: string, now: number, overrides: Partial<CharacterSummary> = {}): CharacterSummary {
  const entry = person(id);
  return {
    id,
    name: entry.name,
    avatar: entry.face ? asset(`${id}-face`, entry.face) : null,
    banner: null,
    description: entry.description,
    card_style: "circle",
    gradient: { type: "none" },
    revision: 1,
    created_at: now - 30 * day,
    updated_at: now - 30 * day,
    ...overrides,
  };
}

const castOf = (ids: readonly string[], now: number) => ids.map((id) => characterOf(id, now));

const chatAppearance: LibraryChatAppearanceV1_Serialize = {
  format_version: 1,
  font_size: "medium",
  line_spacing: "normal",
  bubble_style: "filled",
  bubble_radius: "rounded",
  bubble_max_width: "normal",
  bubble_padding: "normal",
  show_message_author: false,
  show_message_timestamp: false,
  timestamp_format: "relative",
  message_header_placement: "below",
  show_message_model: false,
  show_message_input_tokens: false,
  show_message_output_tokens: false,
  show_message_total_tokens: false,
  show_message_ttft: false,
  show_message_tokens_per_second: false,
  show_message_mtp: false,
  message_info_placement: "below_bubble",
  message_info_size: "small",
  message_gap: "relaxed",
  avatar_shape: "circle",
  avatar_size: "medium",
  chat_column_width: "full",
  chat_column_width_px: null,
  chat_column_align: "center",
  chat_header_moves: false,
  chat_footer_moves: false,
  participants_bar_enabled: true,
  participants_bar_avatar_size: "medium",
  participants_bar_avatar_shape: "round",
  participants_bar_background: "fading",
  participants_bar_gap: "normal",
  participants_bar_align: "left",
  participants_bar_hint_position: "bottom",
  chat_widget_area_enabled: false,
  chat_widget_center_mode: "both",
  chat_widget_slots: { left: [], right: [] },
  user_bubble_color: "accent",
  assistant_bubble_color: "neutral",
  user_bubble_color_hex: null,
  assistant_bubble_color_hex: null,
  footer_input_color_hex: null,
  message_text_color_hex: null,
  plain_text_color_hex: null,
  italic_text_color_hex: null,
  quoted_text_color_hex: null,
  inline_code_text_color_hex: null,
  transparent_header: false,
  background_dim: 0,
  background_blur: 0,
  bubble_blur: "none",
  bubble_opacity: 35,
  text_mode: "auto",
};

interface GroupSeed {
  id: string;
  name: string;
  memberIds: readonly string[];
  speaker: "round_robin" | "director";
  createdDaysAgo: number;
  chats: readonly { id: string; title: string; agoMs: number; preview: string }[];
}

const groupSeeds: readonly GroupSeed[] = [
  {
    id: "g-gate",
    name: "The northern gate",
    memberIds: ["cassian", "rell", "mireille"],
    speaker: "round_robin",
    createdDaysAgo: 9,
    chats: [
      { id: "gc-gate-tooth", title: "Who cracked the tooth", agoMs: 4 * minute, preview: "\"Slow is personal,\" Rell says, and sets the broken tooth on the table between you." },
      { id: "gc-gate-watch", title: "Night watch", agoMs: 2 * day, preview: "I'll take the first watch. Wake me if the counterweight moves." },
      { id: "gc-gate-first", title: "New chat", agoMs: 8 * day, preview: "\"If anyone is hurt, bring them to me. Not to him.\"" },
    ],
  },
  {
    id: "g-council",
    name: "Council of the crown",
    memberIds: ["cassian", "oren", "mireille", "ysolde", "sera", "anselm", "idris"],
    speaker: "director",
    createdDaysAgo: 30,
    chats: [
      { id: "gc-council-vote", title: "The vote on the gate", agoMs: 3 * hour, preview: "\"A council that agrees on everything is a council of one,\" Oren says mildly." },
      { id: "gc-council-tithe", title: "Harbour tithes", agoMs: 6 * day, preview: "\"Double, or the ships go elsewhere. Your choice, majesty.\"" },
    ],
  },
];

export const modelNames: Readonly<Record<string, string>> = { "deepseek-v3": "DeepSeek V3" };

function groupViewOf(seed: GroupSeed, now: number): GroupView_Serialize {
  return {
    id: seed.id,
    archived: false,
    draft: {
      name: seed.name,
      persona: { kind: "inherit" },
      speaker_selection: seed.speaker,
      memory_policy: "dynamic",
      disable_character_lorebooks: false,
      time_awareness: false,
      disable_character_notes: false,
      companion_mode: false,
      companion_personal_memories: true,
      companion_memory_write_back: true,
      group_roleplay_prompt_id: null,
      presentation: chatAppearance,
      presentation_overrides: [],
      members: seed.memberIds.map((id, ordinal) => ({
        character_id: id,
        ordinal,
        muted: false,
        model_profile_override: seed.id === "g-gate" && id === "cassian" ? "deepseek-v3" : null,
      })),
      background_asset_id: null,
      starting_scene: null,
    },
    assets: [],
    revision: 4,
    created_at: now - seed.createdDaysAgo * day,
    updated_at: now - seed.createdDaysAgo * day,
  };
}

function conversationOf(
  init: { id: string; title: string; agoMs: number; preview: string | null; source: ConversationSummary["source"]; faces: readonly string[] },
  now: number,
): ConversationSummary {
  return {
    id: init.id,
    kind: init.source.type === "group" ? "group" : "direct",
    title: init.title,
    avatars: init.faces.flatMap((id) => {
      const face = person(id).face;
      return face ? [asset(`${id}-face`, face)] : [];
    }),
    background: null,
    revision: 1,
    last_message_preview: init.preview,
    updated_at: now - init.agoMs,
    archived: false,
    source: init.source,
    message_count: 12,
    missing_models: [],
    memory_blocked: null,
    origin_conversation_id: null,
    origin_message_id: null,
  };
}

function groupEntryOf(seed: GroupSeed, now: number) {
  return {
    group: groupViewOf(seed, now),
    members: castOf(seed.memberIds, now),
    chats: seed.chats.map((chat) =>
      conversationOf({ ...chat, source: { type: "group", group_id: seed.id }, faces: seed.memberIds }, now),
    ),
  };
}

const memory = (
  id: string,
  text: string,
  patch: Partial<MemoryItemView>,
): MemoryItemView => ({
  id,
  short_id: id,
  text,
  category: null,
  origin: "model",
  pinned: false,
  temperature: "hot",
  observed_at: null,
  observed_time_precision: null,
  token_count: null,
  cycle_label: null,
  ...patch,
});

const emotions = (values: Partial<CompanionEmotionVector>): CompanionEmotionVector => ({
  warmth: null,
  trust: null,
  calm: null,
  vulnerability: null,
  longing: null,
  hurt: null,
  tension: null,
  irritation: null,
  affection_intensity: null,
  reassurance_need: null,
  ...values,
});

const entry = (
  id: string,
  ordinal: number,
  title: string,
  keywords: string[],
  content: string,
  patch: Partial<LorebookEntryView> = {},
  now = 0,
): LorebookEntryView => ({
  id,
  title,
  enabled: true,
  always_active: false,
  keywords,
  secondary_keywords: [],
  secondary_logic: "and_any",
  case_sensitive: false,
  keyword_mode: "literal",
  content,
  priority: 0,
  ordinal,
  revision: 1,
  created_at: now,
  updated_at: now,
  ...patch,
});

const storyTime = new Date(1024, 2, 14, 21, 40).getTime();

/** The bundled cast the 3.0 intro renders its real components with, as the shapes the backend sends. Time-dependent fields are relative to `now`. */
export function createShowcase(now: number) {
  const cassian = characterOf("cassian", now, { banner: asset("throne-room", throneRoom) });
  const mireille = characterOf("mireille", now, {
    card_style: "banner",
    gradient: { type: "custom", colors: ["#d8cba6", "#8a7b4e"], text_color: "#1a1a1a", text_secondary: "rgba(0, 0, 0, 0.62)" },
  });

  const session = conversationOf(
    {
      id: "cassian-gate",
      title: "The northern gate",
      agoMs: 2 * minute,
      preview: "Three days pass. The northern gate is repaired.",
      source: { type: "direct", character_id: "cassian" },
      faces: ["cassian"],
    },
    now,
  );

  const cassianParticipant: ParticipantView = {
    id: "p-cassian",
    role: "character",
    name: cassian.name,
    character_id: cassian.id,
    avatar: cassian.avatar,
  };

  const reply: TimelineMessage = {
    id: "m-reply",
    role: "assistant",
    author_participant_id: cassianParticipant.id,
    parts: [{ type: "text", text: "\"Come closer if you dare. I have an entire kingdom to rule, yet I find myself… *intrigued* by you.\"" }],
    reasoning: null,
    created_at: now - 2 * minute,
    candidate_index: null,
    candidate_count: 1,
    pinned: false,
    scene_image: null,
  };

  const facts = [
    "Hunter counts his breaths to stay calm. Samuel taught him.",
    "Cassian knows Hunter repaired the northern gate.",
    "Cassian wears a signet ring he turns when he's thinking.",
  ].map((text, index) => memory(`fact-${index + 1}`, text, {}));

  const time: TimeView = { enabled: true, mode: "ticking", anchor_at: storyTime, set_at: now, effective_at: storyTime };

  const chatSettings: ConversationSettingsView = {
    conversation_id: session.id,
    kind: "direct",
    title: session.title,
    revision: 1,
    settings_revision: 1,
    author_note: null,
    persona: { id: null, source: "app_default" },
    prompt: { id: null, source: "app_default" },
    lorebooks: { ids: [], source: "app_default" },
    model: { id: null, source: "character" },
    background: { asset: null, hidden: false, source: "character" },
    scene: { id: null, source: "app_default" },
    memory: { mode: "dynamic", source: "app_default" },
    disable_character_lorebooks: null,
    disable_character_notes: null,
    companion_mode: null,
    companion_personal_memories: null,
    companion_memory_write_back: null,
    speaker_selection: null,
    members: null,
    model_settings: null,
    time,
    companion: false,
  };

  const groups = groupSeeds.map((seed) => groupEntryOf(seed, now));
  const gateDraft = groups[0]!.group.draft;
  const relationship: CompanionRelationshipView = {
    closeness: 0.46,
    trust: 0.38,
    affection: 0.52,
    tension: 0.22,
    stability: 0.71,
    interaction_count: 240,
    last_interaction_at: now - 2 * day,
  };
  const relationshipDefaults: CompanionRelationshipDefaults = { closeness: 0.1, trust: 0.1, affection: 0.05, tension: 0 };

  const seraBefore: CreatorDraft = { name: "Sera", definition: "A cartographer who keeps to herself.", scenes: [], avatar: null };
  const seraAfter: CreatorDraft = {
    name: "Sera Vale",
    definition: "A cartographer who keeps to herself.\nShe draws maps of towns before they are founded, and is right more often than she likes.",
    scenes: [],
    avatar: asset("sera-face", seraFace),
  };
  const seraTurn: CreatorTurnView = {
    id: "t1",
    message: "A cartographer who maps places that don't exist yet. Quiet, a little stubborn. Make her a portrait too.",
    reply: "Here is a first take on her. She draws towns before anyone founds them, and is right more often than she likes. Want her younger, or with a different coat?",
    reasoning: "",
    status: "succeeded",
    job_id: "j1",
    base_proposal_id: "p1",
    result_proposal_id: "p2",
    regenerated: false,
    regenerable: false,
    references: [],
    images: [],
    tools: [
      { name: "set_name", status: "succeeded", image: null },
      { name: "write_definition", status: "succeeded", image: null },
      { name: "generate_image", status: "succeeded", image: { asset: asset("sera-portrait", seraPortrait), job_id: null, failed: false } },
    ],
    steps: [
      { kind: "text", text: "Here is a first take on her." },
      { kind: "tool", index: 0 },
      { kind: "tool", index: 1 },
      { kind: "tool", index: 2 },
      { kind: "text", text: "She draws towns before anyone founds them, and is right more often than she likes. Want her younger, or with a different coat?" },
    ],
  };

  return {
    cassian,
    mireille,
    session,
    reply,
    participants: [cassianParticipant],
    chat: { name: cassian.name, description: cassian.description ?? "", conversationId: session.id },
    groups,
    groupForm: { draft: gateDraft, background: null, lorebookIds: [], sceneSource: "none" as const, notes: [] },
    groupMembers: gateDraft.members.map((member) => ({
      member: { id: member.character_id, name: person(member.character_id).name, avatar: characterOf(member.character_id, now).avatar, description: person(member.character_id).description },
      model: member.model_profile_override === null ? null : (modelNames[member.model_profile_override] ?? null),
    })),
    chatMemories: [
      memory("c1", "Ruben is a retired carpenter who still carries his old notebook and talks problems through out loud.", { category: "character_trait", token_count: 29, cycle_label: "41-60" }),
      memory("c2", "You told him you are afraid of flying, and he promised to drive instead of booking a flight.", { category: "relationship", pinned: true, token_count: 24, cycle_label: "41-60" }),
    ],
    facts,
    chatSettings,
    time,
    companion: {
      memory: memory("k1", "You told Mireille about losing your job, and she stayed up talking with you until 3am.", {
        category: "milestone",
        pinned: true,
        token_count: 22,
        observed_at: now - 2 * day,
        cycle_label: "101-120",
      }),
      relationship,
      relationshipDefaults,
      felt: emotions({ warmth: 0.72, longing: 0.41, vulnerability: 0.33, calm: 0.3, affection_intensity: 0.26 }),
      expressed: emotions({ warmth: 0.58, calm: 0.44, affection_intensity: 0.3, vulnerability: 0.12 }),
    },
    lorebook: [
      entry("e1", 0, "The northern gate", ["northern gate", "the gate", "counterweight"], "The northern gate is the only land entrance to Welan's keep. Built to keep something in, not out; the mechanism is older than the castle around it. Rell maintains it and trusts no one else near the counterweight.", {}, now),
      entry("e2", 1, "House Welan", ["Welan", "the crown", "throne"], "Ruled by Cassian since the war. The crown passes by blood but is held by whoever the gate-keepers back. Ysolde has the better claim; Cassian has the gate.", { always_active: true }, now),
      entry("e3", 2, "The war", ["the war", "three months ago", "the siege"], "Ended three months ago with the sack of the eastern harbours. Nobody calls it a victory. Sera Vance ran the blockade twice and is still owed for both.", {}, now),
    ],
    creator: { turn: seraTurn, before: seraBefore, after: seraAfter },
  };
}

export type Showcase = ReturnType<typeof createShowcase>;
