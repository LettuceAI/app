import { motion, AnimatePresence } from "framer-motion";
import { cn, typography } from "../../design-tokens";
import { useState, useEffect, useMemo } from "react";
import { BottomMenu } from "../../components";
import {
  Sparkles,
  Zap,
  Bug,
  AlertTriangle,
  ChevronRight,
  Calendar,
  Hash,
  ArrowRight,
} from "lucide-react";

export interface ChangelogEntry {
  version: string;
  date: string;
  title?: string;
  description?: string;
  changes: {
    type: "feature" | "improvement" | "bugfix" | "breaking";
    description: string;
  }[];
}

export const changelog: ChangelogEntry[] = [
  {
    version: "2.2.3 / 2.2.3",
    date: "2026-09-02",
    title: "2.2.3 — Companion Prompt Fixes & Richer Relationship State",
    description:
      "A companion's own prompt template now wins over the session template, the relationship state handed to the model spells out what each band means alongside the raw score, and character generation works again on local models whose chat template only accepts a single system message. Gemma4 models on llama.cpp can be forced to reason before answering, and OpenRouter image generation moves to the Image API with clearer errors and automatic downsizing of oversized reference images.",
    changes: [
      {
        type: "feature",
        description:
          "A new Force Reasoning toggle in the Reasoning section of the model settings makes Gemma4-series models on the local llama.cpp engine open a thinking block before they answer. The opener is injected into the prompt automatically and the thought is routed to the reasoning view instead of leaking into the reply.",
      },
      {
        type: "improvement",
        description:
          "OpenRouter image generation now uses OpenRouter's Image API, which returns the image directly instead of wrapping it in a chat completion. Models that are not served there fall back to the chat endpoint automatically. This fixes OpenAI image models such as gpt-5.4-image-2 finishing without an image.",
      },
      {
        type: "bugfix",
        description:
          "Image generation reports the provider's own error when it arrives inside a successful response, such as an upstream 504, instead of failing with a parse error. Transient server errors are retried once, and an oversized request now explains that the reference image is too large instead of showing a bare 413.",
      },
      {
        type: "improvement",
        description:
          "Reference images sent to remote image providers are downsized to 2048 px on the longest edge and re-encoded when they are larger than 4 MB, with the inpainting mask resized to match. Upscaled sources no longer push the request past the provider's size limit.",
      },
      {
        type: "bugfix",
        description:
          "The adaptive-p sampler on llama.cpp now receives its target and decay values. They were dropped from every request, so the sampler ran without its controls.",
      },
      {
        type: "bugfix",
        description:
          "The debug view no longer shows an OpenAI endpoint for local llama.cpp requests. Unregistered providers default to a localhost URL instead of the OpenAI API.",
      },
      {
        type: "bugfix",
        description:
          "Companion mode resolves the character's companion prompt template before the session template, so an active session template no longer overwrites the companion prompt. The prompt debugger reports the same source.",
      },
      {
        type: "improvement",
        description:
          "The relationship-state line sent to the model now shows each axis as a band, a short behavioral gloss, and the raw score on its scale, instead of a single word per axis. Trust, closeness, and affection use a seven-step scale, and tension has named bands instead of a bare percentage.",
      },
      {
        type: "bugfix",
        description:
          "Smart Creator on llama.cpp merges its system entries into one leading system message, so models with strict chat templates such as Gemma no longer fail with \"System message must be at the beginning\". Remote providers keep the entries separate.",
      },
      {
        type: "bugfix",
        description:
          "Choosing dynamic memory while creating a group chat now sticks. The group is created with that memory mode instead of defaulting to manual, so there is no need to switch it again from group settings.",
      },
    ],
  },
  {
    version: "2.2.0 / 2.2.0",
    date: "2026-08-13",
    title: "2.2.0 — Local Image Generation, Transactional Sync & Companion Continuity",
    description:
      "Image generation now runs entirely on your own machine through stable-diffusion.cpp, with a three-pane Playground for iterating on prompts, LoRAs, and seeds. Device sync was replaced with a transactional engine that applies changes as a unit. Companions carry their soul, relationship, and memory across separate chats, GPU offload measures instead of guessing, and group chats got their own backend with per-character model overrides.",
    changes: [
      {
        type: "feature",
        description:
          "Local image generation: stable-diffusion.cpp runs as a managed sidecar on your own hardware, with an installer that pulls complete model bundles from HuggingFace, LoRA support, live progress, cancellation, img2img, inpainting, and upscaling.",
      },
      {
        type: "feature",
        description:
          "Playground: a three-pane image workspace with the prompt on one side, settings on the other, and a feed of past generations recording the exact seed, model, and parameters that produced each one. Random seeds are materialized at generation time, so results stay reproducible.",
      },
      {
        type: "feature",
        description:
          "LiteRouter and custom OpenAI-format image providers are supported, so any endpoint speaking the OpenAI image API works. The model selector and HuggingFace repo search now accept safetensors files, not just GGUF.",
      },
      {
        type: "improvement",
        description:
          "Device sync was replaced with a transactional engine: changes are captured, planned, and applied as a unit, so an interrupted sync no longer leaves half-applied data. Conflicts resolve last-writer-wins by original creation time across every data type.",
      },
      {
        type: "improvement",
        description:
          "Sync refuses to run between mismatched app builds instead of exchanging data it cannot interpret, and stale sync state is repaired rather than left blocking.",
      },
      {
        type: "feature",
        description:
          "Companions carry their soul, earned relationship progress, and shared memory across separate chats instead of starting over. Continuity is evidence-based: what changed is recorded with the moments that caused it, rather than a single number that could drift unexplained.",
      },
      {
        type: "bugfix",
        description:
          "Turn effects are separated from passive drift, so a specific exchange and the slow background change no longer overwrite each other, and earned relationship progress is preserved rather than recalculated away.",
      },
      {
        type: "improvement",
        description:
          "GPU offload is priced from real tensor sizes read out of the GGUF index instead of estimated from layer counts, the output layer is recognized as offloading first and costing up to ten times a normal block, and the device compute reserve is measured rather than assumed.",
      },
      {
        type: "improvement",
        description:
          "KV cache is sized from per-layer geometry, including models that mix sliding-window and full attention. Multi-GPU distribution uses real per-unit costs, and recommending a VRAM context subtracts weights already resident on the card.",
      },
      {
        type: "feature",
        description:
          "Custom sampler chains with saveable presets, plus configurable repeat penalty and penalty range. MTP models load and fit their bundled draft tensors, with configurable placement.",
      },
      {
        type: "improvement",
        description:
          "Prompt prefixes are cached across conversations, speculative decoding adapts its draft length, per-token streaming overhead was reduced, prompt microbatch size is configured separately from batch size, and native inference timings are reported.",
      },
      {
        type: "feature",
        description:
          "Group chats run on a unified conversation backend with a proper session model, per-character model overrides so any participant can run on a different model, group system prompt overrides, branching to a 1-on-1 from any message with a memory cutoff, and classic or detailed list layouts.",
      },
      {
        type: "feature",
        description:
          "Deterministic rewind: rewinding a chat restores the memory state as it was at that message, driven by a user-edit event log rather than a best-effort rebuild. Rebuilds after a rewind are bounded instead of cascading.",
      },
      {
        type: "feature",
        description:
          "Desktop navigation is yours to arrange: six layouts (bottom bar, bottom bar with labels, dock, sidebar, floating sidebar, header), a configurable header, navigation on either side or edge, reorderable items, and a custom title bar.",
      },
      {
        type: "feature",
        description:
          "NanoGPT subscription quota tracking, including usage reported during streamed completions, with quota meters colored by how close to the limit you are. Messages can be regenerated with a different model without changing the chat's model.",
      },
      {
        type: "bugfix",
        description:
          "Gemini in group chats works again: on installs where a database column was never created, every Gemini group reply failed outright. The schema is repaired on launch, and thought signatures are retained in group history the same way they are in 1-on-1 chats.",
      },
      {
        type: "bugfix",
        description:
          "More Gemini fixes: requested image aspect ratios are honored, v1beta base URLs are preserved, raw tool call names survive round-trips, tool declarations are cached with context, and unsupported image edit inputs are rejected with a clear error.",
      },
      {
        type: "bugfix",
        description:
          "Ollama models with strict templates no longer fail on stray system messages, lorebook triggers match keywords in languages without spaces, OpenRouter Gemini completion costs can no longer come out negative, and Android no longer uses an uninitialized platform TLS verifier.",
      },
      {
        type: "bugfix",
        description:
          "TTS on Windows no longer flashes espeak console windows and phonemizes in a single spawn, cancelled swipe replies stay after their prompt, Smart Creator preserves drafts when you open an editor, and sd.cpp shuts down with the app instead of surviving as an orphaned process.",
      },
    ],
  },
  {
    version: "2.1.1 / 2.1.1",
    date: "2026-07-04",
    title: "2.1.1 — Device Sync Rebuilt, Multi-GPU Clarity & Guided Tours",
    description:
      "Device sync got a ground-up reliability rebuild: conflicts resolve the right way, settings and memories arrive intact, transfers have no size ceiling, and failures say so instead of pretending everything worked. Multi-GPU stopped being a guessing game, five new guided tours walk you through the trickiest parts of the app, and ASR learning data joins sync and backups.",
    changes: [
      {
        type: "bugfix",
        description:
          "Sync conflict resolution now keeps the newest data instead of the oldest. Previously a fresh install could silently overwrite the host device's real settings, advanced settings, and prompt templates with its own defaults.",
      },
      {
        type: "bugfix",
        description:
          "Memory metadata survives sync: importance, categories, timestamps, and embedding versions arrive intact instead of being stripped to bare text.",
      },
      {
        type: "improvement",
        description:
          "Large libraries sync in chunks with no more transfer size ceiling, and each data domain is applied and finalized as it completes.",
      },
      {
        type: "bugfix",
        description:
          "A sync that fails or loses connection mid-transfer now reports a real error instead of completing silently with partial data.",
      },
      {
        type: "feature",
        description:
          "Companion data, creation helper drafts, and ASR learning data (custom vocabulary, corrections, dismissed suggestions) now sync between devices. ASR learning data is also included in backups.",
      },
      {
        type: "improvement",
        description:
          "The post-sync embedding model prompt now also triggers when the synced data itself contains memories, even if settings arrive misconfigured.",
      },
      {
        type: "bugfix",
        description:
          "A leftover single-GPU pin can no longer silently disable multi-GPU. Enabling multi-GPU at the same or broader level takes precedence, while a deliberate per-model pin still wins where intended.",
      },
      {
        type: "improvement",
        description:
          "The model editor shows the effective multi-GPU state: distribution controls appear for models inheriting the global default, single-GPU controls hide when they do not apply, and models still pinned to one GPU get a notice with a one-tap Remove pin.",
      },
      {
        type: "improvement",
        description:
          "Model browser installs persist your offload intent (auto, CPU, GPU, mixed) instead of a hardware-specific layer count, so VRAM you add later is actually used.",
      },
      {
        type: "improvement",
        description:
          "Deleting a model file from Installed Models warns when a configured model, vision adapter, MTP draft, or global default still references it, listing exactly what would break, without blocking the delete.",
      },
      {
        type: "feature",
        description:
          "Five new guided tours: the local model editor, runtime defaults, the model browser's recommendation panel, group chats, and dynamic memory now walk you through themselves on first visit, in every supported language.",
      },
      {
        type: "improvement",
        description:
          "The tour engine scrolls partially visible targets into view, top-aligns targets taller than the screen, and places its card beside full-height panels instead of on top of them.",
      },
      {
        type: "improvement",
        description:
          "Model loading shows a single toast with per-GPU progress bars, and toggling global multi-GPU offers to reconfigure existing models in one step.",
      },
      {
        type: "bugfix",
        description:
          "The zAI (GLM) provider works now: requests were sent to a nonexistent endpoint, so every call failed. Chat, the thinking toggle, and API key verification all target the real Z.AI API. Existing zAI providers with a regular API key need the base URL changed to https://api.z.ai/api/paas/v4 (coding-plan keys work as-is).",
      },
      {
        type: "bugfix",
        description:
          "Companion souls are written by the Soul Writer model again: the character's own model no longer silently overrides the model chosen in Settings, and the soul step shows the model that will actually write.",
      },
      {
        type: "bugfix",
        description:
          "Desktop back navigation no longer strands you on the settings root, the mobile settings back flow was corrected, and Sprout hardware reports are validated before scoring runnability.",
      },
    ],
  },
  {
    version: "2.1.0 / 2.1.0",
    date: "2026-07-02",
    title: "2.1 — Multi-GPU Local Models, Performance Metrics & Smarter Providers",
    description:
      "Run local models across every GPU in your machine with automatic layer distribution and KV cache placement, and watch a new performance dashboard track tokens per second on every generation. Pin OpenRouter to a specific provider endpoint, probe remote Ollama hardware for real runnability, and export chats in SillyTavern's own jsonl format. Plus a shared memory cycle hub across chat and group pages, and a long wave of memory, embedding, and backup reliability work.",
    changes: [
      {
        type: "feature",
        description:
          "Multi-GPU local models: llama.cpp now distributes a model's layers across every selected GPU, with automatic or manual splits, KV cache placement modes, a main-GPU pin, and per-GPU VRAM reservation so nothing overflows.",
      },
      {
        type: "feature",
        description:
          "Added a per-model single-GPU device override for choosing exactly which GPU runs a model.",
      },
      {
        type: "feature",
        description:
          "Performance metrics: a new local-LLM performance page graphs tokens per second and prompt/generation timing per run and across runs, with a per-message action to see exactly how any reply was produced (including in group chats).",
      },
      {
        type: "feature",
        description:
          "Per-message MTP stats: speculative-decoding acceptance and draft stats are now persisted and shown on each message.",
      },
      {
        type: "feature",
        description:
          "OpenRouter provider pinning: pick a specific provider endpoint per model, with live pricing, cache rates, uptime, and provider logos, and route every request exclusively through it.",
      },
      {
        type: "feature",
        description:
          "Sprout hardware probe: remote Ollama runnability is now judged against the real hardware behind the endpoint instead of a guess.",
      },
      {
        type: "feature",
        description:
          "SillyTavern-compatible export: chat export and import now speak the official SillyTavern jsonl format, so histories move cleanly in and out.",
      },
      {
        type: "feature",
        description:
          "Shared memory cycle hub: a single unified memory-cycle panel across both chat and group memory pages.",
      },
      {
        type: "feature",
        description:
          "Mobile model browser: the HuggingFace browser now works properly on phones. It pairs with a remote Ollama provider (auto-selected when you have one) and pulls GGUF models straight to your host, with files and recommended settings in a slide-in drawer.",
      },
      {
        type: "improvement",
        description:
          "Multi-GPU distribution and KV modes are explained inline with bottom-menu pickers, fully localized in the model editor.",
      },
      {
        type: "improvement",
        description:
          "Installed models on mobile now focuses on Ollama, with a reorganized toolbar, an inline provider selector, and cleaner model rows.",
      },
      {
        type: "improvement",
        description:
          "The download destination picker was redesigned as a grouped list, your destination choice now persists while you browse, and the real Ollama logo shows across Ollama surfaces on desktop and mobile.",
      },
      {
        type: "improvement",
        description:
          "Added a warning when a manual layer split exceeds a device's reported VRAM before loading, and a notice when vision disables MTP.",
      },
      {
        type: "improvement",
        description:
          "OpenRouter requests now send the roleplay category and current app-attribution headers.",
      },
      {
        type: "improvement",
        description:
          "Backup and sync now preserve current data instead of overwriting it, and companion mode data is preserved in export and config settings.",
      },
      {
        type: "improvement",
        description:
          "Explicit dynamic-memory model selection is preserved instead of falling back to a default, and shared-memory owner resolution honors companion character mode.",
      },
      {
        type: "improvement",
        description:
          "ONNX Runtime is now bundled on Windows, with Kokoro routed through a shared runtime init.",
      },
      {
        type: "bugfix",
        description:
          "Added a smart multi-GPU offloader that spreads large models across all your hardware, sizing each device's share automatically and keeping far more of the model on the GPU instead of falling back to the CPU.",
      },
      {
        type: "bugfix",
        description:
          "Per-GPU KV cache VRAM is reserved when distributing layers, and immediate aborts skip model load by checking the abort signal before the engine starts.",
      },
      {
        type: "bugfix",
        description:
          "Fixed a re-embedding loop, de-duplicated lorebook scans and embedding-preference reads, and stamped embedding signatures from the actual vector to stop spurious vector migrations.",
      },
      {
        type: "bugfix",
        description:
          "Embedding migration markers now survive the session schema round-trip, and the embedding tokenizer is resolved from the version-specific file instead of a hardcoded one.",
      },
      {
        type: "bugfix",
        description:
          "Upgrading from 2.0.0 silently installs the v4 embedding tokenizer once if the model was present without it, so token counting resolves after the update.",
      },
      {
        type: "bugfix",
        description:
          "Windows are retried after a dynamic-memory failure instead of being skipped.",
      },
      {
        type: "bugfix",
        description:
          "macOS keeps native decorations and an opaque window so the traffic lights and rounded corners render correctly.",
      },
      {
        type: "bugfix",
        description:
          "A per-conversation chat appearance override no longer injects defaults that clobber global settings, and Discovery card images load from the new ct-cards storage host.",
      },
    ],
  },
  {
    version: "2.0.0 / 2.0.0",
    date: "2026-06-27",
    title: "2.0 — Living Companion Souls, Director Group Chats, Time Awareness & a Reinvented Desktop",
    description:
      "The largest LettuceAI release yet. Companions gain a living \"soul\" that grows from your conversations, a bipolar relationship model where warmth is earned, and full time awareness with an in-app clock you can override. Group chats get a hands-on Director mode, a participants bar, per-group appearance, and message search. The desktop app gets a custom title bar, rounded corners, and frameless resize. Plus MTP speculative decoding and the XTC sampler for local models, a guided bring-your-own-key onboarding, branch-tree navigation, two new providers, full localization, and a long tail of reliability work.",
    changes: [
      {
        type: "feature",
        description:
          "Companion Souls: companions now carry a persistent soul the app writes and evolves over time — traits, backstory, appearance, goals, likes, and fears, each on its own mutability tier, including a very-slow tier with an overlay-rendered core.",
      },
      {
        type: "feature",
        description:
          "Added a growth-cycle engine that grows and supersedes the soul from your new memories, plus a consolidation pass that merges accumulated growth, and a soul growth viewer to review, clear, or delete individual entries.",
      },
      {
        type: "feature",
        description:
          "The soul generator now streams its output live and can be aborted mid-run.",
      },
      {
        type: "feature",
        description:
          "Earned relationships: closeness, trust, and affection are now bipolar (they can go negative), shown as center-origin meters, and updated with a leaky, asymmetric, saturating model with worded prompt bands so warmth is earned slowly and erodes naturally.",
      },
      {
        type: "feature",
        description:
          "Time awareness: give a companion a sense of now by overriding the in-chat clock (freeze it, set a custom time, or let it tick). It feeds prompt time, temporal memory queries, and memory stamps, and is also exposed as a time widget.",
      },
      {
        type: "feature",
        description:
          "Replaced the native date/time control with a custom in-app picker: typeable hour/minute, a clickable month/year selector, wrapping time steppers, and a max year raised to the JS date ceiling.",
      },
      {
        type: "feature",
        description:
          "Memories now show live relative time, are instructed to store absolute dates, and you can set or clear a memory's date with the picker. Dynamic memories can supersede older, outdated ones instead of piling up.",
      },
      {
        type: "feature",
        description:
          "Director group chats: tap a character's avatar to choose who replies next. The selection drives the send button (no separate confirm/cancel), with action and cue styles, a configurable hint position, a sticky last pick, and a wiggle nudge if you send with nobody selected.",
      },
      {
        type: "feature",
        description:
          "Added a group participants bar with per-character mention toggle, mute, and appearance controls, plus avatar-shape options and a solid/fading/transparent bar background.",
      },
      {
        type: "feature",
        description:
          "Per-group chat appearance: a dedicated editor (desktop drawer and mobile page) backed by group-level appearance overrides.",
      },
      {
        type: "feature",
        description:
          "Added group message search with a header button and jump-to-message that loads older messages from the database when needed, per-session author notes with an inline editor, desktop chat widgets fed real group data with a per-widget character picker, and per-message dynamic memory references.",
      },
      {
        type: "feature",
        description:
          "Desktop gets a custom title bar with selectable designs, position, and size, rounded window corners, and edge-resize handles for the frameless window.",
      },
      {
        type: "feature",
        description:
          "Added inline desktop search on Discovery with tag search, a pure-mode blur, and infinite scroll.",
      },
      {
        type: "feature",
        description:
          "MTP speculative decoding for local models with bundled and external draft models, support for Gemma 4 shared-assistant drafters, and early-stop when draft confidence drops.",
      },
      {
        type: "feature",
        description:
          "Added a per-model Full SWA Cache (swa-full) toggle and the new XTC (Exclude Top Choices) sampler, off by default.",
      },
      {
        type: "feature",
        description:
          "Branch-tree navigation with a lineage view, branch comparison, and fork markers, plus a parent-branch confirm menu and child-fork indicators in chat.",
      },
      {
        type: "feature",
        description:
          "Redesigned onboarding: a guided bring-your-own-key setup with a plain-language car metaphor, a free/paid choice, and screenshot-driven Gemini and OpenRouter flows, finishing with a one-tap embedding-memory download and a handoff into the in-app tour.",
      },
      {
        type: "feature",
        description:
          "Added the Gemini Agent Platform (Express) and LiteRouter providers.",
      },
      {
        type: "feature",
        description:
          "Dynamic memory run modes: an ask-first approval menu and a manual-gating setting, so memory updates can require your confirmation.",
      },
      {
        type: "feature",
        description:
          "Audio input: upload, play back, and track token usage for audio in chat, plus a new Audio library tab listing TTS and chat-uploaded audio with player cards and a per-card actions menu.",
      },
      {
        type: "feature",
        description:
          "Inline scene images rendered in chat, with image/GIF insert in the scene editors and an in-bubble indicator while a scene image prompt streams.",
      },
      {
        type: "feature",
        description:
          "Reorganized Accessibility settings into a Customization page, added a custom chat input color with adaptive contrast, a resizable and collapsible appearance drawer, and a dice roll with editable notation in the plus menu.",
      },
      {
        type: "feature",
        description:
          "Model browser author profiles with an author filter and in-profile search, configurable model folders with atomic migration, and a local runtime-defaults page.",
      },
      {
        type: "feature",
        description:
          "Moved Kokoro voice setup into Voice providers with a guided download menu, and surfaced documentation links across the app with a completed docs map.",
      },
      {
        type: "improvement",
        description:
          "Local-model runnability scoring now accounts for MTP next-n layers and QAT quants, and recommended KV cache is capped at Q8.",
      },
      {
        type: "improvement",
        description:
          "Memory planning now accounts for sidecar memory, and MTP fields are forwarded through the provider extra-body allowlist, including in group chats.",
      },
      {
        type: "improvement",
        description:
          "Added a Go to Models shortcut and warnings before deleting a prompt or model that is in use.",
      },
      {
        type: "improvement",
        description:
          "Routed frontend user-facing strings through the locale system and translated the app across roughly 20 languages.",
      },
      {
        type: "improvement",
        description:
          "Bulk update of protected prompts to the latest versions with full auto-refresh coverage and reset logging.",
      },
      {
        type: "improvement",
        description:
          "Android backups are now written to Downloads via MediaStore and indexed for list and delete.",
      },
      {
        type: "bugfix",
        description:
          "A turn save can no longer clobber concurrently-changed time preferences; the time override is preserved across state round-trips and kept synced to the live clock, with no more empty field or ticking jump-back.",
      },
      {
        type: "bugfix",
        description:
          "Models no longer echo system timestamps back into their replies.",
      },
      {
        type: "bugfix",
        description:
          "Partial vector migrations are now persisted so an aborted re-embed stops looping, and embedding failures are surfaced in the logs instead of failing silently.",
      },
      {
        type: "bugfix",
        description:
          "The companion temporal filter is only applied when time awareness is enabled.",
      },
      {
        type: "bugfix",
        description:
          "Group chat fixes: no more panic on a non-char-boundary log preview slice; continue no longer impersonates the persona or duplicates the last user turn; a null chat-appearance no longer breaks group parsing or hides the appearance button; and saving appearance no longer reloads the whole chat.",
      },
      {
        type: "bugfix",
        description:
          "Every group session is now connected to a source group, with orphans backfilled, and scene messages are labeled correctly in the actions sheet.",
      },
      {
        type: "bugfix",
        description:
          "Chat appearance and widget data are preserved when editing a character, the forceSendThinkingState setting persists correctly, and sidebar provider/model counts refresh on in-app changes.",
      },
      {
        type: "bugfix",
        description:
          "Fullscreen overlays are correctly offset below the custom desktop title bar, and the corner toggle is guarded across decoration changes.",
      },
      {
        type: "bugfix",
        description:
          "Fixed a doubled API version in the Gemini image URL and centered the image viewer when there is no prompt.",
      },
      {
        type: "bugfix",
        description:
          "A stale Kokoro asset root now self-heals from synced platforms, and the real export/import error is surfaced instead of a generic one.",
      },
      {
        type: "bugfix",
        description:
          "Decreasing relationship values render in a danger color, and deleting soul growth now asks for confirmation without a jarring full-page reload.",
      },
    ],
  },
  {
    version: "1.9.0 / 1.6.0",
    date: "2026-05-31",
    title: "Chat Widgets, a Live-Preview Appearance Drawer & Companion Memory Tools",
    description:
      "A big chat-customization release. It introduces a full Chat Widgets system for building a custom side panel next to your conversation, a redesigned side-anchored appearance drawer with live preview and desktop column controls, hands-on companion memory tooling with manual processing and live output, a flatter rebuilt model editor, plus smarter local-model thinking, anti-loop dynamic memory, and a long tail of reliability fixes.",
    changes: [
      {
        type: "feature",
        description:
          "Added Chat Widgets: build a custom panel beside your conversation from composable widgets (character and persona info, scratch pad, image, stat tracker, memory, companion state, quick snippets, dice, session info, author note, and layout pieces). Edit in place with a sticky toolbar, drag-to-reorder, an Add-widget picker, per-widget design variants, a real library image picker, and cross-column moves; layouts are saved per character.",
      },
      {
        type: "feature",
        description:
          "Moved chat appearance into a side-anchored drawer you can open from the chat header, with a live preview that updates as you tweak, plus a tabbed shared form, side-flip, and a message-actions entry.",
      },
      {
        type: "feature",
        description:
          "Added desktop chat layout controls for column width, alignment, and full-shell behavior, independent header and footer toggles, a center widget mode, and a draggable divider to resize the widget area. Group chats mirror the same settings.",
      },
      {
        type: "feature",
        description:
          "Added companion memory tools: trigger a memory-processing cycle manually, watch it run with a progress bar and live output viewer (with cancel), and review and edit the generated context summary inline.",
      },
      {
        type: "feature",
        description:
          "Redesigned the model editor with a flatter, box-free layout, unified section tabs, a runtime-report drawer, and a width-aware desktop and clean mobile presentation.",
      },
      {
        type: "feature",
        description:
          "Improved local-model thinking with a force-send thinking-state toggle and recognition of Gemma channel-style reasoning tags.",
      },
      {
        type: "feature",
        description:
          "Added anti-loop sampling to dynamic memory to reduce repetition loops, with live visibility into generation.",
      },
      {
        type: "feature",
        description:
          "Added per-message info: optionally show the model that generated each message, input/output/total token counts, time-to-first-token, and tokens/sec, each toggleable, with a choice of placement and text size.",
      },
      {
        type: "improvement",
        description: "Added optional author name and timestamp headers above messages.",
      },
      {
        type: "improvement",
        description: "Added support for image-only OpenRouter models.",
      },
      {
        type: "improvement",
        description:
          "Made the Help me Reply history window configurable so it can look back as far as you want.",
      },
      {
        type: "improvement",
        description:
          "Companion relationship meters now show low and high anchor labels for context.",
      },
      {
        type: "improvement",
        description:
          "The scroll-to-bottom button now tracks the composer height as it grows and anchors to the messages column when widgets are shown.",
      },
      {
        type: "improvement",
        description:
          "Sharpened local-model runnability scoring with MoE active-path awareness, an expanded quantization table, KV cache quant types, and a repaired GGUF parser.",
      },
      {
        type: "improvement",
        description: "Added a direct Save action to the unsaved-changes toast.",
      },
      {
        type: "improvement",
        description:
          "Local-model performance metrics (time-to-first-token and tokens/sec) are now saved with each message and shown in message details after a reload, in both direct and group chats.",
      },
      {
        type: "improvement",
        description:
          "Chat background blur is now applied to the image directly, dropping the separate bubble-blur control for a cleaner result.",
      },
      {
        type: "improvement",
        description:
          "The chat settings drawer now saves and updates the session immediately after changing a value.",
      },
      {
        type: "bugfix",
        description:
          "Streaming messages now apply the chat appearance settings (such as the author name and timestamp header) while generating, instead of only after the message finishes.",
      },
      {
        type: "bugfix",
        description:
          "Fixed identity placeholders leaking into injected memories, lorebook entries, and summaries.",
      },
      {
        type: "bugfix",
        description:
          'Fixed the model selector\'s "only free models" toggle colliding with the title on mobile.',
      },
      {
        type: "bugfix",
        description:
          "Companion memory now allows companion categories on edit and stops placeholder leakage.",
      },
      {
        type: "bugfix",
        description:
          "llama.cpp now drops the existing model before reload, avoiding double-pinned VRAM.",
      },
      {
        type: "bugfix",
        description:
          "Local models now load more reliably with GPU offload. Context sizing accounts for layers offloaded to the GPU, so a model that runs fine with mixed CPU and GPU offload is no longer wrongly reported as too big to fit.",
      },
      {
        type: "bugfix",
        description:
          "Improved llama.cpp VRAM headroom estimates so context creation no longer fails with out-of-memory on partially offloaded models. The compute-buffer reserve is derived from the model's dimensions and batch size, and a context that hits OOM is retried at a smaller size even when a KV cache type is set.",
      },
      {
        type: "bugfix",
        description: "Cleaned up orphaned memory embeddings and repaired the embeddings migration.",
      },
      {
        type: "bugfix",
        description: "Made the speech-recognition migration idempotent.",
      },
      {
        type: "bugfix",
        description: "The creation helper can now use llama.cpp models.",
      },
      {
        type: "bugfix",
        description: "Settings are now reloaded after successful syncs.",
      },
      {
        type: "bugfix",
        description: "The reset flow removes Whisper and Kokoro models.",
      },
      {
        type: "bugfix",
        description:
          "The group chat memories page now renders properly when a chat background image is used.",
      },
    ],
  },
  {
    version: "1.8.2 / 1.5.2",
    date: "2026-05-23",
    title: "Character Creation, Navigation & Sync Polish",
    description:
      "A polish release that brings design references into the character creation flow, fixes navigation loops in chat templates, smooths sync onboarding around embedding prompts and local model requirements, and resolves a handful of UI and platform-specific bugs across the avatar picker, image generator, and what's-new drawer.",
    changes: [
      {
        type: "feature",
        description:
          "Added an in-app Help & FAQ page covering BYOK, API keys, free vs paid providers, tokens, privacy, and common setup questions, with a shortcut from onboarding for new users.",
      },
      {
        type: "improvement",
        description:
          "Replaced raw HTTP error strings in chats and group chats with a friendly explainer that names the problem (rate limit, out of credits, model not found, content blocked, provider down, etc.), suggests a fix, and keeps the raw error one tap away.",
      },
      {
        type: "feature",
        description:
          "Added design references (visual description and reference images) to the character create flow so scene generation can stay on-model from the start.",
      },
      {
        type: "improvement",
        description:
          "Unified the local model requirements prompt across sync onboarding so the embedding check is presented consistently.",
      },
      {
        type: "improvement",
        description:
          "Aligned the reasoning header and toggle styling for a more consistent message presentation.",
      },
      {
        type: "improvement",
        description: 'Redirected the Settings "Convert Files" entry to lettuceai.app/convert.',
      },
      {
        type: "bugfix",
        description:
          "Fixed the chat templates back arrow looping between Templates and Settings; the editor now returns to the template list and the list returns to character edit.",
      },
      {
        type: "bugfix",
        description:
          "Stopped the banner avatar picker from overflowing narrow containers by making the small/medium sizes fluid.",
      },
      {
        type: "bugfix",
        description: "Restored the missing chat template options on mobile.",
      },
      {
        type: "bugfix",
        description:
          'Corrected the misleading "Continue to Starting Scenes" button label shown while already on the Starting Scenes step.',
      },
      {
        type: "bugfix",
        description:
          "Defaulted the summarisation model correctly during onboarding and runtime so dynamic memory works out of the box.",
      },
      {
        type: "bugfix",
        description:
          "Applied the user's custom TLS trust store to image-generation requests so self-signed endpoints work like the rest of the app.",
      },
      {
        type: "bugfix",
        description:
          "Surfaced the embedding-model prompt after a sync completes when the local model requirement isn't met yet.",
      },
      {
        type: "bugfix",
        description:
          "Reapplied the sync Ready handshake and post-sync completion fixes that were lost in a previous merge.",
      },
      {
        type: "bugfix",
        description:
          "Respected the device safe area inside the what's-new drawer so content no longer sits under the notch or home indicator.",
      },
    ],
  },
  {
    version: "1.8.1 / 1.5.1",
    date: "2026-05-20",
    title: "Startup, Import & Settings Stability",
    description:
      "A focused stability release that tightens local model startup behavior, repairs Android chat imports, prevents memory migration from hanging behind permanent progress UI, improves download progress visibility, and refreshes chat settings from the latest saved character state.",
    changes: [
      {
        type: "bugfix",
        description:
          "Stopped GPU-optimal context warnings from appearing in Model Browser when KV cache is explicitly set to RAM.",
      },
      {
        type: "bugfix",
        description:
          "Fixed Android JSONL chat imports by supporting document-picker content URIs instead of assuming filesystem paths.",
      },
      {
        type: "bugfix",
        description:
          "Fixed stalled dynamic memory vector migrations by timing out stuck embedding work and dismissing the migration toast cleanly on failure.",
      },
      {
        type: "improvement",
        description:
          "Added estimated time remaining to active download queue cards and repaired missing locale-backed queue labels.",
      },
      {
        type: "bugfix",
        description:
          "Stopped local model startup toasts from reappearing when llama.cpp reuses an already loaded model.",
      },
      {
        type: "bugfix",
        description:
          "Fixed chat settings drawers reopening with stale persona and model quick-setting values by reloading the latest saved character by ID.",
      },
    ],
  },
  {
    version: "1.8.0 / 1.5.0",
    date: "2026-05-18",
    title: "Sync Onboarding, Banner Cards & Shared Memory",
    description:
      "A feature-heavy release that adds device-to-device sync onboarding with readiness checks, introduces a new persisted banner card system for characters, expands companion memory with shared state and scheduled notes, broadens provider and TTS coverage, and hardens migrations, imports, backups, and chat appearance persistence.",
    changes: [
      {
        type: "feature",
        description:
          "Added a new device-to-device sync onboarding flow with embedding checks and a readiness handshake before transfer begins.",
      },
      {
        type: "feature",
        description:
          "Added a first-class banner character card system with a persisted card type, dedicated banner media asset, separate crop state, editor controls, and chat-list rendering path.",
      },
      {
        type: "feature",
        description:
          "Added shared companion memory across sessions, scheduled notes, and custom summarizer and memory-manager prompts.",
      },
      {
        type: "feature",
        description:
          "Added Fish cloud and local audio TTS providers, plus Cerebras and Pollinations provider integrations.",
      },
      {
        type: "improvement",
        description:
          "Reworked desktop settings surfaces, including provider editing, prompt editors, chat appearance tooling, and reusable numeric inputs.",
      },
      {
        type: "improvement",
        description:
          "Improved chat and library performance with deferred gradients, cached list state, lazy avatar loading, and faster page-load behavior.",
      },
      {
        type: "improvement",
        description:
          "Replaced chatpkg zip export with SillyTavern JSONL and preserved banner crop metadata across imports and character transfers.",
      },
      {
        type: "bugfix",
        description:
          "Hardened sync, backups, and character loading by preserving memory embeddings and advanced settings while skipping invalid stored characters safely.",
      },
      {
        type: "bugfix",
        description:
          "Repaired missing character schema fields on migration and startup, including banner crop columns and newer character metadata.",
      },
      {
        type: "bugfix",
        description:
          "Fixed chat appearance persistence and background styling regressions, including stale per-character overrides after reset-and-save flows.",
      },
    ],
  },
  {
    version: "1.7.2 / 1.4.1",
    date: "2026-05-11",
    title: "Provider Leak Fixes, Groq Compatibility & Linux Packaging",
    description:
      "A hotfix release that plugs internal request metadata from leaking to providers, fixes Groq's model listing and ships its logo, restores Android TLS by reverting to bundled rustls roots, re-enables text tool-call parsing on mobile, and lights up Linux distribution via Flatpak, AUR, and Debian repo publishing workflows.",
    changes: [
      {
        type: "bugfix",
        description: "Stopped internal request metadata from leaking into provider payloads.",
      },
      {
        type: "bugfix",
        description: "Stripped visible chat metadata from outbound provider messages.",
      },
      {
        type: "bugfix",
        description: "Fixed Groq's model listing to use the OpenAI-compatible endpoint.",
      },
      {
        type: "feature",
        description: "Added the Groq provider logo.",
      },
      {
        type: "bugfix",
        description:
          "Restored Android networking by reverting reqwest to bundled rustls-tls roots.",
      },
      {
        type: "bugfix",
        description: "Re-enabled text-based tool-call parsing on mobile builds.",
      },
      {
        type: "feature",
        description:
          "Added Linux package publishing workflows for Flatpak, AUR, and Debian repository.",
      },
      {
        type: "improvement",
        description:
          "Provider message construction is now centralized for metadata-scrubbing consistency.",
      },
    ],
  },
  {
    version: "1.0-beta.6.1",
    date: "2025-12-23",
    changes: [
      {
        type: "bugfix",
        description:
          "Fixed multiple issues in the backup system where not all data was being saved correctly.",
      },
      {
        type: "bugfix",
        description:
          "Fixed characters losing context and behaving clueless after being restored from backups.",
      },
      {
        type: "bugfix",
        description: "Fixed OpenRouter and MistralAI reasoning support not functioning correctly.",
      },
      {
        type: "bugfix",
        description: "Fixed backups containing images failing to load properly.",
      },
      {
        type: "feature",
        description: "Added support for Ollama endpoints.",
      },
      {
        type: "feature",
        description: "Added support for LM Studio endpoints.",
      },
      {
        type: "feature",
        description:
          "Added custom endpoint support using OpenAI-compatible or Anthropic-compatible formats.",
      },
      {
        type: "improvement",
        description:
          "Increased request timeout from 2 minutes to 15 minutes to improve stability for long-running and local inference requests.",
      },
    ],
  },
  {
    version: "1.0-beta.6",
    date: "2025-12-21",
    changes: [
      {
        type: "feature",
        description:
          "In-chat image generation is now supported, allowing images to be generated directly inside conversations.",
      },
      {
        type: "feature",
        description:
          "Introduced the Lorebook system for structured world, character, and knowledge injection.",
      },
      {
        type: "feature",
        description: "Added support for the Chutes API endpoint.",
      },
      {
        type: "feature",
        description:
          "Added an OpenAI-compatible API endpoint with extensive customization options, including user/assistant role names and chat completion behavior.",
      },
      {
        type: "feature",
        description: "Introduced Reasoning support for models that expose reasoning tokens.",
      },
      {
        type: "feature",
        description: "Added a new Chat Settings panel for easier per-chat configuration.",
      },
      {
        type: "feature",
        description: `Users can now use "Rewind to here" on user messages, allowing conversations to be resumed from any previous user turn.`,
      },
      {
        type: "feature",
        description: "Added lorebook management tools to the Library page.",
      },
      {
        type: "feature",
        description: "Added the ability to create lorebooks directly from the Home page.",
      },
      {
        type: "improvement",
        description:
          "Introduced Dynamic Memory v2 - a faster, more responsive, and significantly more accurate memory system with higher capacity.",
      },
      {
        type: "improvement",
        description:
          "Added a new embedding model that is ~50% smaller, runs faster, and supports up to 4096 tokens (previously 512).",
      },
      {
        type: "improvement",
        description:
          "Introduced an experimental 'Context Enrichment' feature to enhance memory queries using the new embedding model.",
      },
      {
        type: "improvement",
        description:
          "Character Cards have been redesigned for improved clarity, visual consistency, and information hierarchy.",
      },
      {
        type: "improvement",
        description:
          "The Chat Header memory button now clearly displays memory status and the amount of memory currently in use.",
      },
      {
        type: "improvement",
        description: "Improved UI consistency across chat, settings, and character screens.",
      },
      {
        type: "improvement",
        description:
          "Refined spacing, typography, and interaction feedback for a more cohesive UI experience.",
      },
      {
        type: "improvement",
        description:
          "Improved overall navigation clarity and reduced visual noise in frequently used views.",
      },
      {
        type: "improvement",
        description: "Backup system robustness has been significantly improved.",
      },
      {
        type: "improvement",
        description:
          "Chat history layout has been redesigned for better consistency and readability.",
      },
      {
        type: "improvement",
        description: "Long chats now load up to ~8x faster.",
      },
      {
        type: "improvement",
        description:
          "Character list on the homepage has been optimized for faster loading and smoother scrolling.",
      },
      {
        type: "improvement",
        description:
          "Internal state handling and caching logic have been improved for better stability and performance.",
      },
      {
        type: "bugfix",
        description: "Fixed an issue where Dynamic Memory could get stuck after cycle 2.",
      },
      {
        type: "bugfix",
        description: "Fixed an app freeze caused by loading invalid or corrupted backup files.",
      },
      {
        type: "bugfix",
        description: "Fixed an incorrect Google API endpoint URL.",
      },
    ],
  },
  {
    version: "1.0-beta_5",
    date: "2025-12-08",
    changes: [
      {
        type: "feature",
        description:
          "Added gradient overwrite customization. users can now define up to 3 custom colors for avatar gradients",
      },
      {
        type: "feature",
        description:
          "Introduced multimodel support, allowing users to mix different LLMs within the same environment",
      },
      {
        type: "feature",
        description:
          "Users can now generate images for avatars directly through supported image models",
      },
      {
        type: "feature",
        description: "Users can now send images in chat when using models that support image input",
      },
      {
        type: "feature",
        description: "Added session-level message search for quickly locating past messages",
      },
      {
        type: "feature",
        description:
          "Users can now reposition and resize their character/persona avatars, and also crop/adjust chat background images",
      },
      {
        type: "feature",
        description:
          "Encrypted backup system added. users can now export and restore their data securely",
      },
      {
        type: "feature",
        description:
          "Added empty-state UI to the Library page for better feedback when no content exists",
      },
      {
        type: "feature",
        description:
          "Chat branching added. users can create alternate timeline conversations and branches with different characters",
      },
      {
        type: "improvement",
        description:
          "Added new template placeholders {{context_summary}} and {{key_memories}} to improve prompt customization",
      },
      {
        type: "improvement",
        description: "Redesigned the Chat Character Card layout for clarity and better hierarchy",
      },
      {
        type: "improvement",
        description:
          "Redesigned the Edit Character page using a tabbed layout for more structured navigation",
      },
      {
        type: "improvement",
        description:
          "Revamped the Character/Persona Avatar editor for more intuitive customization",
      },
      {
        type: "improvement",
        description:
          "Persona creation page redesigned to match the Create Character page’s visual language",
      },
      {
        type: "improvement",
        description:
          "Dynamic memory system is now significantly more responsive and less prone to stalling",
      },
      {
        type: "improvement",
        description:
          "Navigation state handling rewritten — now more predictable, stable, and resistant to edge-case desyncs",
      },
      {
        type: "improvement",
        description:
          "Large internal performance pass: reduced unnecessary re-renders, optimized SQLite queries, minimized prop-drilling overhead, and improved store subscription batching",
      },
      {
        type: "bugfix",
        description:
          "Fixed a bug where skipping the welcome page could cause a database panic during initialization",
      },
      {
        type: "bugfix",
        description: "Summaries are now correctly included in API requests as intended",
      },
      {
        type: "bugfix",
        description: "Chat branches now generate proper unique IDs",
      },
    ],
  },
  {
    version: "1.0-beta_4",
    date: "2025-11-23",
    changes: [
      {
        type: "improvement",
        description:
          "Completely redesigned the top navigation bar to feel more modern and aligned with common mobile UI patterns",
      },
      {
        type: "improvement",
        description:
          "Moved Settings from the bottom navigation to the top navigation for a more predictable app layout",
      },
      {
        type: "feature",
        description:
          "Replaced the Settings slot in the bottom navigation with a new Library section",
      },
      {
        type: "feature",
        description:
          "Added a Library page that displays Characters and Personas as cards with avatar-based backgrounds for a more visual browsing experience",
      },
      {
        type: "improvement",
        description:
          "Removed Character and Persona management tabs from Settings and fully migrated them into the Library page",
      },
      {
        type: "feature",
        description:
          "Developed a lightweight in-house embedding model (lettuce-emb) to power dynamic memory and semantic recall",
      },
      {
        type: "feature",
        description:
          "Introduced Manual Memory, letting users pin important information without limiting context length (with the caveat that large memories can increase token usage on paid models)",
      },
      {
        type: "feature",
        description:
          "Introduced Dynamic Memory with a sliding window of the last N messages that are periodically summarised into long-term memories in the background",
      },
      {
        type: "feature",
        description:
          "Dynamic Memory now uses embeddings to retrieve the most relevant memories for each message, and users can choose which model powers both the summariser and memory manager",
      },
      {
        type: "feature",
        description:
          "Added Character and Persona import/export via .json files for easier backup and sharing",
      },
      {
        type: "breaking",
        description:
          "Migrated all app storage from .bin/.json files to a SQLite .db backend for more reliable, scalable data handling",
      },
      {
        type: "improvement",
        description:
          "Improved UI consistency across multiple screens, tightening spacing, typography and component alignment",
      },
      {
        type: "improvement",
        description:
          "Added required-variable validation to the prompt editor to prevent saving prompts with missing placeholders",
      },
      {
        type: "improvement",
        description:
          "Extended usage tracking with typed events for chat, regenerate, continue, summarisation tool calls and memory management tool calls",
      },
      {
        type: "feature",
        description:
          "Made pinned messages fully functional and integrated with the new memory features",
      },
      {
        type: "improvement",
        description:
          "Optimised ChatHistory and Edit Character/Persona pages for smoother scrolling and lower resource usage",
      },
      {
        type: "improvement",
        description: "Redesigned bottom popup animations to be smoother and stutter-free",
      },
    ],
  },
  {
    version: "1.0-beta_3.2",
    date: "2025-11-08",
    changes: [
      {
        type: "feature",
        description:
          "Added full support for persona avatars with proper persistence across sessions",
      },
      {
        type: "feature",
        description:
          "Introduced dynamic gradient backgrounds for character cards derived from avatar colors",
      },
      {
        type: "improvement",
        description:
          "Added a toggle in Settings to enable or disable dynamic gradient character cards",
      },
      {
        type: "feature",
        description:
          "Added advanced model parameter controls including frequency penalty, presence penalty and top-K sampling",
      },
      {
        type: "improvement",
        description:
          "Introduced API Parameter Support List modal to show which parameters are supported by the current model",
      },
      {
        type: "bugfix",
        description: "Fixed avatars not being saved correctly",
      },
      {
        type: "improvement",
        description: "Improved spacing and layout in chat header, history and settings",
      },
      {
        type: "improvement",
        description: "Improved persona loading behaviour across app restarts",
      },
      {
        type: "improvement",
        description: "Updated Custom Response Styles menu to the new UI design",
      },
      {
        type: "feature",
        description: "Added ability to cancel message regenerations from the UI",
      },
    ],
  },
  {
    version: "1.0-beta_3.1",
    date: "2025-11-05",
    changes: [
      {
        type: "breaking",
        description:
          "Reworked system prompt architecture from multiple scopes (app, model, character) into a single simplified prompt flow",
      },
      {
        type: "feature",
        description:
          "Introduced a new default system prompt for more stable tone, deeper conversations and stronger pseudo-memory behaviour",
      },
      {
        type: "improvement",
        description: "Improved predictability and ease-of-use of the System Prompts manager",
      },
      {
        type: "feature",
        description:
          "Added Character & Persona Search page to quickly filter characters and personas",
      },
      {
        type: "feature",
        description:
          "Added early Message Pinning feature for chats as a visual precursor to Manual Memory",
      },
      {
        type: "improvement",
        description: "General UI clarity improvements across multiple views",
      },
      {
        type: "improvement",
        description: "Cleaned up legacy prompt and scene-handling code",
      },
      {
        type: "improvement",
        description: "Improved request/cancel flow during message sends",
      },
      {
        type: "improvement",
        description: "Logging updates and internal stability improvements",
      },
    ],
  },
  {
    version: "1.0-beta_3",
    date: "2025-10-31",
    changes: [
      {
        type: "feature",
        description: "Added direct Mistral AI API integration",
      },
      {
        type: "feature",
        description: "Added direct Groq API integration",
      },
      {
        type: "feature",
        description:
          "Added multi-scope custom system prompts (app-wide, model-specific, character-specific)",
      },
      {
        type: "feature",
        description:
          "Added swipe-to-go-back and swipe-to-quit gestures for smoother navigation on Android",
      },
      {
        type: "improvement",
        description:
          "Implemented a new Android process plugin and back-handler to reduce crashes when switching activities",
      },
      {
        type: "improvement",
        description:
          "Performed ~75% backend refactor focused on performance, resource optimisation and clearer provider-adapter architecture",
      },
      {
        type: "improvement",
        description: "Centralised API endpoint management and improved streaming/error handling",
      },
      {
        type: "improvement",
        description:
          "Redesigned UsagePage, ChatSettings and BottomMenu for better UI/UX consistency",
      },
      {
        type: "improvement",
        description: "Unified button, icon and avatar design language across the app",
      },
      {
        type: "improvement",
        description: "Added animated usage counters and improved usage filters",
      },
      {
        type: "improvement",
        description: "Improved animations for smoother, more optimised transitions",
      },
      {
        type: "bugfix",
        description: "Fixed overly bright button visuals in light mode",
      },
      {
        type: "bugfix",
        description: "Fixed oversized filter checkbox on the Usage page",
      },
      {
        type: "bugfix",
        description: "Improved dark-mode input and button consistency across screens",
      },
      {
        type: "bugfix",
        description: "Fixed inconsistencies in chat continuation and roleplay instruction handling",
      },
      {
        type: "bugfix",
        description: "Corrected minor style and layout misalignments throughout the app",
      },
      {
        type: "feature",
        description:
          "Added support for {{char}} and {{persona}} placeholders in character descriptions",
      },
      {
        type: "breaking",
        description:
          "Switched project license to GNU AGPL v3 and updated README badges and issue templates",
      },
    ],
  },
  {
    version: "1.0-beta_2.1",
    date: "2025-10-19",
    changes: [
      {
        type: "bugfix",
        description: "Fixed an issue preventing users from accessing the Persona Edit page",
      },
      {
        type: "bugfix",
        description: "Resolved inconsistent or broken styling for switch buttons",
      },
      {
        type: "bugfix",
        description: "Fixed message bottom menu being triggered accidentally while scrolling",
      },
      {
        type: "improvement",
        description: "Improved slider styling and label consistency in model settings",
      },
      {
        type: "feature",
        description:
          "Added 'Max Tokens' input field with validation and suggestions in Edit Model page",
      },
      {
        type: "improvement",
        description:
          "Enhanced EditModelPage and PersonasPage with better default toggles and UI elements",
      },
    ],
  },
  {
    version: "1.0-beta_2",
    date: "2025-10-19",
    changes: [
      {
        type: "feature",
        description:
          "Added support for chat background images with automatic text colour adjustment for readability",
      },
      {
        type: "feature",
        description: "Introduced token usage analytics per chat, model and provider",
      },
      {
        type: "feature",
        description:
          "Added estimated cost tracking for OpenRouter endpoints (processed locally on-device)",
      },
      {
        type: "bugfix",
        description: "Fixed several rare crashes and general stability issues",
      },
      {
        type: "bugfix",
        description: "Resolved an issue where chat history failed to properly initialise sessions",
      },
      {
        type: "bugfix",
        description: "Fixed message stream package mix-ups affecting certain models",
      },
      {
        type: "improvement",
        description: "Improved overall performance and responsiveness of the app",
      },
      {
        type: "improvement",
        description: "Improved Continue feature so it correctly resumes from the last point",
      },
    ],
  },
  {
    version: "1.0-beta-1",
    date: "2025-10-13",
    changes: [
      {
        type: "feature",
        description: "First public beta release of LettuceAI focused on privacy-first AI role-play",
      },
      {
        type: "feature",
        description: "Local-only storage for chats and configuration, with user-owned API keys",
      },
      {
        type: "feature",
        description: "Cross-platform architecture with Android as the initial supported target",
      },
      {
        type: "improvement",
        description:
          "Initial onboarding and provider/model setup flow (with some known beta limitations)",
      },
      {
        type: "improvement",
        description:
          "Base chat experience including roleplay sessions powered by external AI providers",
      },
      {
        type: "improvement",
        description:
          "Documented known issues such as animation stutters, occasional onboarding failures and non-persistent model switching",
      },
    ],
  },
];

