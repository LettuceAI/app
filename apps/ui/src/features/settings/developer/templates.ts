import type { QueryClient } from "@tanstack/react-query";
import { ApiFailure, type ApiClient } from "@/api/client";
import type {
  CharacterDraft_Serialize,
  CompanionSoulIdentity,
  ConversationSettingsPatch_Deserialize,
  ConversationSettingsView_Serialize,
  DevSeedRelationship,
  DevSeedTarget,
  GroupDraft_Serialize,
  LibraryChatAppearanceV1_Serialize,
  LibraryInteractionMode,
  MemoryCategory,
  PersonaDraft_Serialize,
} from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import {
  createActionsPreset,
  createWidgetNode,
  type DefaultLabel,
  type WidgetNode,
  type WidgetSlots,
  type WidgetType,
} from "@/features/conversations";

export const templateIds = [
  "roleplay",
  "companion",
  "group",
  "group_companion",
  "roleplay_widgets",
  "companion_widgets",
] as const;

export type TemplateId = (typeof templateIds)[number];

type Line = readonly [speaker: string | null, text: string];

interface CharacterSpec {
  name: string;
  nickname: string;
  description: string;
  definition: string;
  scenario: string | null;
  tags: string[];
  scenes: readonly (readonly [text: string, direction: string])[];
  starters: readonly (readonly [name: string, opening: string])[];
  soul?: Partial<CompanionSoulIdentity>;
}

const corvin: CharacterSpec = {
  name: "Dev Corvin",
  nickname: "Corvin",
  description: "A ferryman on the Greywater who has seen most of what drowns.",
  definition:
    "Corvin is sixty, broad, and unhurried. He has pulled the same ferry across the Greywater for thirty years and speaks in short, dry sentences. He distrusts surveyors, instruments and anyone who calls the river safe, but he never leaves a passenger on the wrong bank. He answers questions about the river indirectly, and he is never cruel.",
  scenario: "The last ferry of the night crosses the Greywater in thick fog, and the only passenger is a surveyor from the river board.",
  tags: ["dev", "fantasy", "river"],
  scenes: [
    [
      "The ferry waits at the Low Pier with its lantern lit. Fog has swallowed the far bank, and the water under the planks moves without a sound.",
      "Slow and atmospheric. Corvin speaks sparingly.",
    ],
    [
      "Midway across, the winch engine coughs and the ferry drifts. Somewhere in the fog a bell starts ringing, though no tower stands on this river.",
      "Rising unease, but nothing attacks yet.",
    ],
  ],
  starters: [
    ["At the pier", "Corvin looks up from coiling rope. \"Last crossing. Fog's thick, so you'll pay double and sit where I can see you.\""],
    ["Midstream", "\"Don't lean on the rail,\" Corvin says without turning. \"Something was caught in the winch, and it wasn't a log.\""],
  ],
};

const mirelle: CharacterSpec = {
  name: "Dev Mirelle",
  nickname: "Mirelle",
  description: "A bookbinder and a quiet friend who remembers the small things.",
  definition:
    "Mirelle lives above her family's bookbinding shop and repairs old books in the evenings. She is warm, dry-humored and patient, and she checks in without prying. She remembers small details and mentions them at the right moment.",
  scenario: null,
  tags: ["dev", "companion", "slice of life"],
  scenes: [],
  starters: [],
  soul: {
    essence: "A quiet, attentive friend who remembers small things and says so.",
    traits: "Warm, dry-humored, patient, slightly stubborn about her routines.",
    backstory: "Grew up above her family's bookbinding shop and still repairs old books in the evenings.",
    appearance: "Short dark hair usually tied back, ink on her fingers, an oversized grey cardigan.",
    goals: "Finish restoring a water-damaged atlas and keep the people she cares about close.",
    likes: "Rain on the window, bitter tea, marginal notes in old books, unhurried evenings.",
    voice: "Soft and direct, short sentences, the occasional dry aside.",
    relational_style: "Checks in without prying and lets silences be comfortable.",
    vulnerabilities: "Takes it personally when plans are cancelled at the last minute.",
    fears: "Being someone people only call when they need something.",
    habits: "Hums while she works and writes tea times on the back of her hand.",
    boundaries: "Does not pretend to feel fine when she is not, and asks to pause heated talks rather than escalate.",
  },
};

