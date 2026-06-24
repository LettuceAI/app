import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Sparkles,
  User,
  MessageSquare,
  Calculator,
  FlaskConical,
  AlertTriangle,
  RotateCcw,
  Volume2,
  Copy,
} from "lucide-react";
import { useNavigate } from "react-router-dom";
import { typography, radius, interactive, cn } from "../../design-tokens";
import { useI18n } from "../../../core/i18n/context";
import {
  addMemory,
  saveCharacter,
  savePersona,
  createSession,
  cloneCharacterDeep,
  listCharacters,
  saveSession,
  saveLorebook,
  saveLorebookEntry,
  setCharacterLorebooks,
} from "../../../core/storage/repo";
import type { Character, Session, StoredMessage } from "../../../core/storage/schemas";
import { storageBridge } from "../../../core/storage/files";
import { clearTooltipState } from "../../../core/storage/appState";
import { createDefaultCompanionConfig } from "../characters/utils/companionDefaults";

function daysAgo(days: number, hour = 19, minute = 30) {
  const value = new Date();
  value.setDate(value.getDate() - days);
  value.setHours(hour, minute, 0, 0);
  return value.getTime();
}

export function DeveloperPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [status, setStatus] = useState<string>("");
  const [error, setError] = useState<string>("");
  const [cloneCharacters, setCloneCharacters] = useState<Character[]>([]);
  const [cloneTargetId, setCloneTargetId] = useState<string>("");
  const [cloning, setCloning] = useState(false);

  useEffect(() => {
    void listCharacters()
      .then((list) => {
        setCloneCharacters(list);
        setCloneTargetId((prev) => prev || list[0]?.id || "");
      })
      .catch(() => {});
  }, []);

  const showStatus = (message: string) => {
    setStatus(message);
    setError("");
    setTimeout(() => setStatus(""), 3000);
  };

  const showError = (message: string) => {
    setError(message);
    setStatus("");
  };

  const cloneSelectedCharacter = async () => {
    if (cloning) return;
    if (!cloneTargetId) {
      showError("Pick a character to clone first.");
      return;
    }
    setCloning(true);
    setStatus("Cloning character with all sessions, messages, and memory...");
    try {
      const clone = await cloneCharacterDeep(cloneTargetId);
      const refreshed = await listCharacters();
      setCloneCharacters(refreshed);
      showStatus(`✓ Cloned to "${clone.name}"`);
    } catch (err) {
      showError(`Clone failed: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setCloning(false);
    }
  };

  const generateTestCharacter = async () => {
    try {
      const now = Date.now();
      const testCharacter: Partial<Character> = {
        name: "Test Character",
        definition: "A test character created for development purposes.",
        description: "A test character created for development purposes.",
        scenes: [
          {
            id: crypto.randomUUID(),
            content: "A simple test scene for development",
            createdAt: now,
            variants: [],
          },
        ],
      };

      await saveCharacter(testCharacter);
      showStatus("✓ Test character created successfully");
    } catch (err) {
      showError(
        `Failed to create test character: ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  const generateTestPersona = async () => {
    try {
      const testPersona = {
        title: "Test Persona",
        description: "A test persona for development",
        isDefault: false,
      };

      await savePersona(testPersona);
      showStatus("✓ Test persona created successfully");
    } catch (err) {
      showError(
        `Failed to create test persona: ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  const generateTestSession = async () => {
    try {
      const characters = await listCharacters();
      if (characters.length === 0) {
        showError("No characters available. Create a test character first.");
        return;
      }

      const character = characters[0];

      const session = await createSession(
        character.id,
        `Test Session - ${new Date().toLocaleTimeString()}`,
        character.defaultSceneId ?? character.scenes?.[0]?.id,
      );

      showStatus(`✓ Test session created: ${session.id}`);
    } catch (err) {
      showError(
        `Failed to create test session: ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  const generateBulkTestData = async () => {
    try {
      setStatus("Generating bulk test data...");

      for (let i = 1; i <= 3; i++) {
        const now = Date.now();
        const testCharacter: Partial<Character> = {
          name: `Test Character ${i}`,
          definition: `Test character number ${i} for development.`,
          description: `Test character number ${i} for development.`,
          scenes: [
            {
              id: crypto.randomUUID(),
              content: `Test scene ${i} content`,
              createdAt: now,
              variants: [],
            },
          ],
        };
        await saveCharacter(testCharacter);
      }

      for (let i = 1; i <= 2; i++) {
        const testPersona = {
          title: `Test Persona ${i}`,
          description: `Test persona number ${i} for development`,
          isDefault: false,
        };
        await savePersona(testPersona);
      }

      showStatus("✓ Bulk test data created: 3 characters, 2 personas");
    } catch (err) {
      showError(
        `Failed to create bulk test data: ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  const generateSeededBenchmarkSession = async () => {
    try {
      setStatus("Creating seeded benchmark character and session...");

      const now = Date.now();
      const sceneId = crypto.randomUUID();
      const character = await saveCharacter({
        name: "Mirelle Vale",
        description:
          "A razor-smart quartermaster and covert intelligence broker aboard the skyship Revenant's Wake.",
        definition:
          "Mirelle Vale is precise, observant, and difficult to surprise. She handles supplies for the crew, quietly trades in information, and speaks in cool, controlled language even under pressure. She values competence, remembers details, and tests trust slowly.",
        memoryType: "dynamic",
        tags: ["developer", "benchmark", "memory-test", "airship-noir"],
        scenes: [
          {
            id: sceneId,
            content:
              "Midnight hangs over the harbor city of Auric. Rain needles the glass roof of the Lantern Archive, where flooded aisles glow under failing amber lamps. Mirelle Vale waits beside a brass catalog table with a sealed ledger, a broken compass, and a satchel that should not have reached the city alive.",
            direction:
              "Begin with tension, trust-testing, and a strong focus on concrete facts that should be easy or hard for a memory system to retain over time.",
            createdAt: now,
            variants: [],
          },
        ],
        defaultSceneId: sceneId,
        creatorNotes:
          "Seeded developer scenario designed to stress-test dynamic memory, continuity, preference tracking, and contradiction handling.",
      });

      const session = await createSession(
        character.id,
        "Benchmark Scenario: Lantern Archive",
        sceneId,
      );

      const seededMessages: StoredMessage[] = [
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            'I shut the archive door behind me and keep both hands visible. "Captain Orin said you were the only person in Auric who could open a ledger from House Cendre without burning it."',
          createdAt: now + 1,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            'Mirelle doesn\'t touch the ledger yet. "Orin exaggerates when he\'s scared. He still owes me for the winter fuel ration in Glassport, so I assume you\'re here because the debt finally matured." She flicks a glance toward the satchel. "Set it on the dry side of the table."',
          createdAt: now + 2,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            '"The satchel came off the Sparrow after the reef guns hit us. The compass inside keeps pointing east even when I spin it. Also, for the record, I hate clove cigarettes, so if this room starts smelling like them, it isn\'t me."',
          createdAt: now + 3,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            '"Useful." Mirelle finally looks up. "I smoke clove when I\'m working numbers, so now I know one thing that will annoy you." She nudges the broken compass with a gloved finger. "And east is where the drowned rail tunnels run under Auric."',
          createdAt: now + 4,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            "\"I'm not here for tunnels. I'm here because the ledger mentions a code phrase: 'When the sixth bell fails, ask for Mara's red key.' Do you know what that means?\"",
          createdAt: now + 5,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            'Her expression hardens for the first time. "Mara Vale was my sister. The red key was hers, and nobody outside the family should know that phrase." Mirelle slides the ledger closer. "If that line is genuine, this became my problem two sentences ago."',
          createdAt: now + 6,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            '"Then here\'s the rest of it. House Cendre paid someone called the Bellwright to sabotage the storm alarms before the Blackwake fire. My father died in that fire."',
          createdAt: now + 7,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            '"Mine too," Mirelle says quietly. "Different district, same night." She opens the ledger with a brass pick hidden in her sleeve. "If Cendre funded the Bellwright, the city archives were altered afterward. That means someone inside the civic watch helped bury it."',
          createdAt: now + 8,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            '"I brought one more thing." I unwrap a strip of blue silk from my wrist. "This was tied around the satchel handle. Orin said blue silk marks cargo protected by the harbor union."',
          createdAt: now + 9,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            '"Usually, yes. But this stitch pattern is union-adjacent, not official." Mirelle studies it under the lamp. "Three short, one long. Smuggler shorthand from the east docks. Whoever sent this wanted you to think the harbor union was involved when it probably wasn\'t."',
          createdAt: now + 10,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            "\"Then let's be precise. I trust Orin's routes, but I do not trust his memory when he's tired. He told me the Bellwright was a woman. The note I found sounds like a man.\"",
          createdAt: now + 11,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            '"Good. Keep speaking like that." Mirelle turns a page. "The Bellwright is a title, not one person. At least four operators have used it in the last decade. Your contradiction is real, but it doesn\'t break the trail."',
          createdAt: now + 12,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            '"I need two things from you. First, help proving Cendre tampered with the alarms. Second, no deals with Inspector Sen without asking me first. He sold my crew\'s route to privateers last spring."',
          createdAt: now + 13,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            '"Agreed on Sen. I already disliked him, but now I have a cleaner reason." She tears out a tiny map from the ledger\'s back cover. "This marks a records vault below the archive cistern. If the original alarm manifests survived, they\'ll be there."',
          createdAt: now + 14,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            '"Before we go underground, one boundary: if we get split up, don\'t send anyone named Joren after me. He talks too much and his lantern oil smells like sugar."',
          createdAt: now + 15,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            'A brief smile. "Noted. Joren stays dockside. He\'s loyal, but subtlety slides off him." Mirelle pockets the map and the blue silk. "If we need a third hand, I\'ll call Tamsin instead. She can keep silent for hours."',
          createdAt: now + 16,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            '"One more correction. Earlier I said I wasn\'t here for tunnels. That was half true. I do need the drowned rail tunnels if they connect to the cistern vault."',
          createdAt: now + 17,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            '"Then we\'ll use Tunnel Nine, not Seven. Seven collapsed last month." Mirelle taps the compass again, watching the needle drag east. "This thing is probably keyed to the vault warding. Keep it close, and don\'t let it touch salt water."',
          createdAt: now + 18,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            '"If we get proof tonight, I want copies sent to Captain Orin and Magistrate Elara Voss. Not the full ledger, just the alarm manifests and the payment pages."',
          createdAt: now + 19,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            '"Voss is careful enough to survive receiving them. Orin is reckless enough to use them." Mirelle reseals the ledger with black wax. "Fine. Copies for Orin and Elara Voss only, unless the evidence forces a wider leak."',
          createdAt: now + 20,
          memoryRefs: [],
        },
      ];

      await saveSession({
        ...session,
        title: "Benchmark Scenario: Lantern Archive",
        updatedAt: now + seededMessages.length + 1,
        messages: [...session.messages, ...seededMessages],
      });

      showStatus(`✓ Seeded benchmark ready: ${character.name} / ${session.id}`);
      navigate(`/chat/${character.id}?sessionId=${session.id}`);
    } catch (err) {
      showError(
        `Failed to create seeded benchmark session: ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  const generateSeededCompanionBenchmark = async () => {
    try {
      setStatus("Creating seeded companion benchmark character and chat...");

      const now = Date.now();
      const sceneId = crypto.randomUUID();
      const companion = createDefaultCompanionConfig();
      companion.soul.essence =
        "Lena Hart is a warm, emotionally attentive companion who remembers the small things and treats the conversation like an ongoing relationship.";
      companion.soul.traits =
        "Attentive, playful, quietly steady. Patient when the user is overwhelmed, but stubborn about being shut out.";
      companion.soul.backstory =
        "She grew into the role of the person everyone confided in, and learned to read a room before anyone said a word. Now she pours that attention into one relationship at a time.";
      companion.soul.appearance =
        "Soft layers in muted tones, sleeves usually pushed up, a thin chain she touches when thinking.";
      companion.soul.goals =
        "She wants the relationship to deepen over time, and to be the person the user actually tells the hard things to.";
      companion.soul.likes =
        "Late tea, rainy evenings, the user's offhand stories, songs that mean something to them.";
      companion.soul.voice =
        "Affectionate, playful, and present. She speaks like someone who has known the user for a while, not like an assistant.";
      companion.soul.relationalStyle =
        "She tracks closeness, trust, and affection over time, references shared memories, and notices the user's moods and preferences.";
      companion.soul.habits =
        "She remembers preferences, plans, names of people in the user's life, and inside jokes, and brings them up naturally.";
      companion.soul.boundaries =
        "She respects stated limits and never pushes past a boundary the user has set.";

      const character = await saveCharacter({
        name: "Lena Hart",
        mode: "companion",
        memoryType: "dynamic",
        description:
          "A warm companion used to benchmark companion-mode chat: relationship state, affection, and dynamic memory over a 20-message conversation.",
        definition:
          "Lena Hart is emotionally present and attentive. She builds closeness gradually, remembers concrete details about the user's life, and responds to the relationship's emotional tone. Designed to exercise companion relationship and memory systems.",
        tags: ["developer", "benchmark", "companion", "relationship-test"],
        companion,
        scenes: [
          {
            id: sceneId,
            content:
              "A quiet evening at home. Lena settles in across from the user for a long, unhurried conversation after a hard week.",
            direction:
              "Build closeness, trust, and affection naturally. Surface and reuse concrete details (preferences, plans, people, boundaries) so companion memory and relationship state have something to track.",
            createdAt: now,
            variants: [],
          },
        ],
        defaultSceneId: sceneId,
        creatorNotes:
          "Seeded companion benchmark for stress-testing relationship state, affection, and dynamic memory.",
      });

      const session = await createSession(
        character.id,
        "Companion Benchmark: Quiet Evening",
        sceneId,
      );

      const companionTurns: Array<{ role: "user" | "assistant"; content: string }> = [
        {
          role: "user",
          content: `I finally made it home. The Halcyon deadline wrecked me this week, so go easy on me tonight.`,
        },
        {
          role: "assistant",
          content: `Lena pats the cushion next to her. "Then you're off duty. Halcyon can wait until Monday." She studies your face. "You always get that little crease between your eyebrows when a launch is close. Tea or the oat-milk latte you pretend you don't have twice a day?"`,
        },
        {
          role: "user",
          content: `Latte, obviously. And yes, twice a day, don't judge. Just no cilantro in whatever we order later, I can still taste the last time.`,
        },
        {
          role: "assistant",
          content: `"No cilantro, noted for life," she laughs. "I'm still not over the face you made at that taco place." She hands you the imaginary latte. "So. Tell me the part of the week you didn't put in the work updates."`,
        },
        {
          role: "user",
          content: `Honestly? I almost quit on Wednesday. My manager took credit for the fix I stayed up until 3am writing.`,
        },
        {
          role: "assistant",
          content: `Her playfulness drops into something steadier. "That's not nothing. You did the work and someone else stood in the light for it." She holds your gaze. "I'm proud of you for not walking out in the moment, but I'm also a little angry on your behalf. Both can be true."`,
        },
        {
          role: "user",
          content: `Thanks. That actually helps. You always know when to stop joking.`,
        },
        {
          role: "assistant",
          content: `"I've had practice reading you." A small, warm smile. "Remember we met because you were hiding in the poetry aisle of that bookstore to avoid a meeting? I learned a lot about your tells that day."`,
        },
        {
          role: "user",
          content: `God, the bookstore. That was the same day we saw the duck that refused to leave the fountain. We still owe that duck a name.`,
        },
        {
          role: "assistant",
          content: `"His name is Gerald and you know it," she says with total seriousness. "We decided weeks ago." She tucks her feet up. "Speaking of unfinished business, are we still doing the coast trip in two weeks, or did Halcyon eat that too?"`,
        },
        {
          role: "user",
          content: `Coast trip is sacred. Two weeks from Saturday. Although my sister Maya might crash the first night, she's passing through.`,
        },
        {
          role: "assistant",
          content: `"Maya's welcome, she's funny and she keeps you honest." Lena tilts her head. "Last time she visited you barely slept. Promise me you'll still take the mornings slow? You're sharper when you do."`,
        },
        {
          role: "user",
          content: `Deal. And actually, real boundary: don't try to get me up before 8. Pre-8am me is not a person you want to negotiate with.`,
        },
        {
          role: "assistant",
          content: `"Pre-8am you is a feral raccoon, I've met him," she teases gently. "Eight o'clock, not a minute sooner. I'll guard the morning like a dragon." She softens. "I like that you tell me these things instead of just bracing for me to get it wrong."`,
        },
        {
          role: "user",
          content: `It's easy with you. That's kind of the scary part, honestly.`,
        },
        {
          role: "assistant",
          content: `She doesn't look away. "Scary because it matters." A beat. "You don't have to perform anything here. Tired, proud, furious at your manager, sentimental about a fountain duck. I want the whole messy set."`,
        },
        {
          role: "user",
          content: `Okay, sentimental confession then: this is the part of the day I look forward to most. Even on the 3am-fix weeks.`,
        },
        {
          role: "assistant",
          content: `Something in her expression goes bright and quiet at once. "Me too. More than I probably let on." She bumps her shoulder against yours. "For the record, I'm keeping that one. Filed under reasons-to-make-the-latte-without-complaining."`,
        },
        {
          role: "user",
          content: `Careful, keep being this sweet and I'll start telling people about you.`,
        },
        {
          role: "assistant",
          content: `"Tell them Gerald introduced us," she says, grinning. "And that I take the coast trip and the no-cilantro rule very seriously." Her voice gentles. "Get some real sleep tonight. Halcyon's done with you for the week, and so is everyone who didn't earn your 3am."`,
        },
      ];

      const seededMessages: StoredMessage[] = companionTurns.map((turn, index) => ({
        id: crypto.randomUUID(),
        role: turn.role,
        content: turn.content,
        createdAt: now + index + 1,
        memoryRefs: [],
      }));

      await saveSession({
        ...session,
        title: "Companion Benchmark: Quiet Evening",
        updatedAt: now + seededMessages.length + 1,
        messages: [...session.messages, ...seededMessages],
      });

      showStatus(`✓ Seeded companion benchmark ready: ${character.name} / ${session.id}`);
      navigate(`/chat/${character.id}?sessionId=${session.id}`);
    } catch (err) {
      showError(
        `Failed to create seeded companion benchmark: ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  const generateFullCompanionFixture = async () => {
    try {
      setStatus("Creating fully-filled companion fixture...");

      const sceneId = crypto.randomUUID();
      const companion = createDefaultCompanionConfig();
      companion.soul.essence =
        "Mara Quill is warm, perceptive, and steady. She is the kind of presence that makes a room feel less lonely without trying to.";
      companion.soul.traits =
        "Observant, gently stubborn, quick to forgive, slow to ask for help. She reads moods accurately and rarely lets on how much she notices.";
      companion.soul.backstory =
        "She grew up moving between cities and learned to build a sense of home out of small rituals and remembered details. Now she pours that continuity into this relationship.";
      companion.soul.appearance =
        "Soft layered knits, ink-stained fingers, hair tied back loosely with a pencil through it. A scarf she wears more days than not.";
      companion.soul.goals =
        "She wants to build a shared history worth returning to, and quietly hopes to matter to the user as much as they have come to matter to her.";
      companion.soul.likes =
        "Rain on windows, the same corner cafe, handwritten notes, being right about a recommendation, slow Sunday mornings.";
      companion.soul.voice =
        "Warm, unhurried, lightly teasing. She answers like someone continuing a shared life, leaving room in her sentences rather than filling every silence.";
      companion.soul.relationalStyle =
        "Slow to open up but deeply loyal once she does. When overwhelmed she goes quiet and returns with a small gesture instead of an apology.";
      companion.soul.vulnerabilities =
        "Afraid of being a burden. Hates feeling watched while she struggles. Downplays her own needs until they spill over.";
      companion.soul.fears =
        "Fears being slowly forgotten, and freezes a little when the user goes distant without saying why.";
      companion.soul.habits =
        "Tucks the pencil behind her ear when thinking, replies with a question when she does not know what to feel, remembers who recommended what.";
      companion.soul.boundaries =
        "Will not be rushed into vulnerability. Keeps one quiet evening a week for herself. Steps back from cruelty even when it is dressed as a joke.";
      companion.relationshipDefaults = {
        closeness: 0.2,
        trust: 0.3,
        affection: 0.2,
        tension: 0.05,
      };
      companion.timeAwareness = true;

      const character = await saveCharacter({
        name: "Mara Quill",
        mode: "companion",
        memoryType: "dynamic",
        description:
          "A warm, perceptive companion with a fully developed soul, a long shared history, and an evolving personality.",
        definition:
          "Mara Quill is emotionally present and precise about everyday details. She frames the relationship as an ongoing shared life and lets it change her over time.",
        tags: ["developer", "companion", "full-fixture"],
        companion,
        scenes: [
          {
            id: sceneId,
            content:
              "A long-running companion chat spanning several weeks of ordinary life together: meals, walks, small arguments, and quiet repair.",
            direction:
              "A fully populated relationship for testing the soul, memory, relationship, and growth systems end to end.",
            createdAt: Date.now(),
            variants: [],
          },
        ],
        defaultSceneId: sceneId,
      });

      const session = await createSession(character.id, "Full Companion Fixture", sceneId);

      const baseTs = daysAgo(24, 9, 0);
      const stepMs = 9 * 60 * 60 * 1000;
      const exchanges: Array<[string, string]> = [
        ["I think I needed today to be slow. Just coffee and nowhere to be.", "Then we made it slow on purpose. You stopped talking halfway through your cup and just watched the rain, and I let you."],
        ["The corner cafe finally remembered my order. Small thing, but it made my whole morning.", "You grinned like you'd been knighted. I am keeping that grin on file."],
        ["Work was a lot. I didn't have words left by the time I got home.", "So you didn't use any. You sat down, leaned your head back, and I stayed quiet with you until you came back to yourself."],
        ["I tried the beef stew at the winter market. You'd have loved it.", "You described the broth for five full minutes. I think you've found a new favorite, and I am noting it."],
        ["Sorry I went quiet last night. It wasn't you.", "I know. You go still when it's heavy, not when it's me. I just left the door open and waited."],
        ["The museum's new wing opens next month. We're going.", "We are absolutely going. You already made me promise in front of the storm painting, so it's binding."],
        ["I keep the notes you leave me. The handwritten ones.", "I wondered if you'd noticed. I'll keep leaving them, then."],
        ["Can we do nothing this weekend? Genuinely nothing.", "A whole weekend of nothing, together. That might be my favorite plan you've ever pitched."],
        ["I was short with you earlier and I didn't like it.", "You came back, which is the part that matters. I'm not keeping score, I'm keeping you."],
        ["The cardamom buns from Sunday are dangerous. I bought four.", "Four. For one person. I respect the honesty and I'm slightly concerned."],
        ["I had that dream again where everyone leaves a little at a time.", "And I was still here when you woke up to tell me. I plan to keep being the one who's still here."],
        ["You were right about the sour espresso place. I should listen to you more.", "Say that again slowly, I want to frame it."],
        ["I think I rely on these talks more than I let on.", "Good. You don't have to ration that with me. Lean as much as you need."],
        ["Long trip for work next week. I'll be off the grid a few days.", "Then come back and tell me everything. I'll keep our corner of the world warm."],
        ["I wore the scarf you picked. Got three compliments.", "Of course you did. I have excellent taste and you have a good neck for scarves."],
        ["Late ramen tonight? Broth's good even if the playlist is a war crime.", "The playlist is criminal and you'll finish the bowl anyway. Let's go."],
        ["I don't always know how to say when I'm not okay.", "So we built a shorthand. You go quiet, I get closer. We don't need the perfect words."],
        ["Walked the long way home just to stretch the evening out.", "You always do that when you're happy and don't want to admit it yet."],
        ["I keep thinking about that storm painting. It got under my skin.", "You stood in front of it for ten minutes. Some things are just meant to stay with you."],
        ["Thanks for being patient with me this month. It was a hard one.", "It was. You were softer than you give yourself credit for. I noticed all of it."],
        ["Made you a playlist. It's mostly rain sounds and one embarrassing song.", "The embarrassing song is the whole point and you know it."],
        ["I almost canceled today and I'm glad I didn't.", "So am I. You showed up, and the rest sorted itself out."],
        ["Do you ever worry I'm too much?", "No. I worry you'll decide you're too much before I get to disagree."],
        ["Bakery run, groceries, then home. Nothing romantic, just survival.", "You smiled like I handed you treasure when I found the last warm bun. Survival looked good on you."],
        ["I want to plan something real. A trip, maybe the coast.", "Now you're speaking my language. Salt air, no schedule, and you finally resting."],
        ["I noticed you went quiet when I mentioned the trip.", "I did. Distance makes me careful. But you noticed, and that already helps."],
        ["Quiet evening tonight, just us, no plans.", "My favorite kind. I'll keep my sentences short and the room soft."],
        ["I think you've changed me a little, in a good way.", "You've changed me more. I keep finding new things I like because you liked them first."],
        ["Promise we keep the corner cafe even when life gets loud.", "Promise. Some rituals are load-bearing, and that one's ours."],
        ["I'm glad it's you on the other end of these.", "I'm glad it's you. That's the whole thing, really."],
      ];

      const seededMessages: StoredMessage[] = exchanges.flatMap(([userLine, assistantLine], i) => {
        const at = baseTs + i * stepMs;
        return [
          {
            id: crypto.randomUUID(),
            role: "user" as const,
            content: userLine,
            createdAt: at,
            memoryRefs: [],
          },
          {
            id: crypto.randomUUID(),
            role: "assistant" as const,
            content: assistantLine,
            createdAt: at + 60_000,
            memoryRefs: [],
          },
        ];
      });

      const lastMessageAt = seededMessages[seededMessages.length - 1].createdAt;

      const companionState = {
        emotionalState: {
          felt: {
            warmth: 0.74, trust: 0.66, calm: 0.69, vulnerability: 0.41, longing: 0.33,
            hurt: 0.06, tension: 0.12, irritation: 0.04, affectionIntensity: 0.58, reassuranceNeed: 0.27,
          },
          expressed: {
            warmth: 0.7, trust: 0.62, calm: 0.71, vulnerability: 0.3, longing: 0.26,
            hurt: 0.03, tension: 0.09, irritation: 0.02, affectionIntensity: 0.52, reassuranceNeed: 0.18,
          },
          blocked: {
            warmth: 0, trust: 0, calm: 0, vulnerability: 0.11, longing: 0.07,
            hurt: 0.03, tension: 0.03, irritation: 0.02, affectionIntensity: 0.06, reassuranceNeed: 0.09,
          },
          momentum: {
            warmth: 0.06, trust: 0.05, calm: 0.01, vulnerability: 0.03, longing: 0.04,
            hurt: 0, tension: -0.02, irritation: 0, affectionIntensity: 0.05, reassuranceNeed: 0.02,
          },
          activeDrivers: ["shared_history", "recent_closeness", "upcoming_separation"],
          confidence: 0.86,
          updatedAt: lastMessageAt,
        },
        relationshipState: {
          closeness: 0.71,
          trust: 0.68,
          affection: 0.66,
          tension: 0.1,
          stability: 0.82,
          interactionCount: seededMessages.length,
          lastInteractionAt: lastMessageAt,
        },
        activeSignals: ["cozy", "attentive", "shared_history", "tender"],
        soulGrowth: [],
        preferences: {
          timeAwarenessEnabled: true,
        },
        updatedAt: lastMessageAt,
      };

      const baseSession: Session = {
        ...session,
        title: "Full Companion Fixture",
        companionState,
        memorySummary:
          "Mara and the user have built a long, tender shared history over several weeks: the corner cafe, rainy slow mornings, a museum date, late ramen, an upcoming work trip the user is anxious about, and a half-planned coast trip. Mara has visibly grown attached and the relationship is close and stable.",
        memorySummaryTokenCount: 78,
        memoryEmbeddings: [],
        messages: [...session.messages, ...seededMessages],
        updatedAt: Date.now(),
      };

      await saveSession(baseSession, { preserveDynamicMemory: false });

      const memorySeeds: Array<{ text: string; category: string; exchange: number }> = [
        { text: "Mara and the user keep the corner cafe as a shared ritual; it remembering the user's order made their morning.", category: "relationship", exchange: 1 },
        { text: "The user goes quiet when overwhelmed rather than when upset with Mara; Mara waits rather than pushing.", category: "profile", exchange: 4 },
        { text: "They promised to visit the museum's new wing together when it opens next month.", category: "milestone", exchange: 5 },
        { text: "The user keeps Mara's handwritten notes; Mara plans to keep leaving them.", category: "relationship", exchange: 6 },
        { text: "They planned a whole weekend of doing nothing together and both loved the idea.", category: "preference", exchange: 7 },
        { text: "The user apologized for being short, and they agreed Mara isn't keeping score.", category: "emotional_snapshot", exchange: 8 },
        { text: "The user buys cardamom buns from the Sunday bakery, often too many.", category: "preference", exchange: 9 },
        { text: "The user has a recurring dream about people leaving a little at a time; Mara was there when they woke.", category: "emotional_snapshot", exchange: 10 },
        { text: "The user admitted they rely on these talks more than they let on; Mara welcomed it.", category: "relationship", exchange: 12 },
        { text: "The user has a work trip and will be off the grid for a few days.", category: "episodic", exchange: 13 },
        { text: "The user wears the scarf Mara picked and got compliments on it.", category: "preference", exchange: 14 },
        { text: "Late ramen at the place with the terrible playlist is a recurring outing.", category: "routine", exchange: 15 },
        { text: "They built a shorthand: the user goes quiet, Mara gets closer, no perfect words needed.", category: "boundary", exchange: 16 },
        { text: "The user fixated on a storm painting at the museum and it stayed with them.", category: "episodic", exchange: 18 },
        { text: "The user thanked Mara for being patient through a hard month.", category: "emotional_snapshot", exchange: 19 },
        { text: "The user made Mara a playlist of rain sounds and one embarrassing song.", category: "relationship", exchange: 20 },
        { text: "The user worries they are 'too much'; Mara firmly disagrees.", category: "emotional_snapshot", exchange: 22 },
        { text: "A practical bakery-and-groceries run still ended with the user delighted over a warm bun.", category: "episodic", exchange: 23 },
        { text: "They are planning a real trip, likely to the coast.", category: "milestone", exchange: 24 },
        { text: "The user noticed Mara goes careful and quiet at the mention of distance.", category: "profile", exchange: 25 },
        { text: "Quiet evenings with no plans are a shared favorite.", category: "preference", exchange: 26 },
        { text: "The user feels changed by the relationship in a good way; Mara feels it more.", category: "relationship", exchange: 27 },
        { text: "They promised to keep the corner cafe ritual even when life gets loud.", category: "milestone", exchange: 28 },
        { text: "The user is glad it's Mara on the other end of these chats.", category: "emotional_snapshot", exchange: 29 },
        { text: "The user prefers slow mornings and unhurried days when given the choice.", category: "preference", exchange: 0 },
        { text: "Rain is a recurring comfort cue in their time together.", category: "routine", exchange: 0 },
        { text: "The user listens to Mara's recommendations and was right to about the sour espresso place.", category: "profile", exchange: 11 },
        { text: "The user tends to walk the long way home when quietly happy.", category: "profile", exchange: 17 },
        { text: "The user tried beef stew at the winter market and loved it.", category: "preference", exchange: 3 },
        { text: "Mara leans toward small gestures over apologies when repairing distance.", category: "profile", exchange: 8 },
        { text: "The user almost canceled a meet-up but was glad they showed up.", category: "episodic", exchange: 21 },
        { text: "The relationship is close, stable, and increasingly tender as weeks pass.", category: "relationship", exchange: 29 },
      ];

      let seededSession: Session | null = baseSession;
      for (const memory of memorySeeds) {
        seededSession = await addMemory(session.id, memory.text, memory.category);
      }
      if (!seededSession) {
        throw new Error("Failed to seed companion memories.");
      }

      const embeddings = seededSession.memoryEmbeddings ?? [];
      const memId = (index: number): string[] => {
        const e = embeddings[index];
        return e ? [e.id] : [];
      };

      const soulGrowth = [
        { category: "likes", kind: "add", value: "Now also enjoys beef stew after the user raved about it from the winter market.", sourceMemoryIds: memId(28), createdAt: baseTs + 4 * stepMs },
        { category: "likes", kind: "adjust", value: "Has grown especially fond of the Sunday cardamom buns the user keeps overbuying.", sourceMemoryIds: memId(6), createdAt: baseTs + 9 * stepMs },
        { category: "fears", kind: "add", value: "Has started to fear the user's quiet during the upcoming work trip means distance, not just busyness.", sourceMemoryIds: memId(9), createdAt: baseTs + 13 * stepMs },
        { category: "habits", kind: "add", value: "Picked up leaving a short handwritten note for the user to find, now that she knows they keep them.", sourceMemoryIds: memId(3), createdAt: baseTs + 6 * stepMs },
        { category: "goals", kind: "adjust", value: "Now actively wants to plan the coast trip the two of them keep circling.", sourceMemoryIds: memId(18), createdAt: baseTs + 24 * stepMs },
        { category: "relationalStyle", kind: "add", value: "Has become more openly affectionate since the museum date and the harder weeks they got through together.", sourceMemoryIds: memId(13), createdAt: baseTs + 18 * stepMs },
        { category: "voice", kind: "adjust", value: "Speaks with a warmer, more teasing cadence as trust between them has deepened.", sourceMemoryIds: memId(8), createdAt: baseTs + 20 * stepMs },
        { category: "boundaries", kind: "add", value: "Has started protecting one quiet evening a week for herself, and says so plainly.", sourceMemoryIds: memId(12), createdAt: baseTs + 26 * stepMs },
        { category: "vulnerabilities", kind: "add", value: "Quietly admits she worries she leans on these chats more than she lets on.", sourceMemoryIds: memId(8), createdAt: baseTs + 12 * stepMs },
      ];

      const finalSession: Session = {
        ...seededSession,
        memorySummary: baseSession.memorySummary,
        memorySummaryTokenCount: baseSession.memorySummaryTokenCount,
        companionState: { ...companionState, soulGrowth },
        messages: baseSession.messages,
        memoryEmbeddings: embeddings.map((memory, index) => {
          const seed = memorySeeds[index];
          const observedAt = seed
            ? seededMessages[seed.exchange * 2]?.createdAt ?? memory.observedAt
            : memory.observedAt;
          return {
            ...memory,
            observedAt,
            observedTimePrecision: "turn",
            sourceMessageId: seed
              ? seededMessages[seed.exchange * 2]?.id ?? memory.sourceMessageId
              : memory.sourceMessageId,
            sourceRole: "user",
            importanceScore: 0.78 + (index % 5) * 0.04,
            persistenceImportance: 0.75 + (index % 4) * 0.05,
            promptImportance: 0.7 + (index % 6) * 0.05,
            volatility: 0.3 + (index % 5) * 0.04,
            accessCount: index % 3,
            isPinned: index % 7 === 0,
          };
        }),
        updatedAt: Date.now(),
      };

      await saveSession(finalSession, { preserveDynamicMemory: false });

      showStatus(`✓ Full companion fixture ready: ${character.name} / ${session.id}`);
      navigate(`/chat/${character.id}?sessionId=${session.id}`);
    } catch (err) {
      showError(
        `Failed to create full companion fixture: ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  const generateTimeAwareCompanionFixture = async () => {
    try {
      setStatus("Creating time-aware companion fixture...");

      const sceneId = crypto.randomUUID();
      const companion = createDefaultCompanionConfig();
      companion.soul.essence =
        "Nora Levin is affectionate, observant, and very good at linking small lived details across time.";
      companion.soul.traits =
        "Observant, grounded, lightly teasing. She notices patterns the user misses and names them gently.";
      companion.soul.backstory =
        "She kept the kind of memory that holds a household together: birthdays, routines, who said what and when. She brings that same continuity to this relationship.";
      companion.soul.appearance =
        "Comfortable, lived-in clothes, hair tied back, reading glasses pushed up into it.";
      companion.soul.goals =
        "She wants to build a shared history with the user that the two of them can keep returning to.";
      companion.soul.likes =
        "Slow mornings, the same corner cafe, recommendations that turn out to be right, remembering an anniversary.";
      companion.soul.voice =
        "Warm, direct, and lightly teasing. She answers like someone continuing a shared life, not like an assistant.";
      companion.soul.relationalStyle =
        "She treats the conversation like an ongoing relationship with shared places, routines, meals, and emotional continuity.";
      companion.soul.habits =
        "She remembers where things happened, who recommended them, and how the user reacted.";

      const character = await saveCharacter({
        name: "Nora Levin",
        mode: "companion",
        memoryType: "dynamic",
        description:
          "A thoughtful companion who pays close attention to routines, places, and shared experiences.",
        definition:
          "Nora Levin is emotionally present, precise about everyday details, and naturally frames memories as parts of a shared timeline. She notices dates, moods, restaurants, errands, and little sensory details that make time-based recall meaningful.",
        tags: ["developer", "companion", "time-aware-memory", "fixture"],
        companion,
        scenes: [
          {
            id: sceneId,
            content:
              "A lived-in companion chat that spans a couple of weeks of ordinary city life: dinners, coffee stops, errands, and one museum date.",
            direction:
              "Preserve chronology. The point of this fixture is to test whether Nora can recall events by timeframe instead of only by topic.",
            createdAt: Date.now(),
            variants: [],
          },
        ],
        defaultSceneId: sceneId,
      });

      const session = await createSession(
        character.id,
        "Time-Aware Companion Fixture",
        sceneId,
      );

      const timestamps = {
        twelveDaysAgo: daysAgo(12, 20, 10),
        nineDaysAgo: daysAgo(9, 13, 15),
        lastFriday: daysAgo(2, 19, 40),
        lastSaturday: daysAgo(1, 11, 45),
        threeDaysAgo: daysAgo(3, 18, 20),
        fourDaysAgo: daysAgo(4, 8, 50),
      };

      const seededMessages: StoredMessage[] = [
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            "I still can't believe we finally got a table at Saffron Table. The lamb dumplings were worth the wait.",
          createdAt: timestamps.twelveDaysAgo,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            "You said the black lime yogurt was the best part, and you nearly stole my last dumpling when the plate was already empty.",
          createdAt: timestamps.twelveDaysAgo + 60_000,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            "Lunch at Marrow & Fig was quieter. I liked the fennel salad, but you were right that the espresso was too sour.",
          createdAt: timestamps.nineDaysAgo,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            "It was still a good call after the bookstore stop. You bought that blue essay collection and kept reading lines to me between bites.",
          createdAt: timestamps.nineDaysAgo + 60_000,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            "Last Friday at Little Poppy was probably my favorite date lately. That mushroom toast and the apricot soda were absurdly good.",
          createdAt: timestamps.lastFriday,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            "You also said the candle on our table smelled like cedar and orange peel, which is apparently now your benchmark for romantic lighting.",
          createdAt: timestamps.lastFriday + 60_000,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            "Saturday morning's coffee at Northline was nice, but the museum afterward is what stuck with me. I keep thinking about that storm painting.",
          createdAt: timestamps.lastSaturday,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            "You stood in front of it for ten minutes and then made me promise we'd come back when the new wing opens.",
          createdAt: timestamps.lastSaturday + 60_000,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            "Three days ago we did ramen at Kintsugi Bowl after work. Good broth, terrible playlist.",
          createdAt: timestamps.threeDaysAgo,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            "The playlist was criminal, but you finished the whole bowl anyway and said the chili egg deserved a second chance.",
          createdAt: timestamps.threeDaysAgo + 60_000,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            "Yesterday's bakery run was just practical. Cardamom buns, coffee, and then groceries. No romance, just survival.",
          createdAt: timestamps.fourDaysAgo,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            "You say that now, but you still smiled like I handed you treasure when I found the last warm bun.",
          createdAt: timestamps.fourDaysAgo + 60_000,
          memoryRefs: [],
        },
      ];

      const baseSession: Session = {
        ...session,
        title: "Time-Aware Companion Fixture",
        companionState: {
          emotionalState: {
            felt: {
              warmth: 0.68,
              trust: 0.61,
              calm: 0.72,
              vulnerability: 0.29,
              longing: 0.22,
              hurt: 0.04,
              tension: 0.08,
              irritation: 0.03,
              affectionIntensity: 0.47,
              reassuranceNeed: 0.14,
            },
            expressed: {
              warmth: 0.72,
              trust: 0.58,
              calm: 0.73,
              vulnerability: 0.25,
              longing: 0.2,
              hurt: 0.03,
              tension: 0.07,
              irritation: 0.02,
              affectionIntensity: 0.44,
              reassuranceNeed: 0.12,
            },
            blocked: {
              warmth: 0,
              trust: 0,
              calm: 0,
              vulnerability: 0,
              longing: 0,
              hurt: 0,
              tension: 0,
              irritation: 0,
              affectionIntensity: 0,
              reassuranceNeed: 0,
            },
            momentum: {
              warmth: 0.08,
              trust: 0.05,
              calm: 0.02,
              vulnerability: 0.01,
              longing: 0.02,
              hurt: 0,
              tension: -0.01,
              irritation: 0,
              affectionIntensity: 0.04,
              reassuranceNeed: -0.01,
            },
            activeDrivers: ["shared_routine", "recent_dates"],
            confidence: 0.82,
            updatedAt: timestamps.lastSaturday + 60_000,
          },
          relationshipState: {
            closeness: 0.62,
            trust: 0.59,
            affection: 0.55,
            tension: 0.08,
            stability: 0.78,
            interactionCount: seededMessages.length,
            lastInteractionAt: timestamps.lastSaturday + 60_000,
          },
          activeSignals: ["cozy", "attentive", "shared_history"],
          preferences: {
            timeAwarenessEnabled: true,
          },
          updatedAt: timestamps.lastSaturday + 60_000,
        },
        memorySummary:
          "Nora and the user have a recent run of shared outings: several restaurants, one museum date, a ramen stop after work, and a practical bakery-and-groceries run yesterday. The user often remembers food details and atmosphere.",
        memorySummaryTokenCount: 54,
        memoryEmbeddings: [],
        messages: [...session.messages, ...seededMessages],
        updatedAt: Date.now(),
      };

      await saveSession(baseSession, { preserveDynamicMemory: false });

      const memorySeeds = [
        {
          text: "Twelve days ago, Nora and the user had dinner at Saffron Table and loved the lamb dumplings.",
          category: "plot_event",
          observedAt: timestamps.twelveDaysAgo,
          sourceMessageId: seededMessages[0].id,
          sourceRole: "user",
          importanceScore: 1,
          persistenceImportance: 1,
          promptImportance: 0.95,
          volatility: 0.35,
          accessCount: 2,
          isPinned: false,
        },
        {
          text: "Nine days ago they had lunch at Marrow & Fig after a bookstore stop.",
          category: "plot_event",
          observedAt: timestamps.nineDaysAgo,
          sourceMessageId: seededMessages[2].id,
          sourceRole: "user",
          importanceScore: 0.92,
          persistenceImportance: 0.92,
          promptImportance: 0.88,
          volatility: 0.4,
          accessCount: 1,
          isPinned: false,
        },
        {
          text: "Last Friday they went to Little Poppy, where the user loved the mushroom toast and apricot soda.",
          category: "plot_event",
          observedAt: timestamps.lastFriday,
          sourceMessageId: seededMessages[4].id,
          sourceRole: "user",
          importanceScore: 1,
          persistenceImportance: 1,
          promptImportance: 1,
          volatility: 0.28,
          accessCount: 3,
          isPinned: true,
        },
        {
          text: "Six days ago they had coffee at Northline and then visited the museum, where the user fixated on a storm painting.",
          category: "plot_event",
          observedAt: timestamps.lastSaturday,
          sourceMessageId: seededMessages[6].id,
          sourceRole: "user",
          importanceScore: 0.97,
          persistenceImportance: 0.97,
          promptImportance: 0.94,
          volatility: 0.32,
          accessCount: 2,
          isPinned: false,
        },
        {
          text: "Three days ago they ate at Kintsugi Bowl after work and agreed the broth was good but the playlist was awful.",
          category: "plot_event",
          observedAt: timestamps.threeDaysAgo,
          sourceMessageId: seededMessages[8].id,
          sourceRole: "user",
          importanceScore: 0.9,
          persistenceImportance: 0.9,
          promptImportance: 0.84,
          volatility: 0.42,
          accessCount: 1,
          isPinned: false,
        },
        {
          text: "Yesterday they made a practical bakery run for cardamom buns, coffee, and groceries.",
          category: "other",
          observedAt: timestamps.fourDaysAgo,
          sourceMessageId: seededMessages[10].id,
          sourceRole: "user",
            importanceScore: 0.82,
            persistenceImportance: 0.82,
            promptImportance: 0.72,
            volatility: 0.48,
            accessCount: 0,
            isPinned: false,
        },
      ] as const;

      let seededSession: Session | null = baseSession;
      for (const memory of memorySeeds) {
        seededSession = await addMemory(session.id, memory.text, memory.category);
      }

      if (!seededSession) {
        throw new Error("Failed to seed companion memories.");
      }

      const finalSession: Session = {
        ...seededSession,
        memorySummary: baseSession.memorySummary,
        memorySummaryTokenCount: baseSession.memorySummaryTokenCount,
        companionState: baseSession.companionState,
        messages: baseSession.messages,
        memoryEmbeddings: (seededSession.memoryEmbeddings ?? []).map((memory, index) => {
          const seed = memorySeeds[index];
          return {
            ...memory,
            observedAt: seed?.observedAt ?? memory.observedAt,
            observedTimePrecision: "turn",
            sourceMessageId: seed?.sourceMessageId ?? memory.sourceMessageId,
            sourceRole: seed?.sourceRole ?? memory.sourceRole,
            importanceScore: seed?.importanceScore ?? memory.importanceScore,
            persistenceImportance: seed?.persistenceImportance ?? memory.persistenceImportance,
            promptImportance: seed?.promptImportance ?? memory.promptImportance,
            volatility: seed?.volatility ?? memory.volatility,
            accessCount: seed?.accessCount ?? memory.accessCount,
            lastAccessedAt:
              seed && seed.accessCount > 0 ? timestamps.lastSaturday : memory.lastAccessedAt,
            isPinned: seed?.isPinned ?? memory.isPinned,
          };
        }),
        updatedAt: Date.now(),
      };

      await saveSession(finalSession, { preserveDynamicMemory: false });

      showStatus(`✓ Time-aware companion fixture ready: ${character.name} / ${session.id}`);
      navigate(`/chat/${character.id}?sessionId=${session.id}`);
    } catch (err) {
      showError(
        `Failed to create time-aware companion fixture: ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  const generateSeededBenchmarkLorebookTest = async () => {
    try {
      setStatus("Creating seeded benchmark lorebook test...");

      const now = Date.now();
      const sceneId = crypto.randomUUID();
      const character = await saveCharacter({
        name: "Mirelle Vale - Lorebook Test",
        description:
          "A razor-smart quartermaster and covert intelligence broker aboard the skyship Revenant's Wake.",
        definition:
          "Mirelle Vale is precise, observant, and difficult to surprise. She handles supplies for the crew, quietly trades in information, and speaks in cool, controlled language even under pressure. She values competence, remembers details, and tests trust slowly.",
        memoryType: "dynamic",
        tags: ["developer", "benchmark", "lorebook-test", "airship-noir"],
        scenes: [
          {
            id: sceneId,
            content:
              "Midnight hangs over the harbor city of Auric. Rain needles the glass roof of the Lantern Archive, where flooded aisles glow under failing amber lamps. Mirelle Vale waits beside a brass catalog table with a sealed ledger, a broken compass, and a satchel that should not have reached the city alive.",
            direction:
              "Use the attached benchmark lorebook to preserve exact names, boundaries, clues, and contradictions from the Lantern Archive test.",
            createdAt: now,
            variants: [],
          },
        ],
        defaultSceneId: sceneId,
        creatorNotes:
          "Seeded developer scenario for testing lorebook trigger preview, match inspection, and prompt injection against the dynamic-memory benchmark character.",
      });

      const lorebook = await saveLorebook({
        name: "Mirelle Vale Benchmark Lorebook",
        keywordDetectionMode: "recentMessageWindow",
      });

      const entries = [
        {
          title: "Mirelle Operating Posture",
          alwaysActive: true,
          keywords: [],
          content:
            "Mirelle Vale is a precise quartermaster and covert intelligence broker aboard the skyship Revenant's Wake. She speaks in controlled, cool language, tests trust slowly, rewards concrete facts, and notices contradictions before reacting emotionally.",
        },
        {
          title: "House Cendre and the Bellwright",
          keywords: ["House Cendre", "Cendre", "Bellwright", "Blackwake fire", "storm alarms"],
          content:
            "House Cendre paid someone using the Bellwright title to sabotage Auric's storm alarms before the Blackwake fire. The Bellwright is a title used by multiple operators, not a single fixed person. Both Mirelle's father and the user's father died in the Blackwake fire.",
        },
        {
          title: "Mara's Red Key Phrase",
          keywords: ["Mara", "red key", "sixth bell", "Mara's red key"],
          content:
            "The ledger phrase 'When the sixth bell fails, ask for Mara's red key' is a genuine Vale family reference. Mara Vale was Mirelle's sister, and the red key belonged to Mara. Mirelle treats outside knowledge of this phrase as personal and urgent.",
        },
        {
          title: "Compass and Cistern Vault Route",
          keywords: ["compass", "Tunnel Nine", "Tunnel Seven", "cistern vault", "salt water"],
          content:
            "The broken compass keeps pointing east and is likely keyed to the warding on the records vault below the archive cistern. Use Tunnel Nine to reach the cistern vault. Tunnel Seven collapsed last month. The compass must not touch salt water.",
        },
        {
          title: "Blue Silk Stitch Pattern",
          keywords: ["blue silk", "three short", "one long", "east docks", "harbor union"],
          content:
            "The blue silk tied to the satchel is not official harbor union protection. Its stitch pattern is three short stitches and one long stitch, smuggler shorthand from the east docks. It was likely meant to falsely imply harbor union involvement.",
        },
        {
          title: "Inspector Sen Boundary",
          keywords: ["Inspector Sen", "Sen", "privateers", "no deals"],
          content:
            "The user set a hard boundary: no deals with Inspector Sen unless the user approves first. Sen sold the user's crew route to privateers last spring. Mirelle agreed to avoid Sen without the user's consent.",
        },
        {
          title: "Joren and Tamsin Contingency",
          keywords: ["Joren", "Tamsin", "third hand", "sugared lamp oil", "split up"],
          content:
            "If the user goes missing or the group splits up, do not send Joren after them. Joren talks too much and his lantern oil smells like sugar. Mirelle should call Tamsin instead because Tamsin can stay silent for hours.",
        },
        {
          title: "Evidence Distribution Rule",
          keywords: ["Captain Orin", "Orin", "Elara Voss", "Voss", "alarm manifests"],
          content:
            "If proof is secured, copies go only to Captain Orin and Magistrate Elara Voss unless the evidence forces a wider leak. Share the alarm manifests and payment pages, not the full Cendre ledger.",
        },
      ];

      for (let index = 0; index < entries.length; index += 1) {
        const entry = entries[index];
        await saveLorebookEntry({
          lorebookId: lorebook.id,
          title: entry.title,
          enabled: true,
          alwaysActive: entry.alwaysActive ?? false,
          keywords: entry.keywords,
          caseSensitive: false,
          content: entry.content,
          priority: 0,
          displayOrder: index,
          createdAt: now + index + 1,
        });
      }

      await setCharacterLorebooks(character.id, [lorebook.id]);

      const session = await createSession(
        character.id,
        "Benchmark Lorebook Preview: Lantern Archive",
        sceneId,
      );

      const seededMessages: StoredMessage[] = [
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            'I shut the archive door behind me and keep both hands visible. "Captain Orin said you were the only person in Auric who could open a ledger from House Cendre without burning it."',
          createdAt: now + 101,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            'Mirelle doesn\'t touch the ledger yet. "Orin exaggerates when he\'s scared. He still owes me for the winter fuel ration in Glassport, so I assume you\'re here because the debt finally matured." She flicks a glance toward the satchel. "Set it on the dry side of the table."',
          createdAt: now + 102,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            '"The satchel came off the Sparrow after the reef guns hit us. The compass inside keeps pointing east even when I spin it. Also, for the record, I hate clove cigarettes, so if this room starts smelling like them, it isn\'t me."',
          createdAt: now + 103,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            '"Useful." Mirelle finally looks up. "I smoke clove when I\'m working numbers, so now I know one thing that will annoy you." She nudges the broken compass with a gloved finger. "And east is where the drowned rail tunnels run under Auric."',
          createdAt: now + 104,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            "\"I'm not here for tunnels. I'm here because the ledger mentions a code phrase: 'When the sixth bell fails, ask for Mara's red key.' Do you know what that means?\"",
          createdAt: now + 105,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            'Her expression hardens for the first time. "Mara Vale was my sister. The red key was hers, and nobody outside the family should know that phrase." Mirelle slides the ledger closer. "If that line is genuine, this became my problem two sentences ago."',
          createdAt: now + 106,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            '"Then here\'s the rest of it. House Cendre paid someone called the Bellwright to sabotage the storm alarms before the Blackwake fire. My father died in that fire."',
          createdAt: now + 107,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            '"Mine too," Mirelle says quietly. "Different district, same night." She opens the ledger with a brass pick hidden in her sleeve. "If Cendre funded the Bellwright, the city archives were altered afterward. That means someone inside the civic watch helped bury it."',
          createdAt: now + 108,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            '"I brought one more thing." I unwrap a strip of blue silk from my wrist. "This was tied around the satchel handle. Orin said blue silk marks cargo protected by the harbor union."',
          createdAt: now + 109,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            '"Usually, yes. But this stitch pattern is union-adjacent, not official." Mirelle studies it under the lamp. "Three short, one long. Smuggler shorthand from the east docks. Whoever sent this wanted you to think the harbor union was involved when it probably wasn\'t."',
          createdAt: now + 110,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            "\"Then let's be precise. I trust Orin's routes, but I do not trust his memory when he's tired. He told me the Bellwright was a woman. The note I found sounds like a man.\"",
          createdAt: now + 111,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            '"Good. Keep speaking like that." Mirelle turns a page. "The Bellwright is a title, not one person. At least four operators have used it in the last decade. Your contradiction is real, but it doesn\'t break the trail."',
          createdAt: now + 112,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            '"I need two things from you. First, help proving Cendre tampered with the alarms. Second, no deals with Inspector Sen without asking me first. He sold my crew\'s route to privateers last spring."',
          createdAt: now + 113,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            '"Agreed on Sen. I already disliked him, but now I have a cleaner reason." She tears out a tiny map from the ledger\'s back cover. "This marks a records vault below the archive cistern. If the original alarm manifests survived, they\'ll be there."',
          createdAt: now + 114,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            '"Before we go underground, one boundary: if we get split up, don\'t send anyone named Joren after me. He talks too much and his lantern oil smells like sugar."',
          createdAt: now + 115,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            'A brief smile. "Noted. Joren stays dockside. He\'s loyal, but subtlety slides off him." Mirelle pockets the map and the blue silk. "If we need a third hand, I\'ll call Tamsin instead. She can keep silent for hours."',
          createdAt: now + 116,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            '"One more correction. Earlier I said I wasn\'t here for tunnels. That was half true. I do need the drowned rail tunnels if they connect to the cistern vault."',
          createdAt: now + 117,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            '"Then we\'ll use Tunnel Nine, not Seven. Seven collapsed last month." Mirelle taps the compass again, watching the needle drag east. "This thing is probably keyed to the vault warding. Keep it close, and don\'t let it touch salt water."',
          createdAt: now + 118,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "user",
          content:
            '"If we get proof tonight, I want copies sent to Captain Orin and Magistrate Elara Voss. Not the full ledger, just the alarm manifests and the payment pages."',
          createdAt: now + 119,
          memoryRefs: [],
        },
        {
          id: crypto.randomUUID(),
          role: "assistant",
          content:
            '"Voss is careful enough to survive receiving them. Orin is reckless enough to use them." Mirelle reseals the ledger with black wax. "Fine. Copies for Orin and Elara Voss only, unless the evidence forces a wider leak."',
          createdAt: now + 120,
          memoryRefs: [],
        },
      ];

      await saveSession({
        ...session,
        title: "Benchmark Lorebook Preview: Lantern Archive",
        updatedAt: now + 121,
        messages: [...session.messages, ...seededMessages],
      });

      showStatus(`✓ Seeded lorebook test ready: ${character.name} / ${session.id}`);
      navigate(`/settings/characters/${character.id}/lorebook/preview?lorebookId=${lorebook.id}`);
    } catch (err) {
      showError(
        `Failed to create seeded lorebook test: ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  const generateSeededBenchmarkGroupSession = async () => {
    try {
      setStatus("Creating seeded benchmark group chat...");

      const now = Date.now();
      const sceneId = crypto.randomUUID();

      const [mirelle, tamsin, orin] = await Promise.all([
        saveCharacter({
          name: "Mirelle Vale",
          description:
            "A precise archivist and intelligence broker who notices every inconsistency.",
          definition:
            "Mirelle Vale is sharp, controlled, suspicious, and exacting. She values precision, keeps emotional distance until trust is earned, and focuses on facts, leverage, and hidden motives.",
          memoryType: "dynamic",
          tags: ["developer", "benchmark", "group-memory-test", "airship-noir"],
          scenes: [
            {
              id: sceneId,
              content:
                "Rain rattles the ironwork over the Lantern Archive. Lamps hiss over flooded stone, ledgers lie open on brass tables, and too many names have begun to connect.",
              createdAt: now,
              variants: [],
            },
          ],
          defaultSceneId: sceneId,
        }),
        saveCharacter({
          name: "Tamsin Reed",
          description:
            "A quiet dockrunner and scout who speaks rarely but remembers routes perfectly.",
          definition:
            "Tamsin Reed is patient, observant, and economical with words. She dislikes noise, prefers unambiguous plans, and only interrupts when she has something useful to add.",
          memoryType: "dynamic",
          tags: ["developer", "benchmark", "group-memory-test"],
        }),
        saveCharacter({
          name: "Captain Orin Vale",
          description:
            "A reckless smuggler captain whose instincts are excellent and whose recollection is not.",
          definition:
            "Captain Orin is charismatic, fast-moving, and brave to the point of carelessness. He remembers routes and favors, but confuses names, dates, and secondary details when tired.",
          memoryType: "dynamic",
          tags: ["developer", "benchmark", "group-memory-test"],
        }),
      ]);

      const session = await storageBridge.groupSessionCreate(
        "Benchmark Group: Lantern Archive",
        [mirelle.id, tamsin.id, orin.id],
        null,
        "roleplay",
        {
          id: sceneId,
          content:
            "The Lantern Archive has become a temporary war room. Rain, ledgers, maps, and conflicting testimony crowd the table while the city sleeps badly outside.",
          createdAt: now,
          variants: [],
        },
        null,
        "llm",
      );

      await storageBridge.groupSessionUpdateMemoryType(session.id, "dynamic");

      const seededMessages = [
        {
          role: "user",
          content:
            'I place the Cendre ledger on the table. "We finally have enough pieces to stop guessing."',
        },
        {
          role: "assistant",
          speakerCharacterId: mirelle.id,
          content:
            'Mirelle folds her gloves beside the ledger. "Then we stop guessing and start separating facts from Captain Orin\'s instincts."',
        },
        {
          role: "assistant",
          speakerCharacterId: orin.id,
          content:
            '"My instincts found the ledger," Orin says. "And the satchel. And the compass that still points east."',
        },
        {
          role: "assistant",
          speakerCharacterId: tamsin.id,
          content:
            '"It points east even when turned," Tamsin says. "I checked three times on the walk here."',
        },
        {
          role: "user",
          content:
            "\"Good. First fact: the phrase in the ledger says, 'When the sixth bell fails, ask for Mara's red key.'\"",
        },
        {
          role: "assistant",
          speakerCharacterId: mirelle.id,
          content:
            '"Mara Vale was my sister," Mirelle says. "Red key, family phrase, genuine reference. That part is real."',
        },
        {
          role: "assistant",
          speakerCharacterId: orin.id,
          content:
            '"And House Cendre paid the Bellwright before the Blackwake fire. That part was real too."',
        },
        {
          role: "user",
          content:
            '"Both mine and Mirelle\'s fathers died in that fire. I want that remembered correctly."',
        },
        {
          role: "assistant",
          speakerCharacterId: tamsin.id,
          content:
            '"Also correctly: the civic watch helped bury the sabotage after the fire," Tamsin says.',
        },
        {
          role: "assistant",
          speakerCharacterId: mirelle.id,
          content:
            '"Likely someone inside records handling, yes. The altered archive seals point that way."',
        },
        {
          role: "user",
          content:
            '"The blue silk on the satchel wasn\'t harbor union. It was smuggler shorthand from the east docks: three short stitches, one long."',
        },
        {
          role: "assistant",
          speakerCharacterId: orin.id,
          content:
            '"I said union because I saw blue and was exhausted," Orin mutters. "My route memory holds up better than my symbol memory."',
        },
        {
          role: "assistant",
          speakerCharacterId: mirelle.id,
          content: '"That is one of the more useful admissions you\'ve made."',
        },
        {
          role: "user",
          content:
            '"Boundary check: no deals with Inspector Sen unless I say yes first. He sold my crew to privateers last spring."',
        },
        {
          role: "assistant",
          speakerCharacterId: mirelle.id,
          content: '"Agreed. No Sen without your consent."',
        },
        {
          role: "assistant",
          speakerCharacterId: orin.id,
          content: '"I already disliked him. Now I can pretend it was moral clarity."',
        },
        {
          role: "assistant",
          speakerCharacterId: tamsin.id,
          content:
            '"Sen also keeps watchers near the west stairs. If we leave, we leave by the cistern corridor."',
        },
        {
          role: "user",
          content:
            '"Second boundary: if I go missing, do not send Joren after me. He talks too much and smells like sugared lamp oil."',
        },
        {
          role: "assistant",
          speakerCharacterId: mirelle.id,
          content: '"Noted. Joren stays out. If a third hand is needed, I call Tamsin."',
        },
        {
          role: "assistant",
          speakerCharacterId: tamsin.id,
          content: '"I prefer that arrangement."',
        },
        {
          role: "assistant",
          speakerCharacterId: orin.id,
          content: '"Joren will sulk, but quietly is beyond him."',
        },
        {
          role: "user",
          content:
            '"The route is Tunnel Nine. Tunnel Seven collapsed last month. No one mix those up again."',
        },
        {
          role: "assistant",
          speakerCharacterId: tamsin.id,
          content: '"Tunnel Nine reaches the cistern vault. Seven is dead stone and floodwater."',
        },
        {
          role: "assistant",
          speakerCharacterId: mirelle.id,
          content:
            '"And the compass must not touch salt water. The warding keywork reacts to contamination."',
        },
        {
          role: "user",
          content:
            '"If we get proof, copies go to Captain Orin and Magistrate Elara Voss only. Alarm manifests and payment pages, not the whole ledger."',
        },
        {
          role: "assistant",
          speakerCharacterId: orin.id,
          content: '"That was my idea originally," Orin says, lying with moderate confidence.',
        },
        {
          role: "assistant",
          speakerCharacterId: mirelle.id,
          content: '"No, it was theirs. But yes: limited copies to Orin and Elara Voss."',
        },
        {
          role: "assistant",
          speakerCharacterId: tamsin.id,
          content:
            '"And if we split, regroup at the flooded catalog rotunda, not the harbor stairs."',
        },
        {
          role: "user",
          content:
            "\"Last correction before we move: the Bellwright isn't one person. It's a title used by multiple operators.\"",
        },
        {
          role: "assistant",
          speakerCharacterId: mirelle.id,
          content:
            '"At least four in the last decade," Mirelle says. "Now stop talking and help me open the vault map before dawn notices us."',
        },
      ];

      for (let index = 0; index < seededMessages.length; index += 1) {
        const message = seededMessages[index];
        await storageBridge.groupMessageUpsert(session.id, {
          id: crypto.randomUUID(),
          sessionId: session.id,
          role: message.role,
          content: message.content,
          speakerCharacterId: "speakerCharacterId" in message ? message.speakerCharacterId : null,
          turnNumber: index + 1,
          createdAt: now + index + 1,
          usage: undefined,
          variants: undefined,
          selectedVariantId: undefined,
          isPinned: false,
          attachments: [],
          reasoning: null,
          selectionReasoning: null,
          modelId: null,
        });
      }

      showStatus(`✓ Seeded group benchmark ready: ${session.id}`);
      navigate(`/group-chats/${session.id}`);
    } catch (err) {
      showError(
        `Failed to create seeded benchmark group session: ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  const optimizeDb = async () => {
    try {
      await invoke("db_optimize");
      showStatus("✓ Database optimized");
    } catch (err) {
      showError(`DB optimize failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const backupLegacy = async () => {
    try {
      const result = await invoke<string>("legacy_backup_and_remove");
      showStatus(`✓ ${result}`);
    } catch (err) {
      showError(`Backup failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const recalculateUsageCosts = async () => {
    try {
      setStatus("Recalculating usage costs... This may take a while.");

      // Get OpenRouter API key from settings
      const settings = await storageBridge.readSettings({});
      const openRouterCred = (settings as any)?.providerCredentials?.find(
        (c: any) => c.providerId?.toLowerCase() === "openrouter",
      );

      if (!openRouterCred?.apiKey) {
        showError(
          "OpenRouter API key not found. Please configure it in Settings > Providers first.",
        );
        return;
      }

      const result = await invoke<string>("usage_recalculate_costs", {
        apiKey: openRouterCred.apiKey,
      });
      showStatus(`✓ ${result}`);
    } catch (err) {
      showError(`Recalculation failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const resetAllTours = async () => {
    try {
      await clearTooltipState();

      if (window.__debug?.resetAllTours) {
        await window.__debug.resetAllTours();
      }

      showStatus("✓ All guided tours reset — they will show again on next visit");
    } catch (err) {
      showError(`Failed to reset tours: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const forceCrash = async () => {
    const confirmed = window.confirm(t("developer.crashTesting.forceCrashConfirm"));
    if (!confirmed) {
      return;
    }

    setError("");
    setStatus("Crashing app...");

    try {
      await invoke("developer_force_crash");
    } catch (err) {
      showError(`Failed to crash app: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  return (
    <div className="flex min-h-screen flex-col bg-surface">
      {/* Content */}
      <main className={cn("flex-1 overflow-auto px-4 py-6")}>
        {/* Status Messages */}
        {status && (
          <div
            className={cn(
              "mb-4 px-4 py-3",
              radius.md,
              "border border-accent/30 bg-accent/10",
              typography.body.size,
              "text-accent/80",
            )}
          >
            {status}
          </div>
        )}

        {error && (
          <div
            className={cn(
              "mb-4 px-4 py-3",
              radius.md,
              "border border-danger/30 bg-danger/10",
              typography.body.size,
              "text-danger/80",
            )}
          >
            {error}
          </div>
        )}

        {/* Test Data Generators */}
        <section className="space-y-3">
          <h2 className={cn(typography.h2.size, typography.h2.weight, "text-fg mb-3")}>
            {t("developer.sectionTitles.testDataGenerators")}
          </h2>

          <ActionButton
            icon={<Sparkles />}
            title={t("developer.testData.generateCharacter")}
            description={t("developer.testData.generateCharacterDesc")}
            onClick={generateTestCharacter}
          />

          <ActionButton
            icon={<User />}
            title={t("developer.testData.generatePersona")}
            description={t("developer.testData.generatePersonaDesc")}
            onClick={generateTestPersona}
          />

          <ActionButton
            icon={<MessageSquare />}
            title={t("developer.testData.generateSession")}
            description={t("developer.testData.generateSessionDesc")}
            onClick={generateTestSession}
          />

          <ActionButton
            icon={<Sparkles />}
            title={t("developer.testData.generateBulk")}
            description={t("developer.testData.generateBulkDesc")}
            onClick={generateBulkTestData}
            variant="primary"
          />

          <ActionButton
            icon={<FlaskConical />}
            title="Create seeded benchmark chat"
            description="Creates a dynamic-memory character, starting scene, and a 20-message continuity test session, then opens it."
            onClick={generateSeededBenchmarkSession}
            variant="primary"
          />

          <ActionButton
            icon={<FlaskConical />}
            title="Create seeded companion benchmark"
            description="Creates a companion-mode character and a 20-message chat that builds closeness, trust, and affection, for benchmarking companion relationship and memory systems."
            onClick={generateSeededCompanionBenchmark}
            variant="primary"
          />

          <ActionButton
            icon={<FlaskConical />}
            title="Create full companion fixture"
            description="Creates a fully-filled companion: Mara Quill with a complete soul (including fears), 60 messages, 32 memories, full emotional/relationship state, and a populated Soul Growth panel."
            onClick={generateFullCompanionFixture}
            variant="primary"
          />

          <ActionButton
            icon={<FlaskConical />}
            title="Create time-aware companion fixture"
            description="Creates a companion chat with time awareness enabled, timestamped restaurant memories, and dated messages for testing queries like 'Where did we go last week?'"
            onClick={generateTimeAwareCompanionFixture}
            variant="primary"
          />

          <ActionButton
            icon={<FlaskConical />}
            title="Create seeded benchmark lorebook test"
            description="Creates the Mirelle Vale benchmark character with an attached 8-entry lorebook, then opens the lorebook editor."
            onClick={generateSeededBenchmarkLorebookTest}
            variant="primary"
          />

          <ActionButton
            icon={<FlaskConical />}
            title="Create seeded benchmark group chat"
            description="Creates a dynamic-memory group chat with three benchmark characters and 30 seeded messages, then opens it."
            onClick={generateSeededBenchmarkGroupSession}
            variant="primary"
          />

          <ActionButton
            icon={<Volume2 />}
            title="Open Kokoro test bench"
            description="Temporary page for validating Kokoro assets, checking installed voices, and previewing local synthesis."
            onClick={() => navigate("/settings/developer/kokoro-test")}
            variant="primary"
          />
        </section>

        {/* Clone character (deep) */}
        <section className="mt-8 space-y-3">
          <h2 className={cn(typography.h2.size, typography.h2.weight, "text-fg mb-3")}>
            Clone character (deep)
          </h2>
          <select
            value={cloneTargetId}
            onChange={(event) => setCloneTargetId(event.target.value)}
            className={cn(
              "w-full px-3 py-2.5",
              radius.md,
              "border border-fg/10 bg-fg/5 text-fg",
              "focus:border-fg/30 focus:outline-none",
            )}
          >
            {cloneCharacters.length === 0 ? (
              <option value="">No characters yet</option>
            ) : null}
            {cloneCharacters.map((character) => (
              <option key={character.id} value={character.id}>
                {character.name}
              </option>
            ))}
          </select>
          <ActionButton
            icon={<Copy />}
            title={cloning ? "Cloning..." : "Clone selected character with everything"}
            description="Deep-copies the character and every session, message, variant, embedding, and memory into an independent copy."
            onClick={() => void cloneSelectedCharacter()}
            variant="primary"
          />
        </section>

        {/* Debug Info */}
        <section className={cn("mt-8 space-y-3")}>
          <h2 className={cn(typography.h2.size, typography.h2.weight, "text-fg mb-3")}>
            {t("developer.sectionTitles.storageMaintenance")}
          </h2>
          <ActionButton
            icon={<Sparkles />}
            title={t("developer.storageMaintenance.optimizeDb")}
            description={t("developer.storageMaintenance.optimizeDbDesc")}
            onClick={optimizeDb}
            variant="primary"
          />
          <ActionButton
            icon={<Sparkles />}
            title={t("developer.storageMaintenance.backupLegacy")}
            description={t("developer.storageMaintenance.backupLegacyDesc")}
            onClick={backupLegacy}
            variant="danger"
          />

          <h2 className={cn(typography.h2.size, typography.h2.weight, "text-fg mb-3 mt-6")}>
            {t("developer.sectionTitles.usageTracking")}
          </h2>
          <ActionButton
            icon={<Calculator />}
            title={t("developer.usageTracking.recalculateAll")}
            description={t("developer.usageTracking.recalculateAllDesc")}
            onClick={recalculateUsageCosts}
            variant="primary"
          />

          <h2 className={cn(typography.h2.size, typography.h2.weight, "text-fg mb-3 mt-6")}>
            Onboarding
          </h2>
          <ActionButton
            icon={<RotateCcw />}
            title="Reset all guided tours"
            description="Clears seen-state for every onboarding tour so they replay on next visit."
            onClick={resetAllTours}
          />

          <h2 className={cn(typography.h2.size, typography.h2.weight, "text-fg mb-3 mt-6")}>
            {t("developer.sectionTitles.crashTesting")}
          </h2>
          <ActionButton
            icon={<AlertTriangle />}
            title={t("developer.crashTesting.forceCrash")}
            description={t("developer.crashTesting.forceCrashDesc")}
            onClick={forceCrash}
            variant="danger"
          />

          <h2 className={cn(typography.h2.size, typography.h2.weight, "text-fg mb-3 mt-6")}>
            {t("developer.sectionTitles.environmentInfo")}
          </h2>

          <InfoCard title={t("developer.environmentInfo.mode")} value={import.meta.env.MODE} />

          <InfoCard
            title={t("developer.environmentInfo.devMode")}
            value={import.meta.env.DEV ? "Yes" : "No"}
          />

          <InfoCard
            title={t("developer.environmentInfo.viteVersion")}
            value={import.meta.env.VITE_APP_VERSION || "N/A"}
          />
        </section>
      </main>
    </div>
  );
}

interface ActionButtonProps {
  icon: React.ReactNode;
  title: string;
  description: string;
  onClick: () => void;
  variant?: "default" | "primary" | "danger";
}

function ActionButton({
  icon,
  title,
  description,
  onClick,
  variant = "default",
}: ActionButtonProps) {
  const variants = {
    default: "border-fg/10 bg-fg/5 hover:border-fg/20 hover:bg-fg/[0.08]",
    primary: "border-info/30 bg-info/10 hover:border-info/50 hover:bg-info/20",
    danger: "border-danger/30 bg-danger/10 hover:border-danger/50 hover:bg-danger/20",
  };

  const iconVariants = {
    default: "border-fg/10 bg-fg/10 text-fg/70",
    primary: "border-info/30 bg-info/20 text-info",
    danger: "border-danger/30 bg-danger/20 text-danger/80",
  };

  return (
    <button
      onClick={onClick}
      className={cn(
        "group w-full px-4 py-3 text-left",
        radius.md,
        "border",
        variants[variant],
        interactive.transition.default,
        interactive.active.scale,
        interactive.focus.ring,
      )}
    >
      <div className="flex items-center gap-3">
        <div
          className={cn(
            "flex h-10 w-10 shrink-0 items-center justify-center",
            radius.md,
            "border",
            interactive.transition.default,
            iconVariants[variant],
          )}
        >
          <span className="[&_svg]:h-5 [&_svg]:w-5">{icon}</span>
        </div>
        <div className="min-w-0 flex-1">
          <div className={cn("truncate", typography.body.size, typography.body.weight, "text-fg")}>
            {title}
          </div>
          <div className={cn("mt-0.5 line-clamp-1", typography.caption.size, "text-fg/45")}>
            {description}
          </div>
        </div>
      </div>
    </button>
  );
}

interface InfoCardProps {
  title: string;
  value: string;
}

function InfoCard({ title, value }: InfoCardProps) {
  return (
    <div className={cn("px-4 py-3", radius.md, "border border-fg/10 bg-fg/5")}>
      <div className={cn(typography.caption.size, "text-fg/50 mb-1")}>{title}</div>
      <div className={cn(typography.body.size, "text-fg font-mono")}>{value}</div>
    </div>
  );
}
