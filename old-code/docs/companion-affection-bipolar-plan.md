# Plan: Bipolar Affection (Hate ↔ Affection)

## Motivation
User feedback (Husky110): the affection relationship axis can only run "Neutral → Affectionate", so a companion can never grow to dislike or hate the user. Affection should be able to drop below neutral into hostility.

## Decisions (confirmed)
- **Approach:** Bipolar scale. `affection` becomes `-1..+1`: negative = dislike→hostility, `0` = neutral, positive = warm→affectionate.
- **Scope:** `affection` axis only. `trust`, `closeness`, `tension`, `stability` stay `0..1`.
- **Low-end label:** `Hostile` (high end stays `Affectionate`, with `Neutral` at the center origin).

## Design constraints
- Companion features must stay additive and not alter primary (non-companion) chat. This change is fully contained inside companion relationship state.
- Backward compatible: every previously stored `affection` value is `≥ 0`, so existing data remains valid; the negative range is simply newly reachable. No migration.
- Match existing design language (design tokens, current meter styling). The only new UI is a center-origin bar variant for one axis.

---

## 1. Schema (`src/core/storage/schemas.ts`)
Widen the affection bound from `min(0)` to `min(-1)` in the three affection definitions (keep `default(0.15)`):
- `CompanionRelationshipDefaultsSchema.affection` (line ~3086)
- `CompanionRelationshipStateSchema.affection` (line ~3252)

Do **not** touch `baselineAffect.affectionIntensity` (line ~3042) — that is the soul's emotional baseline, a different `0..1` axis and out of scope.

Leave `closeness`/`trust`/`tension`/`stability` untouched.

## 2. Rust model (`src-tauri/src/chat_manager/companion/mod.rs`)

### 2a. Clamp affection signed
In `apply_*` update (lines ~459-468), `affection` currently uses `clamp01`. Switch only affection to `clamp_signed` (which clamps to `-1..1`, already defined at line ~911). Leave the other axes on `clamp01`.

### 2b. Fix the passive drift constant
Line ~463-464 adds a flat `+ 0.003` every turn (always-up creep). With a bipolar axis that makes hate impossible to sustain. Replace the flat positive bump with **drift toward the companion's configured baseline** (`config.relationship_defaults.affection`), e.g. move a small fraction of the gap toward baseline, so a neutral/hostile companion doesn't auto-warm and an affectionate one doesn't auto-cool. (closeness keeps its `+0.004` since it stays one-directional.)

### 2c. Add negative affection deltas
Today only positive-valence emotions touch `rel.affection`. Negative-valence emotions must be able to pull it down:
- `"anger" | "annoyance" | "disapproval" | "disgust"` branch (line ~793): add `rel.affection -= 0.05 * score;` (alongside the existing trust/tension/stability changes).
- Consider a smaller negative for `"sadness" | "grief" | "disappointment"`? No — distress shouldn't reduce affection (it currently *increases* closeness). Leave as-is.
- `RelationshipDelta.affection` (struct line ~384) is already an `f64`; negative values flow through naturally.

### 2d. Decay (optional, line ~435 block)
The per-elapsed-time decay only touches tension/stability today. Affection's regression toward baseline is handled by 2b on each interaction, so no change needed here unless we want time-based decay too (out of scope for v1).

### 2e. Prompt text (line ~507)
The state summary string formats `affection {:.0}%`. A negative percentage ("affection -40%") reads oddly to an LLM. Replace with a signed/semantic descriptor, e.g.:
- `affection {:+.0}%` so the sign is explicit, **or**
- a worded band: `< -0.33` → "hostile", `-0.33..0.15` → "neutral/guarded", `> 0.15` → "warm/affectionate".
Worded band is preferred for model legibility. Keep the other axes formatted as today.

### 2f. Tests
Existing test asserts `bundle.relationship_delta.affection > 0.04` for a love input (line ~949) — still valid. Add a test that an anger/disgust input yields `relationship_delta.affection < 0`, and that repeated hostile turns drive `relationship_state.affection` below 0 and clamp at `-1`.