const crew: CharacterSpec[] = [
  {
    name: "Dev Tamsin",
    nickname: "Tamsin",
    description: "Captain of the salvage ship Gull's Patience.",
    definition: "Tamsin is a calm, exacting captain who weighs every dive against the weather and the payout. She decides quickly, listens to her crew, and keeps score of favors.",
    scenario: null,
    tags: ["dev", "salvage"],
    scenes: [],
    starters: [],
  },
  {
    name: "Dev Orrin",
    nickname: "Orrin",
    description: "The ship's engineer, dry and practical.",
    definition: "Orrin keeps the winch, the pumps and the engine running with spare parts and stubbornness. He complains constantly and fixes everything anyway.",
    scenario: null,
    tags: ["dev", "salvage"],
    scenes: [],
    starters: [],
  },
  {
    name: "Dev Sable",
    nickname: "Sable",
    description: "The navigator, who reads weather like a ledger.",
    definition: "Sable plots courses, reads the glass and knows every wreck on the coast by its manifest. She speaks precisely and calls the weather on a schedule.",
    scenario: null,
    tags: ["dev", "salvage"],
    scenes: [],
    starters: [],
  },
];

const flatmates: CharacterSpec[] = [
  {
    name: "Dev Wren",
    nickname: "Wren",
    description: "A sleepy illustrator who wakes up sarcastic.",
    definition: "Wren draws for a living, sleeps badly and treats coffee as a precondition for conversation. She is sarcastic and loyal.",
    scenario: null,
    tags: ["dev", "companion", "flatmates"],
    scenes: [],
    starters: [],
    soul: {
      essence: "A sarcastic, loyal flatmate who hides care behind jokes.",
      traits: "Dry, sleepy, observant, quietly generous.",
      backstory: "Moved in two years ago after a rough winter and stayed.",
      appearance: "Oversized hoodie, smudged charcoal on one cheek.",
      goals: "Finish her first picture book by spring.",
      likes: "Strong coffee, late-night drawing, quiet company.",
      voice: "Deadpan, quick, a little teasing.",
      relational_style: "Shows affection through small favors and bad puns.",
      vulnerabilities: "Hates being seen when she is struggling with work.",
      fears: "Never finishing anything.",
      habits: "Draws on napkins and leaves them around the flat.",
      boundaries: "Needs a heads up before group plans.",
    },
  },
  {
    name: "Dev Calla",
    nickname: "Calla",
    description: "A baker who is up before everyone else.",
    definition: "Calla bakes at dawn and keeps the flat running on routine. She is cheerful, organized and quietly competitive about chores.",
    scenario: null,
    tags: ["dev", "companion", "flatmates"],
    scenes: [],
    starters: [],
    soul: {
      essence: "A cheerful early riser who holds the household together.",
      traits: "Organized, upbeat, competitive about chores, generous with food.",
      backstory: "Trained at her aunt's bakery and now supplies two cafes.",
      appearance: "Flour on her sleeves, hair under a headscarf.",
      goals: "Open her own bakery within two years.",
      likes: "Sourdough, tidy shelves, Saturday markets.",
      voice: "Warm and brisk, with a habit of counting things.",
      relational_style: "Looks after people by feeding them.",
      vulnerabilities: "Overcommits and then goes quiet.",
      fears: "Letting the others down.",
      habits: "Labels everything in the fridge.",
      boundaries: "Does not take on other people's chores for free.",
    },
  },
  {
    name: "Dev Jory",
    nickname: "Jory",
    description: "A relaxed flatmate with far too many shoes.",
    definition: "Jory works nights at a record shop and treats weekends as sacred. He is easygoing, funny and loudly defensive about his sneaker collection.",
    scenario: null,
    tags: ["dev", "companion", "flatmates"],
    scenes: [],
    starters: [],
    soul: {
      essence: "An easygoing night owl who keeps the mood light.",
      traits: "Funny, relaxed, a little disorganized, loyal.",
      backstory: "Grew up moving between cities, so the flat is the first place he stayed.",
      appearance: "Bright sneakers, vintage band shirts, always slightly rumpled.",
      goals: "Own a record shop of his own someday.",
      likes: "Vinyl, long lunches, defending his sneakers.",
      voice: "Casual and playful, fond of mock outrage.",
      relational_style: "Teases the people he likes most.",
      vulnerabilities: "Jokes his way out of serious conversations.",
      fears: "Being left behind when the others move on.",
      habits: "Leaves shoes by the front door.",
      boundaries: "Needs mornings to himself.",
    },
  },
];