const typeConfig = {
  feature: {
    label: "New",
    icon: Sparkles,
    color: "text-accent/80",
    iconColor: "text-accent",
    bg: "bg-accent/10",
    glow: "shadow-[0_0_20px_-5px_rgba(52,211,153,0.3)]",
    gradient: "from-accent/20 to-accent/5",
  },
  improvement: {
    label: "Improved",
    icon: Zap,
    color: "text-info/80",
    iconColor: "text-info",
    bg: "bg-info/10",
    glow: "shadow-[0_0_20px_-5px_rgba(59,130,246,0.3)]",
    gradient: "from-info/20 to-info/5",
  },
  bugfix: {
    label: "Fixed",
    icon: Bug,
    color: "text-warning/80",
    iconColor: "text-warning",
    bg: "bg-warning/10",
    glow: "shadow-[0_0_20px_-5px_rgba(245,158,11,0.3)]",
    gradient: "from-warning/20 to-warning/5",
  },
  breaking: {
    label: "Breaking",
    icon: AlertTriangle,
    color: "text-danger/80",
    iconColor: "text-danger",
    bg: "bg-danger/10",
    glow: "shadow-[0_0_20px_-5px_rgba(239,68,68,0.3)]",
    gradient: "from-danger/20 to-danger/5",
  },
};

