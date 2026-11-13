/** Every feature and fix in 3.0, from the rewrite's commit history, grouped by area. */
export interface ChangeArea {
  area: string;
  items: readonly { kind: "new" | "fixed"; text: string }[];
}

export const CHANGELOG: readonly ChangeArea[] = [
 {
  "area": "Chats",
  "items": [
   {
    "kind": "new",
    "text": "Chat models can return images, and they appear in the message"
   },
   {
    "kind": "new",
    "text": "Replies use your current characters, personas, prompts and models on every turn"
   },
   {
    "kind": "new",
    "text": "Generate a scene image for any message and add it to the chat"
   },
   {
    "kind": "new",
    "text": "Group chats can show scene images of who is in the room"
   },
   {
    "kind": "new",
    "text": "Help me reply drafts your next message, and group chats draft with the whole cast"
   },
   {
    "kind": "new",
    "text": "Branch tree page to see and move between conversation branches"
   },
   {
    "kind": "new",
    "text": "Fork, rename and switch conversation branches"
   },
   {
    "kind": "new",
    "text": "Branch a conversation into a group or another character"
   },
   {
    "kind": "new",
    "text": "Switch the starting scene by swiping its message"
   },
   {
    "kind": "new",
    "text": "Duplicate direct and group chats together with their memory"
   },
   {
    "kind": "new",
    "text": "Duplicated companion chats get their own private state"
   },
   {
    "kind": "new",
    "text": "Regenerate with a chosen direction, model or speaker"
   },
   {
    "kind": "new",
    "text": "Send a message as system from the composer"
   },
   {
    "kind": "new",
    "text": "Redesigned message menu"
   },
   {
    "kind": "new",
    "text": "Replies show the lorebook entries and memories they used"
   },
   {
    "kind": "new",
    "text": "Time awareness is available in every direct and group chat"
   },
   {
    "kind": "new",
    "text": "Set a companion's time override in chat settings"
   },
   {
    "kind": "new",
    "text": "Messages render Markdown and restricted HTML"
   },
   {
    "kind": "new",
    "text": "Choose an AM/PM timestamp format"
   },
   {
    "kind": "new",
    "text": "Edit a character's or group's appearance from the chat, beside a live preview"
   },
   {
    "kind": "new",
    "text": "Widget panels sit beside the chat column again"
   },
   {
    "kind": "new",
    "text": "Open a chat's settings or its character from the chat menu"
   },
   {
    "kind": "new",
    "text": "Chat header shows only the character name and opens chat settings"
   },
   {
    "kind": "new",
    "text": "Chat screen with a plus menu, dictation and a chat settings panel"
   },
   {
    "kind": "new",
    "text": "Character chat history page with filters"
   },
   {
    "kind": "new",
    "text": "Edit memories and see their update history from the chat"
   },
   {
    "kind": "new",
    "text": "Each chat keeps its own model settings and background"
   },
   {
    "kind": "new",
    "text": "Reset chat settings to their launch defaults"
   },
   {
    "kind": "new",
    "text": "Group chats can change how speakers are selected"
   },
   {
    "kind": "new",
    "text": "Add members to a group chat after it starts"
   },
   {
    "kind": "new",
    "text": "A director can add a user message without triggering a reply"
   },
   {
    "kind": "new",
    "text": "Every character can keep scheduled notes"
   },
   {
    "kind": "new",
    "text": "Search chat messages and see a pinned messages list"
   },
   {
    "kind": "new",
    "text": "Prompt details show a breakdown of context usage"
   },
   {
    "kind": "new",
    "text": "Message stats show the provider's token usage"
   },
   {
    "kind": "new",
    "text": "Progress shows in toasts and compact inline notes"
   },
   {
    "kind": "new",
    "text": "Pure mode filters replies by level and gives chat prompts that level's content rules"
   },
   {
    "kind": "new",
    "text": "Pure mode works in more languages by detecting the language of the text"
   },
   {
    "kind": "new",
    "text": "Reasoning effort and budget are available for local and Ollama models, with xhigh and max levels"
   },
   {
    "kind": "new",
    "text": "A reasoning prefill dropdown replaces the local Gemma thinking toggle"
   },
   {
    "kind": "new",
    "text": "Replies use the output token limit from model settings"
   },
   {
    "kind": "new",
    "text": "The chat column is full width by default"
   },
   {
    "kind": "fixed",
    "text": "Chats keep their scroll position while the composer grows"
   },
   {
    "kind": "fixed",
    "text": "Chats scroll to the latest message after you send"
   },
   {
    "kind": "fixed",
    "text": "Stopping a reply keeps only what you saw"
   },
   {
    "kind": "fixed",
    "text": "Chat backgrounds, memory rewinds and message deletion work again"
   },
   {
    "kind": "fixed",
    "text": "Chats work with models and accounts that cannot stream"
   },
   {
    "kind": "fixed",
    "text": "Chats no longer cap history length, lorebooks or participants"
   },
   {
    "kind": "fixed",
    "text": "Group chats can start and run with one remaining member"
   },
   {
    "kind": "fixed",
    "text": "Long reasoning text is no longer cut at 256 KiB"
   },
   {
    "kind": "fixed",
    "text": "No content rules are sent when Pure mode is off"
   },
   {
    "kind": "fixed",
    "text": "Prompt and reply token arrows are no longer swapped"
   },
   {
    "kind": "fixed",
    "text": "Context windows below 512 tokens are no longer raised"
   },
   {
    "kind": "fixed",
    "text": "Deleting messages on one branch never changes other branches"
   }
  ]
 },
 {
  "area": "Groups",
  "items": [
   {
    "kind": "new",
    "text": "Group library with editing and appearance controls"
   },
   {
    "kind": "new",
    "text": "Director modes, mentions and reply selection in group chats"
   },
   {
    "kind": "new",
    "text": "Chat members can use their own models, with inheritance resets and history"
   },
   {
    "kind": "new",
    "text": "Groups have a time default and scheduled notes"
   },
   {
    "kind": "new",
    "text": "Companion characters can join group chats as ordinary members"
   },
   {
    "kind": "new",
    "text": "Group companion mode switch for groups and group chats"
   },
   {
    "kind": "new",
    "text": "Group companion state and effects are shown, with warnings for members without a Soul"
   },
   {
    "kind": "new",
    "text": "Each group companion member keeps its own perspective memories"
   },
   {
    "kind": "new",
    "text": "Group companion members grow their Soul from their perspective memories"
   },
   {
    "kind": "new",
    "text": "Social event names are translated in the group effect sheet"
   },
   {
    "kind": "new",
    "text": "Undoing a group companion memory update also undoes its perspective memories"
   },
   {
    "kind": "new",
    "text": "Group companion chats track who heard each message"
   },
   {
    "kind": "new",
    "text": "Group companion page has a member picker, stances and Memories and Activity tabs"
   },
   {
    "kind": "new",
    "text": "Group companion state is synced, backed up and copied with the group"
   },
   {
    "kind": "fixed",
    "text": "Group speaker selection with an LLM reports its reason"
   },
   {
    "kind": "fixed",
    "text": "Group replies use the speaking character's group prompt"
   },
   {
    "kind": "fixed",
    "text": "Regenerating with a chosen speaker keeps that speaker"
   }
  ]
 },
 {
  "area": "Companions",
  "items": [
   {
    "kind": "new",
    "text": "Companions detect emotion with Lettuce Thymos"
   },
   {
    "kind": "new",
    "text": "Companions can share one memory pool across chats, with a share memory toggle"
   },
   {
    "kind": "new",
    "text": "Companion chats grow their own Soul when soul growth sharing is off"
   },
   {
    "kind": "new",
    "text": "Clear, remove and lock Soul growth entries"
   },
   {
    "kind": "new",
    "text": "The relationship stance is described in seven bands with raw scores"
   },
   {
    "kind": "new",
    "text": "Soul growth and consolidation run after each memory update and resume after a restart"
   },
   {
    "kind": "new",
    "text": "Companion chats keep their own session clock for time-aware replies"
   },
   {
    "kind": "new",
    "text": "New companion chats start time aware when the companion is"
   },
   {
    "kind": "new",
    "text": "Story dates before 1970 are supported"
   },
   {
    "kind": "new",
    "text": "Companions get a default Soul and follow the current persona"
   },
   {
    "kind": "new",
    "text": "Timestamps a model echoes back are removed from time-aware replies"
   },
   {
    "kind": "new",
    "text": "Memory effects that fail three times show as failed and can be retried"
   },
   {
    "kind": "fixed",
    "text": "Continuing a companion chat creates missing companion state, as sending does"
   },
   {
    "kind": "fixed",
    "text": "Replies recorded for a deleted character are still recovered"
   },
   {
    "kind": "fixed",
    "text": "A failed memory cycle retries its messages on the next cycle"
   },
   {
    "kind": "fixed",
    "text": "Scheduled notes and companion state follow the chat's companion clock"
   },
   {
    "kind": "fixed",
    "text": "Rewinds keep a shared memory pool intact"
   },
   {
    "kind": "fixed",
    "text": "One bad growth or consolidation item no longer discards the rest"
   },
   {
    "kind": "fixed",
    "text": "Growth and consolidation use the model's companion memory settings"
   }
  ]
 },
 {
  "area": "Memory",
  "items": [
   {
    "kind": "new",
    "text": "Memory updates run after replies, resume after a restart and retry failed windows"
   },
   {
    "kind": "new",
    "text": "Trigger, skip or approve a memory update by hand"
   },
   {
    "kind": "new",
    "text": "Memory update log with revert and error dismissal"
   },
   {
    "kind": "new",
    "text": "Memories can be edited per conversation"
   },
   {
    "kind": "new",
    "text": "Conversation branches keep their own memory"
   },
   {
    "kind": "new",
    "text": "Deleting after a message rewinds the memory changes made since"
   },
   {
    "kind": "new",
    "text": "A failed memory rewind after a delete is shown and can be retried"
   },
   {
    "kind": "new",
    "text": "Dynamic memory honors your prompt overrides"
   },
   {
    "kind": "new",
    "text": "Lettuce Eidos v5 embeds memories with calibrated similarity and re-embeds when needed"
   },
   {
    "kind": "new",
    "text": "See why a proposed memory was skipped or already known"
   },
   {
    "kind": "new",
    "text": "The meaning-based duplicate check can be turned off"
   },
   {
    "kind": "new",
    "text": "Memories without an embedding are embedded before retrieval"
   },
   {
    "kind": "new",
    "text": "Companion memory retrieval narrows to the calendar range being asked about"
   },
   {
    "kind": "new",
    "text": "Pinned memories are never deleted, and hot ones cool down instead"
   },
   {
    "kind": "new",
    "text": "Dynamic memory starts on the Balanced profile"
   },
   {
    "kind": "fixed",
    "text": "Edited facts are no longer treated as duplicates of the original"
   },
   {
    "kind": "fixed",
    "text": "Valid memories are no longer rejected as duplicates or meta text"
   },
   {
    "kind": "fixed",
    "text": "Reverting a memory update no longer overwrites your edits"
   },
   {
    "kind": "fixed",
    "text": "Memory spaces are no longer capped at 4,096 items"
   },
   {
    "kind": "fixed",
    "text": "Memory message counters refresh when the conversation changes"
   },
   {
    "kind": "fixed",
    "text": "Pending message windows are processed after automatic memory updates"
   },
   {
    "kind": "fixed",
    "text": "Forcing a memory update with nothing new re-runs the latest window"
   },
   {
    "kind": "fixed",
    "text": "A manual memory update during a running one is reported instead of dropped"
   },
   {
    "kind": "fixed",
    "text": "Unusable memory tool calls are skipped instead of failing the update"
   },
   {
    "kind": "fixed",
    "text": "Pending approvals stay with their own branch"
   },
   {
    "kind": "fixed",
    "text": "Chat turns, memory cycles and rewinds no longer undo each other"
   },
   {
    "kind": "fixed",
    "text": "Memories are embedded at the configured dimension"
   }
  ]
 },
 {
  "area": "Characters and personas",
  "items": [
   {
    "kind": "new",
    "text": "Smart creator proposes characters, personas and lorebooks and applies them when you confirm"
   },
   {
    "kind": "new",
    "text": "Smart creator can edit existing characters, personas and lorebooks"
   },
   {
    "kind": "new",
    "text": "Smart creator works from references, uploads and image tools"
   },
   {
    "kind": "new",
    "text": "Smart creator drafts design notes from a subject's images"
   },
   {
    "kind": "new",
    "text": "Regenerate the latest Smart creator reply from the draft it started with"
   },
   {
    "kind": "new",
    "text": "Character creation wizard"
   },
   {
    "kind": "new",
    "text": "Persona creation wizard"
   },
   {
    "kind": "new",
    "text": "Character edit page"
   },
   {
    "kind": "new",
    "text": "Persona editor page"
   },
   {
    "kind": "new",
    "text": "Characters keep their authored scenario and rules"
   },
   {
    "kind": "new",
    "text": "The emotion model is offered when companion mode is chosen"
   },
   {
    "kind": "new",
    "text": "Search characters and personas in ordered lists"
   },
   {
    "kind": "new",
    "text": "Large libraries browse smoothly with fast image previews"
   },
   {
    "kind": "new",
    "text": "Duplicate a character"
   },
   {
    "kind": "new",
    "text": "Archive and restore characters"
   },
   {
    "kind": "new",
    "text": "Crop avatars and keep banner framing"
   },
   {
    "kind": "new",
    "text": "Generate and edit avatars from prompts"
   },
   {
    "kind": "new",
    "text": "Avatars get gradients from their dominant colors"
   },
   {
    "kind": "new",
    "text": "Avatar downloads can be cancelled"
   },
   {
    "kind": "new",
    "text": "Cover cards tint from the name when there is no avatar"
   },
   {
    "kind": "new",
    "text": "Discover shows that it is on its way"
   },
   {
    "kind": "fixed",
    "text": "Characters, groups, scenes and prompts have no count limits"
   },
   {
    "kind": "fixed",
    "text": "A group whose members were deleted is kept instead of being deleted"
   },
   {
    "kind": "fixed",
    "text": "Personas can have an empty description"
   },
   {
    "kind": "fixed",
    "text": "Deleting a character's scene clears its default scene and starter links"
   },
   {
    "kind": "fixed",
    "text": "Groups with an unavailable persona fall back to the default persona"
   },
   {
    "kind": "fixed",
    "text": "Entry and media menus animate out when closed"
   },
   {
    "kind": "fixed",
    "text": "Smart creator sends the whole dialogue instead of trimming it to 512 messages"
   }
  ]
 },
 {
  "area": "Lorebooks",
  "items": [
   {
    "kind": "new",
    "text": "Import World Info files and export lorebooks as World Info or USC"
   },
   {
    "kind": "new",
    "text": "Create lorebooks with AI in stages: outline, draft, refine, review coherence, commit"
   },
   {
    "kind": "new",
    "text": "AI lorebook creation reads PDF and text sources"
   },
   {
    "kind": "new",
    "text": "AI can write lorebook entries and suggest keywords"
   },
   {
    "kind": "new",
    "text": "Failed lorebook planners can be retried"
   },
   {
    "kind": "new",
    "text": "Lorebook entry batches are written concurrently"
   },
   {
    "kind": "new",
    "text": "Lorebook drafts can be edited and committed during coherence review"
   },
   {
    "kind": "new",
    "text": "Trigger preview for chats and text"
   },
   {
    "kind": "new",
    "text": "Lorebook editor page"
   },
   {
    "kind": "new",
    "text": "Regex keywords have their own pattern editor"
   },
   {
    "kind": "new",
    "text": "Secondary keywords with SillyTavern's four conditions, editable in the entry sheet"
   },
   {
    "kind": "new",
    "text": "Plain keywords can match phrase patterns"
   },
   {
    "kind": "new",
    "text": "Set the lorebook keyword scan depth in app settings, shared by prompt keyword conditions"
   },
   {
    "kind": "new",
    "text": "System prompt and entry editors"
   },
   {
    "kind": "new",
    "text": "Review built-in prompt updates before they overwrite your edited prompts"
   },
   {
    "kind": "new",
    "text": "Prompt text tokens are counted with the o200k encoding"
   },
   {
    "kind": "fixed",
    "text": "Case-insensitive keywords match the Turkish I, İ and ı as i"
   },
   {
    "kind": "fixed",
    "text": "Case-insensitive regex keywords keep working"
   },
   {
    "kind": "fixed",
    "text": "Persona lorebook bindings apply in direct and group chats"
   },
   {
    "kind": "fixed",
    "text": "Scene messages count toward lorebook keyword matching in one-to-one chats"
   },
   {
    "kind": "fixed",
    "text": "Entry drag handles stay visible on touch screens and the content field is larger"
   },
   {
    "kind": "fixed",
    "text": "Long entry titles stay inside the trigger preview"
   },
   {
    "kind": "fixed",
    "text": "Long match context and names over 256 characters are no longer rejected"
   },
   {
    "kind": "fixed",
    "text": "Long World Info keys and titles import, and lorebooks have no count limit"
   },
   {
    "kind": "fixed",
    "text": "Blank lorebook entry titles are accepted"
   },
   {
    "kind": "fixed",
    "text": "All non-blank lorebook drafts are committed"
   },
   {
    "kind": "fixed",
    "text": "Stray or malformed {{#if}} controls are kept as text instead of failing the turn"
   },
   {
    "kind": "fixed",
    "text": "The char and user template variables work in prompts"
   },
   {
    "kind": "fixed",
    "text": "Empty lore and summary sections are left out of prompts"
   },
   {
    "kind": "fixed",
    "text": "Chat history is sent in conversation order when the companion clock has moved"
   },
   {
    "kind": "fixed",
    "text": "Prompt names are kept when a built-in prompt is reset"
   },
   {
    "kind": "fixed",
    "text": "Prompt groups and entry cards stay inside the screen on phones"
   }
  ]
 },
 {
  "area": "Models and providers",
  "items": [
   {
    "kind": "new",
    "text": "Prompt caching for supported providers, including explicit caching on Gemini"
   },
   {
    "kind": "new",
    "text": "Streaming replies can be cancelled"
   },
   {
    "kind": "new",
    "text": "Native reasoning requests for supported models"
   },
   {
    "kind": "new",
    "text": "Tool calling works with OpenAI, Anthropic, Gemini and Ollama models"
   },
   {
    "kind": "new",
    "text": "Anthropic thinking and Gemini thought signatures carry across turns"
   },
   {
    "kind": "new",
    "text": "Pin OpenRouter providers"
   },
   {
    "kind": "new",
    "text": "Run llama.cpp models on the embedded runtime"
   },
   {
    "kind": "new",
    "text": "llama.cpp vision models receive image and audio attachments"
   },
   {
    "kind": "new",
    "text": "List, pull and delete models on an Ollama server"
   },
   {
    "kind": "new",
    "text": "Ollama accounts judge whether a model can run on the server's hardware"
   },
   {
    "kind": "new",
    "text": "llama.cpp tries GPU offload candidates and falls back to CPU"
   },
   {
    "kind": "new",
    "text": "Estimate context memory across RAM and GPU"
   },
   {
    "kind": "new",
    "text": "GGUF fit estimates come from llama.cpp's memory projection"
   },
   {
    "kind": "new",
    "text": "Browse Hugging Face models, authors, files and READMEs with your saved token"
   },
   {
    "kind": "new",
    "text": "Hugging Face files are scored for this machine with a recommended file, context and KV type"
   },
   {
    "kind": "new",
    "text": "Unsloth dynamic and importance-matrix quants are labelled"
   },
   {
    "kind": "new",
    "text": "Download Hugging Face GGUF models and register them as llama.cpp models"
   },
   {
    "kind": "new",
    "text": "Warn about where a model is used before deleting it"
   },
   {
    "kind": "new",
    "text": "Model editor with responsive columns and linked layer sliders"
   },
   {
    "kind": "new",
    "text": "Forgiving search in the provider model list"
   },
   {
    "kind": "new",
    "text": "Per-model settings for llama.cpp, image models and features like reply help and dynamic memory"
   },
   {
    "kind": "new",
    "text": "llama.cpp settings resolve per field from chat, model and app"
   },
   {
    "kind": "new",
    "text": "Separate quantization types for the K and V caches"
   },
   {
    "kind": "new",
    "text": "Adaptive-p sampler and Gemma4 forced reasoning"
   },
   {
    "kind": "new",
    "text": "Top-n-sigma sampling"
   },
   {
    "kind": "new",
    "text": "MTP speculative decoding with adaptive draft length"
   },
   {
    "kind": "new",
    "text": "DFlash and DSpark speculative decoding"
   },
   {
    "kind": "new",
    "text": "Integrated GPUs can be selected, for APU machines"
   },
   {
    "kind": "new",
    "text": "Local model load progress and throughput show during use"
   },
   {
    "kind": "new",
    "text": "Unload loaded llama.cpp models and see the device list"
   },
   {
    "kind": "new",
    "text": "Models are sorted alphabetically in model and provider lists"
   },
   {
    "kind": "new",
    "text": "Providers show logos and are grouped by vendor"
   },
   {
    "kind": "new",
    "text": "Moving the models folder can be cancelled and recovers if interrupted, and queued downloads resume"
   },
   {
    "kind": "fixed",
    "text": "Custom endpoints, keyless accounts and header overrides work"
   },
   {
    "kind": "fixed",
    "text": "Image and audio attachments are sent to vision models instead of being rejected"
   },
   {
    "kind": "fixed",
    "text": "OpenRouter skips unusable endpoints, and keyless image servers run"
   },
   {
    "kind": "fixed",
    "text": "Gemini Express image models run instead of being rejected"
   },
   {
    "kind": "fixed",
    "text": "Tool-call arguments in any shape local models emit are accepted"
   },
   {
    "kind": "fixed",
    "text": "Models may return any number of tool calls in one response"
   },
   {
    "kind": "fixed",
    "text": "A failed Ollama pull is reported when its stream ends without success"
   },
   {
    "kind": "fixed",
    "text": "Reasoning and prompt caching are blocked only when the model marks them unsupported"
   },
   {
    "kind": "fixed",
    "text": "Reasoning stays off unless enabled, and the reasoning budget is sent"
   },
   {
    "kind": "fixed",
    "text": "Models copied into the library never delete the original unless the copy matches"
   },
   {
    "kind": "fixed",
    "text": "Private app data is protected from models folder moves"
   },
   {
    "kind": "fixed",
    "text": "Interrupted model downloads finish or clean up, and models are not rehashed on each use"
   },
   {
    "kind": "fixed",
    "text": "llama.cpp runs on CPU on Apple"
   },
   {
    "kind": "fixed",
    "text": "KV cache placement is honored for single-GPU setups"
   },
   {
    "kind": "fixed",
    "text": "Streaming replies handle chunk boundaries and completion correctly"
   },
   {
    "kind": "fixed",
    "text": "Credentials no longer appear in error messages"
   },
   {
    "kind": "fixed",
    "text": "Requests honor system proxies and follow same-host redirects"
   },
   {
    "kind": "fixed",
    "text": "Large streamed responses are no longer capped"
   },
   {
    "kind": "fixed",
    "text": "Stalled downloads report a timeout, and connections time out after 30 seconds"
   },
   {
    "kind": "fixed",
    "text": "Refused Hugging Face installs name the repository"
   }
  ]
 },
 {
  "area": "Images",
  "items": [
   {
    "kind": "new",
    "text": "Automatic scene images toggle"
   },
   {
    "kind": "new",
    "text": "Scene image tools in chat"
   },
   {
    "kind": "new",
    "text": "Pick who appears in a scene image, in group chats too"
   },
   {
    "kind": "new",
    "text": "Scene images support approval and manual generation"
   },
   {
    "kind": "new",
    "text": "Run stable-diffusion.cpp locally with verified engine, model and upscaler installs"
   },
   {
    "kind": "new",
    "text": "Local image models can be assembled from Hugging Face bundles"
   },
   {
    "kind": "new",
    "text": "Check whether a catalog model or bundle can run on this machine"
   },
   {
    "kind": "new",
    "text": "Local LoRA library with keyword hydration"
   },
   {
    "kind": "new",
    "text": "Download CivitAI LoRAs, each on its own page"
   },
   {
    "kind": "new",
    "text": "ComfyUI, Automatic1111, NanoGPT and custom image providers are supported"
   },
   {
    "kind": "new",
    "text": "ComfyUI workflows are kept on import"
   },
   {
    "kind": "new",
    "text": "OpenRouter image generation retries transient errors and shrinks reference images before upload"
   },
   {
    "kind": "new",
    "text": "Image generations run as background jobs"
   },
   {
    "kind": "new",
    "text": "Local image generation shows progress and can be cancelled"
   },
   {
    "kind": "new",
    "text": "Playground history lists and deletes past generations"
   },
   {
    "kind": "new",
    "text": "Each image feature picks its model from the list in settings order"
   },
   {
    "kind": "new",
    "text": "Local image pages use the full width and the LoRA library is polished"
   },
   {
    "kind": "new",
    "text": "Local image models run on discrete GPUs and an AMD APU's integrated GPU"
   },
   {
    "kind": "fixed",
    "text": "The models folder cannot move while local image work is active"
   },
   {
    "kind": "fixed",
    "text": "Queued local image jobs wait during a models folder move instead of being cancelled"
   },
   {
    "kind": "fixed",
    "text": "An idle image server no longer blocks a models folder move"
   }
  ]
 },
 {
  "area": "Voice",
  "items": [
   {
    "kind": "new",
    "text": "Transcribe speech with verified Whisper models you can install and remove"
   },
   {
    "kind": "new",
    "text": "Dictation learns corrections from your edits"
   },
   {
    "kind": "new",
    "text": "Transcription learning libraries transfer between devices"
   },
   {
    "kind": "new",
    "text": "Text to speech with OpenAI-compatible providers"
   },
   {
    "kind": "new",
    "text": "Text to speech with ElevenLabs, with voice refresh and credential check"
   },
   {
    "kind": "new",
    "text": "Text to speech with Fish Audio, with voice refresh and credential check"
   },
   {
    "kind": "new",
    "text": "Text to speech with Fish Speech servers"
   },
   {
    "kind": "new",
    "text": "Text to speech with Gemini, with credential check"
   },
   {
    "kind": "new",
    "text": "Preview and create designed voices on ElevenLabs"
   },
   {
    "kind": "new",
    "text": "Search an ElevenLabs account's voice library by text"
   },
   {
    "kind": "new",
    "text": "Kokoro voices run natively, with verified model and voice installs"
   },
   {
    "kind": "new",
    "text": "Blend Kokoro voices"
   },
   {
    "kind": "new",
    "text": "Kokoro runs on Android"
   },
   {
    "kind": "new",
    "text": "Narrate messages and reuse finished audio"
   },
   {
    "kind": "new",
    "text": "Count and clear the speech audio cache"
   },
   {
    "kind": "new",
    "text": "Android asks for microphone permission before recording"
   },
   {
    "kind": "new",
    "text": "Deleting a voice provider names the characters that still use it"
   },
   {
    "kind": "fixed",
    "text": "Whisper and Kokoro downloads that failed, were cancelled or interrupted can be started again"
   },
   {
    "kind": "fixed",
    "text": "ElevenLabs voices with null labels are read"
   },
   {
    "kind": "fixed",
    "text": "Large voice libraries and long voice names are kept"
   },
   {
    "kind": "fixed",
    "text": "Transcription works after automatic language detection"
   },
   {
    "kind": "fixed",
    "text": "Gemini speech audio is saved as WAV"
   },
   {
    "kind": "fixed",
    "text": "Transcriptions have no duration cap"
   },
   {
    "kind": "fixed",
    "text": "Voice creation is sent only once so it is billed once"
   },
   {
    "kind": "fixed",
    "text": "Cached audio is skipped after a provider's settings change"
   },
   {
    "kind": "fixed",
    "text": "Editing a voice keeps its provider selection"
   },
   {
    "kind": "fixed",
    "text": "Dictation audio is kept after a late stop failure"
   },
   {
    "kind": "fixed",
    "text": "The dictation model choice is kept in backups"
   },
   {
    "kind": "fixed",
    "text": "GPU controls are hidden in CPU-only builds"
   }
  ]
 },
 {
  "area": "Sync, backup and import",
  "items": [
   {
    "kind": "new",
    "text": "Pair devices with a QR code and resolve sync conflicts in the sync settings page"
   },
   {
    "kind": "new",
    "text": "Sync personas, characters, lorebooks, groups and their media"
   },
   {
    "kind": "new",
    "text": "Sync prompts and application settings"
   },
   {
    "kind": "new",
    "text": "Sync provider accounts and model profiles, with local model files staying on each device"
   },
   {
    "kind": "new",
    "text": "API keys sync between devices, encrypted"
   },
   {
    "kind": "new",
    "text": "Sync conversations with their messages, branches and usage"
   },
   {
    "kind": "new",
    "text": "Sync memory, companion Souls, relationships and scheduled notes"
   },
   {
    "kind": "new",
    "text": "Sync voice providers, user voices and speech learning"
   },
   {
    "kind": "new",
    "text": "Sync playground history and Creation Helper sessions"
   },
   {
    "kind": "new",
    "text": "Messages written on two devices at once fork into a shared branch and notify you"
   },
   {
    "kind": "new",
    "text": "Deleting a conversation or character on one device deletes it on the others"
   },
   {
    "kind": "new",
    "text": "Sync asks before connecting to a paired device"
   },
   {
    "kind": "new",
    "text": "Every unresolved conflict and fork is listed"
   },
   {
    "kind": "new",
    "text": "Entities larger than a batch sync, and those that cannot sync are reported"
   },
   {
    "kind": "new",
    "text": "Encrypted backups with stronger key derivation, streamed in sections without a total size limit"
   },
   {
    "kind": "new",
    "text": "Backups include chats, memory, companion state, media, providers, voices and usage"
   },
   {
    "kind": "new",
    "text": "Backups carry Hugging Face and CivitAI tokens, trusted certificates and the embedding choice"
   },
   {
    "kind": "new",
    "text": "Backup and restore page"
   },
   {
    "kind": "new",
    "text": "Restore shows the current step, elapsed time and where it stopped"
   },
   {
    "kind": "new",
    "text": "Restore a backup into a fresh database and switch to it"
   },
   {
    "kind": "new",
    "text": "A restore requires the memory and emotion models it needs"
   },
   {
    "kind": "new",
    "text": "Restore a backup from other options"
   },
   {
    "kind": "new",
    "text": "Restore previous-app backups"
   },
   {
    "kind": "new",
    "text": "Inspect a backup before restoring"
   },
   {
    "kind": "new",
    "text": "Backups and restores run as background jobs"
   },
   {
    "kind": "new",
    "text": "The Welcome back flow migrates from the old app's backup or live database"
   },
   {
    "kind": "new",
    "text": "Import old direct chats as finished conversations with their variants"
   },
   {
    "kind": "new",
    "text": "Import old group chats with their cast and unknown speakers"
   },
   {
    "kind": "new",
    "text": "Import old memories, summaries and embeddings with their chats"
   },
   {
    "kind": "new",
    "text": "Import old companion chats into the shared pool with state, episodes, Soul facts and notes"
   },
   {
    "kind": "new",
    "text": "Import the old app's usage records, daily usage, providers, models, voices and settings"
   },
   {
    "kind": "new",
    "text": "Import the old app's LoRA library and playground entries"
   },
   {
    "kind": "new",
    "text": "Import the old app's per-message speed stats and chat attachments"
   },
   {
    "kind": "new",
    "text": "Import the old app's Hugging Face and CivitAI tokens"
   },
   {
    "kind": "new",
    "text": "Concurrent old message edits import as conversation forks you can choose between"
   },
   {
    "kind": "new",
    "text": "Large old media libraries migrate"
   },
   {
    "kind": "new",
    "text": "Repeating an import from the same old source is recognized instead of duplicating it"
   },
   {
    "kind": "new",
    "text": "Old databases from releases 2.2.0 to 2.2.5 are accepted"
   },
   {
    "kind": "new",
    "text": "Read and write Character Card V1 to V3 files"
   },
   {
    "kind": "new",
    "text": "Read and write UEC character and persona cards, World Info and USC lorebooks"
   },
   {
    "kind": "new",
    "text": "Import and export SillyTavern JSONL chat transcripts, including multi-speaker chats as groups"
   },
   {
    "kind": "new",
    "text": "Import a character file with its lorebooks, notes and images as a new character"
   },
   {
    "kind": "new",
    "text": "Export a character as UEC or a character card"
   },
   {
    "kind": "new",
    "text": "Character files can carry a companion's shared memory"
   },
   {
    "kind": "new",
    "text": "Import and export prompt templates, chat templates and model profiles"
   },
   {
    "kind": "new",
    "text": "Import and export persona files"
   },
   {
    "kind": "new",
    "text": "UEC 3 lorebook and entity files"
   },
   {
    "kind": "new",
    "text": "Platform file pickers and Android storage access"
   },
   {
    "kind": "new",
    "text": "Disabled template lorebooks show in the import preview"
   },
   {
    "kind": "new",
    "text": "Characters embedded in PNG cards keep their picture as the avatar"
   },
   {
    "kind": "new",
    "text": "A card's embedded character book becomes a lorebook bound to the character"
   },
   {
    "kind": "fixed",
    "text": "Imported group chats keep their mode, memory, lorebook, model settings and starting scene"
   },
   {
    "kind": "fixed",
    "text": "Imported companions keep their settings and Soul facts even when some data is broken"
   },
   {
    "kind": "fixed",
    "text": "Old chats whose branch links dangle are kept"
   },
   {
    "kind": "fixed",
    "text": "An import from the old app that leaves parts out is marked partial"
   },
   {
    "kind": "fixed",
    "text": "Imported selected scenes are kept when the chat does not open with them"
   },
   {
    "kind": "fixed",
    "text": "Old memory texts without an embedding stay visible"
   },
   {
    "kind": "fixed",
    "text": "Cards with blank tags, odd localized notes and long names import"
   },
   {
    "kind": "fixed",
    "text": "Lorebook entry positions written as numbers in character cards are accepted"
   },
   {
    "kind": "fixed",
    "text": "Chat template names import whole instead of being cut at 256 characters"
   },
   {
    "kind": "fixed",
    "text": "Many custom headers and long speech dictionary fields are no longer rejected"
   },
   {
    "kind": "fixed",
    "text": "Backup, restore and import no longer cap record counts"
   },
   {
    "kind": "fixed",
    "text": "Backups made with a password handle surrounding spaces in the password"
   },
   {
    "kind": "fixed",
    "text": "Restored usage keeps usage from deleted chats and rewound history"
   },
   {
    "kind": "fixed",
    "text": "Restore stays responsive and shows completion percentages"
   },
   {
    "kind": "fixed",
    "text": "A restored device rejoins sync as a new device"
   },
   {
    "kind": "fixed",
    "text": "Pairing stays available after device and socket failures"
   },
   {
    "kind": "fixed",
    "text": "Android exports are saved through Downloads"
   },
   {
    "kind": "fixed",
    "text": "Unreadable database files are listed instead of failing startup"
   },
   {
    "kind": "fixed",
    "text": "Symlinked app data directories are resolved on startup"
   },
   {
    "kind": "fixed",
    "text": "A persona file's avatar is deleted when its import fails"
   }
  ]
 },
 {
  "area": "App and settings",
  "items": [
   {
    "kind": "new",
    "text": "Onboarding flow"
   },
   {
    "kind": "new",
    "text": "Home screen with character cards, layouts and long-press menus"
   },
   {
    "kind": "new",
    "text": "Collapsible sidebar with recent chats and a create sheet"
   },
   {
    "kind": "new",
    "text": "Create sheet offers lorebook creation"
   },
   {
    "kind": "new",
    "text": "Settings sidebar, section list and About page"
   },
   {
    "kind": "new",
    "text": "Memory settings page"
   },
   {
    "kind": "new",
    "text": "Image generation and local image management settings"
   },
   {
    "kind": "new",
    "text": "Reply helper settings page"
   },
   {
    "kind": "new",
    "text": "Speech recognition settings page"
   },
   {
    "kind": "new",
    "text": "Companions and chat defaults settings"
   },
   {
    "kind": "new",
    "text": "Voices page with Kokoro setup, library and blends"
   },
   {
    "kind": "new",
    "text": "Local LLM performance page, with per-model comparison"
   },
   {
    "kind": "new",
    "text": "Usage analytics dashboard with statistics and CSV export"
   },
   {
    "kind": "new",
    "text": "Usage tracks cost, including provider-reported billing and OpenRouter routed job costs"
   },
   {
    "kind": "new",
    "text": "Usage tracks cached and reasoning tokens plus image and audio tokens"
   },
   {
    "kind": "new",
    "text": "Focused app time is counted per day"
   },
   {
    "kind": "new",
    "text": "Log viewer and unified prompt pickers"
   },
   {
    "kind": "new",
    "text": "Help, release notes and setup guide"
   },
   {
    "kind": "new",
    "text": "Security page"
   },
   {
    "kind": "new",
    "text": "Reset page"
   },
   {
    "kind": "new",
    "text": "Developer page with ready-made test chats"
   },
   {
    "kind": "new",
    "text": "Choose models for the creation helper, lorebook entry writer and Soul writer"
   },
   {
    "kind": "new",
    "text": "Set a manual context window"
   },
   {
    "kind": "new",
    "text": "Theme, chat and feedback settings"
   },
   {
    "kind": "new",
    "text": "Group chats have their own appearance tab and a real preview"
   },
   {
    "kind": "new",
    "text": "The detail sidebar choice moves to Customization"
   },
   {
    "kind": "new",
    "text": "Pick a light or dark mode, an accent and a theme from previews"
   },
   {
    "kind": "new",
    "text": "Carbon Blue and Carbon Red presets"
   },
   {
    "kind": "new",
    "text": "Translations for 19 languages"
   },
   {
    "kind": "new",
    "text": "Hungarian, Czech, Ukrainian and Thai translations, detected automatically"
   },
   {
    "kind": "new",
    "text": "Downloads menu in the title bar"
   },
   {
    "kind": "new",
    "text": "New shell navigation with rounded window controls"
   },
   {
    "kind": "new",
    "text": "Android back, system language and opened files are handled"
   },
   {
    "kind": "new",
    "text": "Mobile tabs are limited to main screens"
   },
   {
    "kind": "new",
    "text": "System app icon appearances"
   },
   {
    "kind": "new",
    "text": "Haptics"
   },
   {
    "kind": "new",
    "text": "The desktop app runs as a single instance"
   },
   {
    "kind": "new",
    "text": "Credentials are kept in native keyrings and protected by Android Keystore"
   },
   {
    "kind": "new",
    "text": "Credentials are kept in a passphrase vault when no OS credential store exists"
   },
   {
    "kind": "new",
    "text": "Deleting chats and characters also removes media nothing else uses"
   },
   {
    "kind": "new",
    "text": "Chats that need a missing optional model fail with a clear message"
   },
   {
    "kind": "new",
    "text": "The app shows when jobs or generations are running"
   },
   {
    "kind": "new",
    "text": "Generation left in flight by a previous run is settled so chats accept new turns"
   },
   {
    "kind": "new",
    "text": "Running work is cancelled and the diffusion server stops when the app exits"
   },
   {
    "kind": "new",
    "text": "The app version shows the CUDA build"
   },
   {
    "kind": "new",
    "text": "Daily log files and a report file for every crash"
   },
   {
    "kind": "new",
    "text": "Diagnostics redact personal data and report embedding health"
   },
   {
    "kind": "new",
    "text": "NanoGPT quota usage is shown"
   },
   {
    "kind": "new",
    "text": "Local generation metrics can be listed and cleared"
   },
   {
    "kind": "new",
    "text": "Deleting a lorebook or prompt removes its references"
   },
   {
    "kind": "fixed",
    "text": "Floating create buttons stay above the system navigation bar"
   },
   {
    "kind": "fixed",
    "text": "Phone screens fit the system bars and mobile settings are tidier"
   },
   {
    "kind": "fixed",
    "text": "The page no longer scrolls past its content"
   },
   {
    "kind": "fixed",
    "text": "Responsive grids use a single column on narrow screens"
   },
   {
    "kind": "fixed",
    "text": "Long unbroken text wraps in logs and recent chats"
   },
   {
    "kind": "fixed",
    "text": "Log rows stack on phones instead of overflowing"
   },
   {
    "kind": "fixed",
    "text": "Recent chats no longer allow text selection"
   },
   {
    "kind": "fixed",
    "text": "Skip anyway returns when no chat model is set up"
   },
   {
    "kind": "fixed",
    "text": "The no image model notice shows as a row and empty model pickers are disabled"
   },
   {
    "kind": "fixed",
    "text": "Character exports ask where to save before writing"
   },
   {
    "kind": "fixed",
    "text": "Tags and keywords separated by commas split into separate entries"
   },
   {
    "kind": "fixed",
    "text": "The first launch detects the system language without blocking startup"
   },
   {
    "kind": "fixed",
    "text": "The splash screen stays visible until the backend is ready"
   },
   {
    "kind": "fixed",
    "text": "The webview resizes above the keyboard on Android"
   },
   {
    "kind": "fixed",
    "text": "Exports and backups save to Downloads on Android"
   },
   {
    "kind": "fixed",
    "text": "The adaptive icon stays inside its safe zone"
   },
   {
    "kind": "fixed",
    "text": "The background work notification is reworded"
   },
   {
    "kind": "fixed",
    "text": "The OpenRouter logo is updated"
   },
   {
    "kind": "fixed",
    "text": "The app data directory opens on Windows"
   },
   {
    "kind": "fixed",
    "text": "Local generation stats stay on a message past metrics retention and clearing"
   },
   {
    "kind": "fixed",
    "text": "The app refuses to open a database written by a newer build"
   },
   {
    "kind": "fixed",
    "text": "The executable is named lettuceai"
   }
  ]
 }
];