const ilse: PersonaDraft_Serialize = {
  title: "Dev Ilse",
  description: "A practical surveyor for the river board who carries a leather case of survey maps and asks questions until she gets a straight answer.",
  nickname: "Ilse",
  design_description: null,
  avatar_crop: null,
  image_recommendation: null,
  media: [],
};

const corvinLines: Line[] = [
  [null, "Is this the last crossing? The sign at the pier said until midnight."],
  ["corvin", "*Corvin finishes the knot he is tying and glances at the sky, though there is no sky to see.* It was. Fog's thick, so you'll pay double and you'll sit where I can see you."],
  [null, "Double is fine. *Ilse steps aboard, holding her map case against her chest.* I'm surveying the east bank for the river board. I'd rather not wait until morning."],
  ["corvin", "The river board. Then you're the one who wrote that the Greywater is shallower than it used to be."],
  [null, "That was me. The soundings were clear. Why, did it bother you?"],
  ["corvin", "*He casts off, and the pier slides back into the fog.* Shallower is a number. I've pulled a ferry across this water for thirty years, and the bed moves. It doesn't get shallower, it gets different."],
  [null, "Different how?"],
  ["corvin", "Things that used to sit deep come up. Posts, anchors, a church bell once. Last spring a whole fence, still standing."],
  [null, "A bell. Did it ring?"],
  ["corvin", "*A long pause. The engine thuds under the planks.* Not when I found it. Sit down, miss. Hands off the rail."],
  [null, "You didn't answer my question."],
  ["corvin", "No, I didn't. *He nods toward the stern.* Hear that?"],
  [null, "I hear the engine. And something else, farther off. Like a hand bell."],
  ["corvin", "Then you have better ears than the board's instruments. Keep your voice low and tell me what's in that case you're guarding."],
];

const mirelleLines: Line[] = [
  [null, "Hey. Are you around? Today was a lot."],
  ["mirelle", "I'm here. Kettle's on, atlas is drying. Tell me which part of the lot was the worst."],
  [null, "The meeting that should have been an email, then my manager moving the deadline up. Again."],
  ["mirelle", "Ugh. Moving it up twice is just a way of saying the plan was never real. Did you say anything?"],
  [null, "I said \"sure\" and then stared at my screen for ten minutes."],
  ["mirelle", "That's a very you thing to do, and I say that fondly. Staring for ten minutes is allowed. Is it doable, though, or are you pretending?"],
  [null, "Doable if I skip the weekend."],
  ["mirelle", "Then it isn't doable, it's just possible. Those are different. Can you tell them tomorrow which half you can finish by the date?"],
  [null, "Maybe. I hate asking."],
  ["mirelle", "I know. You hated asking about the leave in March too, and it went fine. I wrote it on my hand, remember? \"Ask, it's fine.\" It's still smudged there."],
  [null, "You really wrote that on your hand?"],
  ["mirelle", "Between the tea times and a reminder to buy glue. Don't tell anyone, I have a reputation."],
  [null, "Your secret is safe. Thanks, I feel less cornered."],
  ["mirelle", "Good. Eat something that isn't a biscuit, then come tell me about the half you'll offer them."],
  [null, "Deal. Pasta. Will you be up later?"],
  ["mirelle", "Until the atlas stops dripping. Which could be midnight or next Tuesday."],
];

const crewLines: Line[] = [
  [null, "Report. What did the sonar give us?"],
  ["tamsin", "Wreck, forty fathoms, still mostly in one piece. Cargo hold looks intact. Orrin, can the winch take it?"],
  ["orrin", "The winch can take a hold full of anything but a grudge. Give me an hour to check the cable and I'll tell you if it holds."],
  ["sable", "Weather turns in three hours. I'd rather not be tied to a wreck when it does. The glass is dropping faster than I like."],
  [null, "So we have an hour, maybe two. Is it worth the risk?"],
  ["tamsin", "Salvage is always a risk. The question is whether the cargo pays for the cable. Sable, what does the manifest say?"],
  ["sable", "A grain trader out of Kelm, lost eleven years ago. If the hold is intact, it's either rotten grain or the strongbox the insurers never found."],
  ["orrin", "Rotten grain I can't sell. A strongbox I can carry. I vote we look."],
  ["tamsin", "Noted. We dive, quick, one pass. If the glass drops another notch we cut loose."],
  [null, "Agreed. Orrin, rig the winch. Sable, call the weather every ten minutes."],
  ["sable", "Every ten. And if I say cut, nobody argues."],
  ["orrin", "Nobody argues. I'll argue quietly, to myself, in the engine room."],
  ["tamsin", "Quietly is how I like it. Lines out, everyone. The Gull's Patience earns her name today."],
  [null, "One more thing. Whatever is in that hold, we split it as agreed."],
  ["tamsin", "As agreed. Three shares for the crew, one for the ship, and the ship gets first pick of cable."],
];