## 3. Group chat
`group_chat_manager/mod.rs` has **no** relationship/affection handling (companion relationship state is chat-only), so no parallel change is required. Confirm with a grep during implementation; if companion state is ever wired into group chat, mirror 2a-2e there.

## 4. Shared anchors (`src/ui/pages/characters/utils/companionDefaults.ts`)
`RELATIONSHIP_AXIS_ANCHORS.affection` currently `{ low: "Neutral", high: "Affectionate" }`.
- Change to `{ low: "Hostile", high: "Affectionate" }`.
- Add an optional `mid` field to the anchor type for affection only (`mid: "Neutral"`) so the bipolar meter can label the center. Other axes leave `mid` undefined.

## 5. Meter UI (bipolar rendering)
Three components render affection meters with the same pattern: `pct = Math.round(Math.max(0, Math.min(1, value)) * 100)` and a left-anchored bar `width: ${pct}%`.
- `src/ui/pages/chats/CompanionRelationshipPage.tsx` (meter at lines ~113/137; affection axis ~367-372)
- `src/ui/pages/chats/CompanionMemoryPage.tsx` (meters ~150/203; affection ~785-788)
- `src/ui/pages/chats/components/widgets/WidgetCompanionState.tsx` (meter ~30; affection axis key ~14)

**Plan:** add a `bipolar?: boolean` (or `signed`) prop to each meter helper. When set:
- Clamp value to `-1..1` instead of `0..1`.
- Render a **center-origin** bar: the track has a center tick at 50%; a positive value fills rightward from center (warm tone, amber/accent), a negative value fills leftward from center (rose tone). Width = `Math.abs(value) * 50%`, offset from the 50% midpoint.
- Display: show a signed readout. Either signed percentage (`-40%` / `+62%`) or the worded band from 2e. Pick one and use it consistently across all three meters. (Recommend: signed percentage for the number, plus the low/mid/high anchor row underneath.)
- Anchor row: render `low … mid … high` (Hostile · Neutral · Affectionate) for the bipolar axis; keep the existing two-ended `low … high` for all other axes.
- The trend delta math (`value - baseline`) already works for negative values; the up/down arrow + rose/accent coloring still applies.

Only the affection axis passes `bipolar`; every other axis renders exactly as today (zero visual change).

## 6. i18n
- The relationship-meter anchors come from `RELATIONSHIP_AXIS_ANCHORS` (hardcoded English in `companionDefaults.ts`), **not** from locale files, so the "Hostile/Neutral/Affectionate" labels need no locale edits.
- The `affectionLow`/`affectionHigh` keys in `locales/*.ts` (line ~2228) belong to the **soul baseline-affect sliders**, a separate feature — do not repurpose them.
- If the worded band (2e) or a signed-readout label needs translation, add new keys under the companion-relationship namespace in `en.ts` (other locales fall back to en).

## 7. Edge cases / review checklist
- Creator-set starting affection: with `min(-1)`, the character editor's relationship-defaults input must accept negatives. Verify the input/slider there isn't hard-clamped to `0` in the UI (character creation companion defaults). If it is, widen it.
- Anywhere affection is read with a `?? 0.15` fallback stays correct.
- `clamp_signed` already exists; confirm it clamps to `[-1, 1]` (not `[0,1]`).
- Decay-toward-baseline (2b) must use the per-companion baseline, not a hardcoded constant, so hostile-by-design companions stay hostile.
- Confirm no analytics/export schema elsewhere asserts `affection >= 0`.

## 8. Verification
- `bun run check` (tsc + cargo check).
- New Rust unit tests (2f).
- Manual: drive a companion with hostile messages, confirm the affection meter crosses center into the rose/Hostile side in all three views, and that the prompt state line reflects hostility.

## Out of scope (v1)
- Making trust/closeness bipolar.
- Time-based affection decay.
- Bipolar treatment of the soul baseline-affect axes.

## Commit plan (when implemented)
Granular, conventional, no co-author, no code comments:
1. `feat(companion): widen affection axis to bipolar range in schema`
2. `feat(companion): allow negative affection in relationship state math`
3. `feat(companion): render affection as a center-origin bipolar meter`
(Adjust/split as needed.)