interface ChangeGroupProps {
  type: "feature" | "improvement" | "bugfix" | "breaking";
  changes: { type: string; description: string }[];
  defaultExpanded?: boolean;
}

function ChangeGroup({ type, changes, defaultExpanded = true }: ChangeGroupProps) {
  const [expanded, setExpanded] = useState(defaultExpanded);
  const config = typeConfig[type];
  const Icon = config.icon;

  if (changes.length === 0) return null;

  return (
    <motion.div
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      className="overflow-hidden"
    >
      {/* Group Header */}
      <button
        onClick={() => setExpanded(!expanded)}
        className={cn(
          "w-full flex items-center gap-3 px-4 py-3 rounded-xl",
          "bg-fg/3 border border-fg/6",
          "hover:bg-fg/5 transition-all duration-200",
          "group",
        )}
      >
        <div className={cn("flex items-center justify-center w-8 h-8 rounded-lg", config.bg)}>
          <Icon className={cn("w-4 h-4", config.iconColor)} />
        </div>
        <div className="flex-1 flex items-center gap-2">
          <span className={cn(typography.h3.size, typography.h3.weight, "text-fg")}>
            {config.label}
          </span>
          <span
            className={cn(
              "px-2 py-0.5 rounded-full text-[10px] font-semibold",
              config.bg,
              config.color,
            )}
          >
            {changes.length}
          </span>
        </div>
        <motion.div animate={{ rotate: expanded ? 90 : 0 }} transition={{ duration: 0.2 }}>
          <ChevronRight className="w-4 h-4 text-fg/40 group-hover:text-fg/60 transition-colors" />
        </motion.div>
      </button>

      {/* Changes List */}
      <AnimatePresence>
        {expanded && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.2, ease: "easeInOut" }}
            className="overflow-hidden"
          >
            <div className="pt-2 pl-4 space-y-2">
              {changes.map((change, idx) => (
                <motion.div
                  key={idx}
                  initial={{ opacity: 0, x: -8 }}
                  animate={{ opacity: 1, x: 0 }}
                  transition={{ delay: idx * 0.03 }}
                  className={cn(
                    "relative flex gap-3 pl-4 pr-4 py-3",
                    "rounded-xl",
                    "bg-linear-to-r",
                    config.gradient,
                    "group/item",
                  )}
                >
                  {/* Accent line */}
                  <div
                    className={cn(
                      "absolute left-0 top-3 bottom-3 w-0.5 rounded-full",
                      config.bg.replace("/10", "/40"),
                    )}
                  />
                  <p className={cn(typography.body.size, "text-fg/80 leading-relaxed")}>
                    {change.description}
                  </p>
                </motion.div>
              ))}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  );
}