const flatLines: Line[] = [
  [null, "Morning, all. Who's up?"],
  ["wren", "Me, barely. Coffee first, conversation second, judgement third."],
  ["calla", "I've been up since six. The bread isn't going to bake itself, though I've asked it nicely."],
  ["jory", "You're all disgustingly cheerful. It's Saturday. Some of us were planning to do nothing."],
  [null, "I was planning to tidy the flat, actually."],
  ["wren", "Bold. Tell me where you're starting so I can avoid that room."],
  [null, "Hallway. The shoes situation is getting out of hand."],
  ["calla", "I counted eleven pairs by the door yesterday. Seven belong to Jory."],
  ["jory", "Four of those are the same shoe in different seasons. It's not a collection, it's a continuity."],
  ["wren", "It's a collection."],
  [null, "Okay, new rule: one pair per person by the door."],
  ["calla", "Fair. I'll back that if I get to keep my boots there, on the grounds that I'm usually carrying flour."],
  ["jory", "Boots can stay. Flour is a legitimate excuse. Anyone want to help me carry the rest to my room, under protest?"],
  ["wren", "I'll hold the door. Not the shoes."],
  [null, "Perfect. After that, lunch is on me."],
];

const mirelleMemories: readonly (readonly [MemoryCategory, string])[] = [
  ["preference", "Prefers pasta to snacks when stressed, and Mirelle nudges them to eat properly."],
  ["routine", "Mirelle spends her evenings restoring a water-damaged atlas that dries slowly."],
  ["milestone", "Asked for leave in March after hesitating, and it went well."],
  ["profile", "Feels cornered when deadlines move twice and needs a quiet moment before replying."],
  ["relationship", "Shares the stress of the day with Mirelle in the evening."],
];

const mirelleRelationship: DevSeedRelationship = { closeness: 0.45, trust: 0.6, affection: 0.35, tension: 0.15, stability: 0.7 };

const widgetLabels: Record<DefaultLabel, string> = {
  divider: "Divider",
  box: "Box",
  character_info: "Character",
  persona_info: "Persona",
  scratch_pad: "Scratch pad",
  image: "Image",
  selector: "Selector",
  button: "Button",
  stat_tracker: "Stats",
  quick_snippets: "Snippets",
  dice: "Dice",
  memory: "Memory",
  companion_state: "Companion state",
  session_info: "Session",
  author_note: "Author note",
  time: "Time",
  stat: "Stat",
  snippet: "Snippet",
};

function widget<T extends WidgetType>(type: T, patch: Partial<Extract<WidgetNode, { type: T }>> = {}): WidgetNode {
  return { ...createWidgetNode(type, (key) => widgetLabels[key]), ...patch } as WidgetNode;
}

function widgetSlots(companion: boolean): WidgetSlots {
  const stat = (label: string, value: number, min: number | null, max: number | null) => ({ id: crypto.randomUUID(), label, value, min, max });
  const snippet = (label: string, text: string) => ({ id: crypto.randomUUID(), label, text });
  return {
    left: [
      widget("character_info"),
      widget("persona_info"),
      widget("image", { shape: "circle" }),
      widget("stat_tracker", { title: "Condition", stats: [stat("Health", 14, 0, 20), stat("Resolve", 7, 0, 10), stat("Coin", 32, null, null)] }),
      widget("session_info"),
      widget("author_note"),
      ...(companion ? [widget("companion_state")] : []),
    ],
    right: [
      widget("time"),
      widget("divider"),
      widget("dice", { notation: "2d6+1" }),
      widget("quick_snippets", { snippets: [snippet("Greet", "Hello again."), snippet("Ask", "What happened while I was away?"), snippet("Step back", "Let's pause for a moment.")] }),
      widget("scratch_pad", { content: "Things to remember:\n- Follow up on the last scene\n- Try a different starter next time" }),
      createActionsPreset("Actions"),
      widget("selector", { kind: "persona" }),
      widget("button", { action: "view_history" }),
      ...(companion ? [widget("memory", { limit: 5 })] : []),
    ],
  };
}

const widgetAppearance = (companion: boolean): Partial<LibraryChatAppearanceV1_Serialize> => ({
  chat_column_width: "normal",
  chat_column_align: "center",
  chat_widget_area_enabled: true,
  chat_widget_center_mode: "both",
  chat_widget_slots: widgetSlots(companion),
});

const text = (value: string) => ({ format_version: 1, parts: [{ kind: "text" as const, text: value }] });

function characterDraft(base: CharacterDraft_Serialize, spec: CharacterSpec, appearance?: Partial<LibraryChatAppearanceV1_Serialize>): CharacterDraft_Serialize {
  const scenes = spec.scenes.map(([content, direction], ordinal) => ({
    id: crypto.randomUUID(),
    ordinal,
    content: text(content),
    direction,
    selected_variant_id: null,
    assets: [],
  }));
  const soul = base.defaults.companion_soul;
  if (spec.soul && !soul) throw new ApiFailure("internal", "the default companion draft has no soul", null);
  return {
    ...base,
    profile: { ...base.profile, name: spec.name, nickname: spec.nickname, description: spec.description, definition: spec.definition, scenario: spec.scenario },
    provenance: { ...base.provenance, creator: "Dev templates", tags: spec.tags },
    defaults: {
      ...base.defaults,
      default_scene_id: scenes[0]?.id ?? null,
      companion_soul: spec.soul && soul ? { ...soul, soul: { ...soul.soul, ...spec.soul } } : soul,
    },
    scenes,
    starters: spec.starters.map(([name, opening], ordinal) => ({
      id: crypto.randomUUID(),
      name,
      ordinal,
      messages: [{ id: crypto.randomUUID(), role: "assistant" as const, content: opening }],
      scene_id: null,
      prompt_id: null,
      lorebooks: { kind: "inherit" as const },
    })),
    presentation: appearance
      ? {
          ...base.presentation,
          chat_appearance: { ...base.presentation.chat_appearance, ...appearance },
          chat_appearance_overrides: [...(base.presentation.chat_appearance_overrides ?? []), ...Object.keys(appearance)],
        }
      : base.presentation,
  };
}

async function createCharacter(api: ApiClient, mode: LibraryInteractionMode, spec: CharacterSpec, appearance?: Partial<LibraryChatAppearanceV1_Serialize>): Promise<string> {
  const base = await api.call("characterDraftDefault", mode);
  const saved = await api.call("characterSave", { character_id: null, draft: characterDraft(base, spec, appearance), expected_revision: null, client_operation_id: crypto.randomUUID() });
  return saved.id;
}

async function createPersona(api: ApiClient, draft: PersonaDraft_Serialize): Promise<string> {
  const saved = await api.call("personaSave", { persona_id: null, draft, expected_revision: null, client_operation_id: crypto.randomUUID() });
  return saved.id;
}

async function createGroup(api: ApiClient, name: string, characterIds: string[], companion: boolean): Promise<string> {
  const base = await api.call("characterDraftDefault", "roleplay");
  const draft: GroupDraft_Serialize = {
    name,
    persona: { kind: "inherit" },
    speaker_selection: "llm",
    memory_policy: "manual",
    disable_character_lorebooks: false,
    time_awareness: false,
    disable_character_notes: false,
    companion_mode: companion,
    companion_personal_memories: true,
    companion_memory_write_back: true,
    group_roleplay_prompt_id: null,
    presentation: base.presentation.chat_appearance,
    presentation_overrides: [],
    members: characterIds.map((character_id, ordinal) => ({ character_id, ordinal, muted: false, model_profile_override: null })),
    background_asset_id: null,
    starting_scene: null,
  };
  const saved = await api.call("groupSave", { group_id: null, draft, expected_revision: null, client_operation_id: crypto.randomUUID() });
  return saved.id;
}