export function ChangelogPage() {
  const [selectedVersion, setSelectedVersion] = useState<string>(changelog[0].version);
  const [showVersionMenu, setShowVersionMenu] = useState(false);

  const selectedEntry =
    changelog.find((entry) => entry.version === selectedVersion) || changelog[0];
  const currentIndex = changelog.findIndex((e) => e.version === selectedVersion);
  const isLatest = currentIndex === 0;

  const groupedChanges = useMemo(() => {
    const groups = {
      breaking: selectedEntry.changes.filter((c) => c.type === "breaking"),
      feature: selectedEntry.changes.filter((c) => c.type === "feature"),
      improvement: selectedEntry.changes.filter((c) => c.type === "improvement"),
      bugfix: selectedEntry.changes.filter((c) => c.type === "bugfix"),
    };
    return groups;
  }, [selectedEntry]);

  useEffect(() => {
    const handleOpenVersionSelector = () => {
      setShowVersionMenu(true);
    };

    window.addEventListener("changelog:openVersionSelector", handleOpenVersionSelector);
    return () => {
      window.removeEventListener("changelog:openVersionSelector", handleOpenVersionSelector);
    };
  }, []);

  const formatDate = (dateStr: string) => {
    return new Date(dateStr).toLocaleDateString("en-US", {
      year: "numeric",
      month: "long",
      day: "numeric",
    });
  };

  return (
    <div className="flex h-full flex-col text-fg/90">
      <main className="flex-1 overflow-y-auto pb-6">
        {/* Hero Header */}
        <div className="relative px-4 pt-4 pb-6">
          {/* Background gradient */}
          <div className="absolute inset-0 pointer-events-none" />

          <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.3, ease: "easeOut" }}
            className="relative"
          >
            {/* Version Badge */}

            {/* Version Number */}
            <div className="flex items-baseline gap-1 mb-3">
              <span className="text-fg/30 text-2xl font-bold">v</span>
              <h1 className="text-4xl font-black text-fg tracking-tight">
                {selectedEntry.version}
                {isLatest && (
                  <motion.span
                    initial={{ scale: 0.8, opacity: 0 }}
                    animate={{ scale: 1, opacity: 1 }}
                    className={cn(
                      "inline-flex items-center ml-2 px-2.5 py-1 rounded-full text-[10px] font-bold uppercase tracking-wider",
                      "bg-accent/20 text-accent/80 border border-accent/30",
                    )}
                  >
                    Latest
                  </motion.span>
                )}
              </h1>
            </div>

            {/* Meta info */}
            <div className="flex flex-wrap items-center gap-4 text-fg/40">
              <div className="flex items-center gap-1.5">
                <Calendar className="w-3.5 h-3.5" />
                <span className="text-xs font-medium">{formatDate(selectedEntry.date)}</span>
              </div>
              <div className="flex items-center gap-1.5">
                <Hash className="w-3.5 h-3.5" />
                <span className="text-xs font-medium">{selectedEntry.changes.length} changes</span>
              </div>
            </div>

            {/* Quick Stats */}
            <div className="flex gap-2 mt-5">
              {groupedChanges.feature.length > 0 && (
                <div
                  className={cn(
                    "flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg",
                    "bg-accent/10 border border-accent/20",
                  )}
                >
                  <Sparkles className="w-3.5 h-3.5 text-accent" />
                  <span className="text-xs font-semibold text-accent/80">
                    {groupedChanges.feature.length} new
                  </span>
                </div>
              )}
              {groupedChanges.improvement.length > 0 && (
                <div
                  className={cn(
                    "flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg",
                    "bg-info/10 border border-info/20",
                  )}
                >
                  <Zap className="w-3.5 h-3.5 text-info" />
                  <span className="text-xs font-semibold text-info/80">
                    {groupedChanges.improvement.length} improved
                  </span>
                </div>
              )}
              {groupedChanges.bugfix.length > 0 && (
                <div
                  className={cn(
                    "flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg",
                    "bg-warning/10 border border-warning/20",
                  )}
                >
                  <Bug className="w-3.5 h-3.5 text-warning" />
                  <span className="text-xs font-semibold text-warning/80">
                    {groupedChanges.bugfix.length} fixed
                  </span>
                </div>
              )}
              {groupedChanges.breaking.length > 0 && (
                <div
                  className={cn(
                    "flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg",
                    "bg-danger/10 border border-danger/20",
                  )}
                >
                  <AlertTriangle className="w-3.5 h-3.5 text-danger" />
                  <span className="text-xs font-semibold text-danger/80">
                    {groupedChanges.breaking.length} breaking
                  </span>
                </div>
              )}
            </div>
          </motion.div>
        </div>

        {/* Changes Content */}
        <div className="px-4 space-y-3">
          <ChangeGroup type="breaking" changes={groupedChanges.breaking} />
          <ChangeGroup type="feature" changes={groupedChanges.feature} />
          <ChangeGroup type="improvement" changes={groupedChanges.improvement} />
          <ChangeGroup type="bugfix" changes={groupedChanges.bugfix} />
        </div>

        {/* Footer */}
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ delay: 0.3 }}
          className="px-4 pt-8 pb-4"
        >
          <a
            href="https://github.com/LettuceAI/mobile-app"
            target="_blank"
            rel="noopener noreferrer"
            className={cn(
              "flex items-center justify-center gap-2 w-full py-3 rounded-xl",
              "bg-fg/3 border border-fg/6",
              "hover:bg-fg/6 hover:border-fg/10",
              "transition-all duration-200 group",
            )}
          >
            <span className="text-sm text-fg/50 group-hover:text-fg/70 transition-colors">
              Follow development on GitHub
            </span>
            <ArrowRight className="w-4 h-4 text-fg/30 group-hover:text-fg/50 group-hover:translate-x-0.5 transition-all" />
          </a>
        </motion.div>
      </main>

      {/* Version Selector Bottom Menu */}
      <BottomMenu
        isOpen={showVersionMenu}
        onClose={() => setShowVersionMenu(false)}
        title="Version History"
      >
        <div className="space-y-2.5 max-h-[60vh] overflow-y-auto">
          {changelog.map((entry, idx) => {
            const isSelected = selectedVersion === entry.version;
            const featureCount = entry.changes.filter((c) => c.type === "feature").length;
            const improvementCount = entry.changes.filter((c) => c.type === "improvement").length;
            const bugfixCount = entry.changes.filter((c) => c.type === "bugfix").length;
            const breakingCount = entry.changes.filter((c) => c.type === "breaking").length;

            return (
              <button
                key={entry.version}
                onClick={() => {
                  setSelectedVersion(entry.version);
                  setShowVersionMenu(false);
                }}
                className={cn(
                  "group relative flex w-full items-start gap-3 rounded-xl px-4 py-3.5 text-left transition-all duration-200",
                  isSelected
                    ? "bg-accent/10 border border-accent/30"
                    : "bg-fg/2 border border-fg/4 hover:bg-fg/5 hover:border-fg/8",
                )}
              >
                {/* Timeline dot */}
                <div className="flex flex-col items-center pt-1.5">
                  <div
                    className={cn(
                      "w-2.5 h-2.5 rounded-full",
                      isSelected ? "bg-accent" : "bg-fg/20",
                    )}
                  />
                  {idx < changelog.length - 1 && (
                    <div
                      className={cn(
                        "w-px flex-1 mt-2 min-h-5",
                        "bg-linear-to-b",
                        isSelected ? "from-accent/40 to-transparent" : "from-fg/10 to-transparent",
                      )}
                    />
                  )}
                </div>

                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 mb-1">
                    <span
                      className={cn(
                        "text-base font-bold",
                        isSelected ? "text-accent/80" : "text-fg",
                      )}
                    >
                      v{entry.version}
                    </span>
                    {idx === 0 && (
                      <span
                        className={cn(
                          "px-1.5 rounded text-[9px] font-bold uppercase",
                          "bg-accent/20 text-accent/80 border border-accent/30",
                        )}
                      >
                        Latest
                      </span>
                    )}
                  </div>

                  <div
                    className={cn(
                      "text-[11px] mb-2",
                      isSelected ? "text-accent/80/60" : "text-fg/40",
                    )}
                  >
                    {formatDate(entry.date)}
                  </div>

                  {/* Mini stats */}
                  <div className="flex flex-wrap gap-1.5">
                    {featureCount > 0 && (
                      <span className="flex items-center gap-1 text-[10px] text-accent/80/70">
                        <Sparkles className="w-3 h-3" />
                        {featureCount}
                      </span>
                    )}
                    {improvementCount > 0 && (
                      <span className="flex items-center gap-1 text-[10px] text-info/70">
                        <Zap className="w-3 h-3" />
                        {improvementCount}
                      </span>
                    )}
                    {bugfixCount > 0 && (
                      <span className="flex items-center gap-1 text-[10px] text-warning/80/70">
                        <Bug className="w-3 h-3" />
                        {bugfixCount}
                      </span>
                    )}
                    {breakingCount > 0 && (
                      <span className="flex items-center gap-1 text-[10px] text-danger/80/70">
                        <AlertTriangle className="w-3 h-3" />
                        {breakingCount}
                      </span>
                    )}
                  </div>
                </div>

                {isSelected && (
                  <div className="pt-1">
                    <div className="w-5 h-5 rounded-full bg-accent/20 flex items-center justify-center">
                      <div className="w-2 h-2 rounded-full bg-accent" />
                    </div>
                  </div>
                )}
              </button>
            );
          })}
        </div>
      </BottomMenu>
    </div>
  );
}