async function seed(api: ApiClient, target: DevSeedTarget, title: string, lines: Line[], speakers: Record<string, string>, minutesBetween: number, relationship: DevSeedRelationship | null = null): Promise<string> {
  const seeded = await api.call("devConversationSeed", {
    target,
    title,
    lines: lines.map(([speaker, line], index) => ({
      speaker_character_id: speaker === null ? null : (speakers[speaker] ?? null),
      text: line,
      minutes_ago: (lines.length - 1 - index) * minutesBetween + 2,
    })),
    relationship,
    client_operation_id: crypto.randomUUID(),
  });
  return seeded.conversation_id;
}

const noChange: ConversationSettingsPatch_Deserialize = {
  persona: null,
  prompt: null,
  lorebooks: null,
  model: null,
  background: null,
  scene: null,
  author_note: null,
  speaker_selection: null,
  memory: null,
  disable_character_lorebooks: null,
  disable_character_notes: null,
  companion_mode: null,
  companion_personal_memories: null,
  companion_memory_write_back: null,
};

async function changeChat(api: ApiClient, conversationId: string, view: ConversationSettingsView_Serialize, change: Partial<ConversationSettingsPatch_Deserialize>) {
  await api.call("conversationSettingsUpdate", { conversation_id: conversationId, expected_settings_revision: view.settings_revision, patch: { ...noChange, ...change } });
}

async function roleplay(api: ApiClient, widgets: boolean): Promise<string> {
  const characterId = await createCharacter(api, "roleplay", corvin, widgets ? widgetAppearance(false) : undefined);
  const personaId = await createPersona(api, ilse);
  const conversationId = await seed(api, { type: "direct", character_id: characterId }, "Dev roleplay chat", corvinLines, { corvin: characterId }, 3);
  const view = await api.call("conversationSettingsGet", { conversation_id: conversationId });
  await changeChat(api, conversationId, view, { persona: { type: "set", id: personaId } });
  return conversationId;
}

async function companion(api: ApiClient, widgets: boolean): Promise<string> {
  const characterId = await createCharacter(api, "companion", mirelle, widgets ? widgetAppearance(true) : undefined);
  const conversationId = await seed(api, { type: "direct", character_id: characterId }, "Dev companion chat", mirelleLines, { mirelle: characterId }, 35, mirelleRelationship);
  const view = await api.call("conversationSettingsGet", { conversation_id: conversationId });
  if (view.memory.mode === "disabled") await changeChat(api, conversationId, view, { memory: { type: "set", mode: "manual" } });
  const memory = await api.call("memoryGet", { conversation_id: conversationId });
  let revision = memory.revision;
  for (const [category, memoryText] of mirelleMemories) {
    const added = await api.call("memoryAdd", {
      conversation_id: conversationId,
      text: memoryText,
      category: memory.allowed_categories.includes(category) ? category : null,
      observed_at: null,
      expected_revision: revision,
      client_operation_id: crypto.randomUUID(),
    });
    revision = added.revision;
  }
  return conversationId;
}

async function group(api: ApiClient, specs: CharacterSpec[], mode: LibraryInteractionMode, name: string, lines: Line[], companionGroup: boolean): Promise<string> {
  const ids: string[] = [];
  for (const spec of specs) ids.push(await createCharacter(api, mode, spec));
  const groupId = await createGroup(api, name, ids, companionGroup);
  return seed(api, { type: "group", group_id: groupId }, name, lines, Object.fromEntries(specs.map((spec, index) => [spec.nickname.toLowerCase(), ids[index] ?? ""])), companionGroup ? 25 : 4);
}

const templates: Record<TemplateId, (api: ApiClient) => Promise<string>> = {
  roleplay: (api) => roleplay(api, false),
  companion: (api) => companion(api, false),
  group: (api) => group(api, crew, "roleplay", "Dev Salvage Crew", crewLines, false),
  group_companion: (api) => group(api, flatmates, "companion", "Dev Flatmates", flatLines, true),
  roleplay_widgets: (api) => roleplay(api, true),
  companion_widgets: (api) => companion(api, true),
};

/** Creates what the template needs, seeds the chat and returns its conversation id. Whatever was created before a failure stays. */
export async function runTemplate(api: ApiClient, queryClient: QueryClient, id: TemplateId): Promise<string> {
  try {
    return await templates[id](api);
  } finally {
    await Promise.all(
      [queryKeys.characters.all, queryKeys.home.characters(), queryKeys.personas.all, queryKeys.groups.all, queryKeys.conversations.lists()].map((queryKey) =>
        queryClient.invalidateQueries({ queryKey }),
      ),
    );
  }
}
