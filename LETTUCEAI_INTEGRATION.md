# LettuceAI Mobile App — Engine Integration Plan

This document covers the complete integration between the **LettuceAI mobile app** (Tauri + React) and **Lettuce Engine** as a character backend. It includes the setup wizard flow, endpoint reference, feature specifications, and UI behavior guidelines.

> **Full API reference:** See [`CONFIG_API.md`](./CONFIG_API.md) for the complete Engine API with request/response schemas.

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Provider Registration Flow](#2-provider-registration-flow)
3. [Engine Setup Wizard](#3-engine-setup-wizard)
4. [Step 1: Provider & Model Configuration](#4-step-1-provider--model-configuration)
5. [Step 2: Character Creation](#5-step-2-character-creation)
6. [Step 3: Global Settings](#6-step-3-global-settings)
7. [Engine Home Page](#7-engine-home-page)
8. [Engine-Backed Character Behavior](#8-engine-backed-character-behavior)
9. [Chat Integration & Persona Passthrough](#9-chat-integration--persona-passthrough)
10. [Observability Panel](#10-observability-panel)
11. [Complete Endpoint Reference](#11-complete-endpoint-reference)
12. [WebSocket Reference](#12-websocket-reference)
13. [Schema Reference](#13-schema-reference)
14. [Error Handling](#14-error-handling)

---

## 1. Architecture Overview

```
LettuceAI Mobile App (Tauri + React)
    |
    |  HTTP REST + WebSocket
    |  Authorization: Bearer <API_KEY>
    |
Lettuce Engine API (FastAPI, default port 8000)
    |
    ├── LLM Backends (Anthropic, OpenAI, OpenRouter, Ollama)
    ├── SQLite + ChromaDB per character
    ├── Knowledge Graph (NetworkX)
    ├── Emotion Engine (Plutchik 8-dimension)
    ├── Research Loop (web scraping → memory)
    └── Background Loops (synthesis, consolidation, BM25, drip research)
```

### Key Difference from Standard LLM Providers

The Engine is NOT just an LLM proxy. It is a **full identity system**. For Engine-backed characters, the app must:

- **Disable roleplay features** — scenes, lorebooks, character rules are all handled internally by the Engine
- **Send personas as user identity** — the app's Persona maps to the Engine's per-user relationship tracking
- **Display Engine observability** — memory, emotions, relationships, knowledge, activity, live logs
- **Use the Engine's character system** — characters are defined in the Engine, not in the app

---

## 2. Provider Registration Flow

### How users add Engine as a provider

The Engine appears in the app's **Providers page** alongside Anthropic, OpenAI, etc. When a user adds it:

1. User opens **Settings > Providers > Add Provider**
2. User selects **"Lettuce Engine"** from the provider list
3. User enters:
   - **Engine URL** — e.g. `http://localhost:8000` or a remote URL
   - **API Key** — the Bearer token for auth (optional in dev mode)
4. App calls `GET {engine_url}/health` to validate the connection
5. App calls `GET {engine_url}/setup/status` to check if setup is needed
6. Provider is saved in app settings

### Post-save behavior

After saving, the app checks `setup/status`:

```json
{
  "needs_setup": true,
  "configured_providers": [],
  "has_api_key": false
}
```

**If `needs_setup` is `true`:**
- Show a bottom sheet / modal: **"This is a new Engine — let's set it up!"**
- Button: **"Start Setup"** → navigates to Engine Setup Wizard
- The bottom sheet should feel welcoming, not alarming

**If `needs_setup` is `false`:**
- Engine is already configured, go directly to the Engine Home Page

---

## 3. Engine Setup Wizard

A multi-step wizard that configures the Engine from scratch. The wizard lives inside the LettuceAI app, but communicates with the Engine API.

### Wizard Steps

```
[Welcome] → [1. LLM Provider] → [2. Characters] → [3. Settings] → [Engine Home]
```

### Welcome Screen

- **Title:** "Welcome to Lettuce Engine"
- **Subtitle:** "Let's configure your AI character engine. This will take about 2 minutes."
- **Body:** Brief explanation of what the Engine does:
  - "The Engine gives your AI characters persistent memory, emotions, relationships, and a real identity."
  - "First, we'll set up an LLM backend, then create your characters."
- **Button:** "Let's Go" → Step 1

---

## 4. Step 1: Provider & Model Configuration

The Engine needs at least one LLM backend to function. The user picks a provider and configures its model.

### UI Behavior

1. Show a list of available providers: **Anthropic, OpenAI, OpenRouter, Ollama**
2. User selects one or more providers
3. For each selected provider, show configuration:
   - **Model** (text input or dropdown — see Model System below)
   - **API Key** (for Anthropic, OpenAI, OpenRouter — not Ollama)
   - **Base URL** (for Ollama only, defaults to `http://localhost:11434`)
   - **Max Tokens** (slider, default 1024)
   - **Temperature** (slider, default 0.9)
4. User picks a **default backend** from the configured providers
5. Save configuration

### API Calls

```
PUT /config/llm/{provider}
Body: {
  "model": "claude-sonnet-4-5-20250929",
  "api_key": "sk-ant-...",
  "max_tokens": 1024,
  "temperature": 0.9
}

PUT /config/llm/default
Body: { "provider": "anthropic" }
```

### Model System Considerations

The Engine currently accepts any model string. The LettuceAI app should maintain its own model catalog (since it already has one for its other providers) and present models in a dropdown. The Engine stores whatever model string is sent.

**Recommended models for Engine use:**
- Anthropic: `claude-sonnet-4-5-20250929` (best balance of quality and speed for characters)
- OpenAI: `gpt-4o` or `gpt-4o-mini`
- OpenRouter: any model available on the platform
- Ollama: local models like `llama3`, `mistral`, `qwen2.5`

### Validation

Before proceeding to Step 2, verify at least one provider is configured:

```
GET /setup/status
→ { "needs_setup": false, "configured_providers": ["anthropic"], ... }
```

If `needs_setup` is still `true`, show an error: "Please configure at least one LLM provider."

---

## 5. Step 2: Character Creation

Users create one or more characters. They can fill in every field manually **or** use the AI Booster for auto-generation.

### UI Layout

Two paths presented side by side or as tabs:

1. **Manual Creation** — full form with all character fields
2. **AI Boost** — enter a name + brief description, AI generates everything

### Manual Creation

Fetch the template schema to build the form dynamically:

```
GET /characters/template
→ {
    "fields": {
      "name": {"type": "string", "required": true, "description": "..."},
      "era": {"type": "string", "default": "modern", "description": "..."},
      ...
    },
    "template_yaml": "# Lettuce Engine — Original Character Template\n..."
  }
```

The `fields` object describes every field with its type, default value, and description. Use this to build form inputs:

| Field | Input Type | Notes |
|-------|-----------|-------|
| `name` | Text input | **Required.** Full name. |
| `era` | Dropdown | modern, Victorian, medieval, or custom text |
| `setting` | Textarea | First-person, where they are now |
| `role` | Text input | Occupation or position |
| `core_identity` | Textarea | First-person identity paragraph |
| `backstory` | Textarea | First-person life story |
| `personality_traits` | Tag/chip input | 4-6 traits |
| `speech_patterns` | Nested form | See sub-fields below |
| `speech_patterns.formality` | Radio/dropdown | formal, casual, texting |
| `speech_patterns.verbosity` | Radio/dropdown | terse, medium, verbose |
| `speech_patterns.text_style` | Radio/dropdown | formal, casual, texting |
| `speech_patterns.dialect` | Text input | e.g. "Southern US" |
| `speech_patterns.catchphrases` | Tag input | Phrases they say |
| `speech_patterns.vocabulary_preferences` | Tag input | Words they use |
| `speech_patterns.vocabulary_avoidances` | Tag input | Words they avoid |
| `speech_patterns.filler_words` | Tag input | Natural filler words |
| `speech_patterns.example_quotes` | List of text inputs | 3-5 example lines |
| `knowledge_domains` | Tag input | Topics they know |
| `knowledge_boundaries` | Tag input | What they don't know |
| `research_seeds` | Tag input | URLs or search terms |
| `research_enabled` | Toggle switch | Enable/disable research |
| `physical_description` | Textarea | Appearance |
| `physical_habits` | Tag input | Mannerisms |
| `idle_behaviors` | Tag input | Idle activities |
| `time_behaviors.*` | Text inputs | Activity per time period |
| `baseline_emotions.*` | Sliders (0-1) | 8 Plutchik dimensions |
| `backend` | Dropdown | Optional LLM override |
| `model` | Dropdown | Optional model override |
| `temperature` | Slider | Optional temperature override |

### AI Boost (Auto-fill)

The boost flow:

1. User enters:
   - **Name** (optional — AI can generate)
   - **Seed description** (required) — e.g. "jazz pianist, 1950s Harlem" or "retired marine biologist, lives on a houseboat in Key West"
   - **Era** (optional, default: modern)
2. User clicks **"Generate with AI"**
3. **Show a loading state** — the boost takes 5-15 seconds depending on the LLM
   - Spinner or skeleton UI
   - Message: "Generating character..."
   - **Do NOT navigate away.** Keep the user on this page.
4. When ready, **display the full generated character for review**
   - All fields pre-filled in the form (same form as manual creation)
   - User can edit any field before saving
   - Highlight fields that were AI-generated (subtle visual indicator)
5. User clicks **"Save Character"** or **"Regenerate"**

### Boost API Call

```
POST /characters/boost
Body: {
  "name": "Clarence 'Keys' Washington",
  "seed": "jazz pianist, 1950s Harlem, played at Minton's Playhouse",
  "era": "1950s"
}
→ {
    "character": { ...full CharacterCreateRequest... },
    "status": "generated"
  }
```

The response contains a complete `CharacterCreateRequest` object. Display it in the form for user approval. It is NOT saved until the user explicitly saves.

### Saving a Character

```
POST /characters
Body: { ...CharacterCreateRequest... }
→ 201 Created
  { ...CharacterFullResponse with slug, loaded: false... }
```

### Character List During Setup

Show created characters in a list with:
- Name, role, era
- "Edit" and "Delete" buttons
- "Add Another Character" button

When user has at least one character, show "Continue to Settings" button.

### Relevant Endpoints for Character Management

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/characters` | List all characters |
| `GET` | `/characters/template` | Get form schema |
| `GET` | `/characters/{slug}` | Basic info |
| `GET` | `/characters/{slug}/full` | Full character definition |
| `POST` | `/characters` | Create character |
| `PUT` | `/characters/{slug}` | Update character |
| `DELETE` | `/characters/{slug}` | Delete character |
| `POST` | `/characters/boost` | AI auto-generate |
| `POST` | `/characters/{slug}/load` | Load into memory |
| `POST` | `/characters/{slug}/unload` | Unload from memory |
| `PUT` | `/characters/{slug}/research` | Toggle research |

---

## 6. Step 3: Global Settings

Configure engine-wide settings. These are optional — sensible defaults are already set.

### Settings Form

| Setting | API Path | Type | Default | Description |
|---------|----------|------|---------|-------------|
| Data directory | `PUT /config/engine` → `data_dir` | Text | `./data` | Where character databases are stored |
| Log level | `PUT /config/engine` → `log_level` | Dropdown | `INFO` | DEBUG, INFO, WARNING, ERROR |
| Max history | `PUT /config/engine` → `max_history` | Number | 40 | Max conversation turns in context window |
| Synthesis interval | `PUT /config/background` → `synthesis_interval_minutes` | Number | 10 | How often to generate memories from conversations |
| Consolidation interval | `PUT /config/background` → `consolidation_interval_minutes` | Number | 60 | How often to cluster/prune memories |
| BM25 rebuild interval | `PUT /config/background` → `bm25_rebuild_interval_minutes` | Number | 15 | How often to rebuild search index |
| Drip research interval | `PUT /config/background` → `drip_research_interval_minutes` | Number | 60 | How often to research new topics |
| Embedding model | `PUT /config/memory` → `embedding_model` | Text | `all-MiniLM-L6-v2` | Sentence transformer model |
| Max retrieval results | `PUT /config/memory` → `max_retrieval_results` | Number | 15 | Memories retrieved per query |
| Dense weight | `PUT /config/memory` → `dense_weight` | Slider (0-1) | 0.5 | Weight for vector similarity |
| BM25 weight | `PUT /config/memory` → `bm25_weight` | Slider (0-1) | 0.3 | Weight for keyword matching |
| Graph weight | `PUT /config/memory` → `graph_weight` | Slider (0-1) | 0.2 | Weight for knowledge graph |

### API Calls

```
PUT /config/engine
Body: { "data_dir": "./data", "log_level": "INFO", "max_history": 40 }

PUT /config/background
Body: { "synthesis_interval_minutes": 10, ... }

PUT /config/memory
Body: { "embedding_model": "all-MiniLM-L6-v2", ... }
```

### Completing Setup

After settings are saved, call:

```
POST /setup/complete
→ { "status": "ok" }
```

This marks setup as done. Navigate to the Engine Home Page.

---

## 7. Engine Home Page

A dedicated dashboard for the Engine. This is the main landing page when users navigate to their Engine provider.

### Layout Sections

#### 7.1 Header

- Engine name + version (`GET /health` → `version`)
- Connection status indicator (green/red dot)
- Setup status badge

#### 7.2 Characters Overview

For each character, show a card with:
- Name, role, era
- Loaded status (green = loaded, gray = not loaded)
- Quick stats (if loaded): memory count, emotion, turns
- Load/Unload button
- Click → Character Detail page

Data source: `GET /status`

#### 7.3 Usage & Cost Dashboard

Display token usage across all characters:

```
GET /usage
→ {
    "characters": [
      {
        "character": "Samuel Thompson",
        "total_calls": 142,
        "total_input_tokens": 285000,
        "total_output_tokens": 71000,
        "total_tokens": 356000,
        "by_model": [
          {"model": "claude-sonnet-4-5-20250929", "backend": "anthropic", "calls": 142, ...}
        ],
        "by_source": [
          {"source": "chat", "calls": 120, ...},
          {"source": "synthesis", "calls": 15, ...},
          {"source": "research", "calls": 7, ...}
        ]
      }
    ],
    "total_calls": 142,
    "total_input_tokens": 285000,
    "total_output_tokens": 71000,
    "total_tokens": 356000
  }
```

**Display ideas:**
- Total tokens bar chart (input vs output)
- Per-character breakdown pie chart
- Per-source breakdown (chat vs synthesis vs research) — helps users understand "hidden" token usage from background loops
- Cost estimation: multiply tokens by per-model pricing (maintained in the app, not the Engine)
- Daily/weekly trends (store historical snapshots in app's local storage)

#### 7.4 Background Activity

Show the status of background loops per character:

```
GET /characters/{slug}/activity
→ {
    "loops_running": true,
    "synthesis": {"last_run": "2025-01-15T14:30:00Z", "interval_minutes": 10, "status": "running"},
    "consolidation": {"last_run": "2025-01-15T14:00:00Z", "interval_minutes": 60, "status": "running"},
    "bm25_rebuild": {"last_run": "2025-01-15T14:25:00Z", "interval_minutes": 15, "status": "running"},
    "drip_research": {"last_run": "2025-01-15T13:30:00Z", "interval_minutes": 60, "status": "running"}
  }
```

Show each loop as a row with:
- Name, status (running/stopped), last run time, interval
- "Time until next run" countdown

#### 7.5 Quick Actions

- **Add Character** → Character creation page
- **Configure Providers** → Standalone LLM providers settings page (`/settings/engine/{id}/providers`)
- **Engine Settings** → Standalone engine settings page (`/settings/engine/{id}/settings`)
- **View Logs** → Live log viewer

#### 7.6 Standalone Settings Pages

Two dedicated settings pages allow editing Engine configuration without going through the setup wizard:

**LLM Providers Config** (`EngineProvidersConfigPage`)
- Route: `/settings/engine/:credentialId/providers`
- Loads current config from `GET /config` → `llm` section
- Shows each provider (Anthropic, OpenAI, OpenRouter, Ollama) with enable/disable, model, API key, max tokens, temperature
- Import from app providers feature
- Default backend selection
- Saves via `PUT /config/llm/{provider}`, `PUT /config/llm/default`, `DELETE /config/llm/{provider}`

**Engine Settings Config** (`EngineSettingsConfigPage`)
- Route: `/settings/engine/:credentialId/settings`
- Loads current config from `GET /config` → engine, background, memory, safety, research sections
- Sections: Engine (data dir, log level, max history), Background Loops (intervals), Memory (weights, embedding model), Safety (honesty, data deletion), Research (scrape on boot, interval)
- Saves via `PUT /config/engine`, `PUT /config/background`, `PUT /config/memory`, `PUT /config/safety`, `PUT /config/research`

See `CONFIG_API.md` for the full API reference including response shapes.

---

## 8. Engine-Backed Character Behavior

When a character is backed by the Engine (not a raw LLM provider), the app must adjust its behavior.

### Features to DISABLE

These are handled internally by the Engine and should be hidden/disabled in the UI:

| Feature | Why |
|---------|-----|
| **Scenes / Scenarios** | Engine characters maintain their own setting and context |
| **Lorebooks** | Engine has its own knowledge graph + research loop |
| **Character Rules / Instructions** | Identity is defined in the Engine's character YAML |
| **System prompt editing** | Engine builds its own 9-section system prompt |
| **Memory management** (app-side) | Engine has its own SQLite + ChromaDB + BM25 memory system |
| **Temperature / Max tokens** (per-chat) | Set in Engine config, not per-request |

### Features to ENABLE / SHOW

| Feature | What to show |
|---------|-------------|
| **Observability panel** | Memory, emotions, relationships, knowledge, research, activity |
| **Persona as identity** | Send the active Persona as `user_description` |
| **Engine character selector** | List from `GET /characters` instead of app's local characters |
| **Research toggle** | Per-character research on/off |
| **Character editor** | Edit the full Engine character definition |

### How to detect Engine-backed characters

When the user selects a character for chat, check if the current provider is "Lettuce Engine". If so, use Engine endpoints for chat and disable roleplay features.

---

## 9. Chat Integration & Persona Passthrough

### Sending Messages

```
POST /characters/{slug}/chat
Headers: Authorization: Bearer <KEY>
Body: {
  "message": "Hey Sam, what's on your mind today?",
  "user_id": "user_abc123",
  "user_name": "Alex",
  "user_description": "A 25-year-old software developer who enjoys hiking, coffee, and sci-fi novels. Tends to be curious and asks a lot of questions."
}
→ {
    "response": "honestly not much, just been staring at this...",
    "character": "Samuel Thompson",
    "emotion": "anticipation",
    "emotion_intensity": 0.35
  }
```

### Persona Mapping

The app's **Persona** system maps directly to Engine fields:

| App Persona Field | Engine Chat Field | Purpose |
|-------------------|-------------------|---------|
| Persona ID | `user_id` | Unique user identity for relationship tracking |
| Persona name | `user_name` | Display name the character uses |
| Persona description | `user_description` | Injected into relationship context so the character knows who they're talking to |

**Important:** Each Persona should have a consistent `user_id` across sessions. This is how the Engine tracks relationships, familiarity, trust, etc. If you send a different `user_id`, the Engine treats it as a different person.

### Streaming Chat (WebSocket)

For real-time streaming responses:

```
Connect: ws://{engine_url}/ws/chat/{slug}?token={API_KEY}

Send:
{
  "type": "message",
  "content": "Hey Sam",
  "user_id": "user_abc123",
  "user_name": "Alex",
  "user_description": "A 25-year-old software developer..."
}

Receive:
{"type": "stream_start", "character": "Samuel Thompson"}
{"type": "stream_chunk", "content": "honestly "}
{"type": "stream_chunk", "content": "not much, "}
...
{"type": "stream_end", "content": "honestly not much...", "emotion": "anticipation", "emotion_intensity": 0.35}
```

### Conversation History

```
GET /characters/{slug}/history/{user_id}?limit=50
→ [
    {
      "id": "...",
      "user_id": "user_abc123",
      "user_name": "Alex",
      "role": "user",
      "content": "Hey Sam",
      "timestamp": "2025-01-15T14:30:00Z",
      "entities_mentioned": ["Sam"]
    },
    ...
  ]
```

---

## 10. Observability Panel

A collapsible side panel or bottom drawer that shows real-time Engine state for the active character. This is the main value-add of Engine integration.

### 10.1 Memory View

```
GET /characters/{slug}/memory
→ {
    "total_sqlite": 245,
    "total_vector": 230,
    "by_type": {"EPISODIC": 80, "SEMANTIC": 95, "EMOTIONAL": 30, "CONVERSATION": 40},
    "by_source": {"GENERATED": 50, "CANONICAL": 20, "CONVERSATION": 120, "CONSOLIDATED": 25, "RESEARCH": 30},
    "recent": [
      {
        "id": "...",
        "content": "Alex mentioned they're working on a new hiking app...",
        "type": "CONVERSATION",
        "source": "CONVERSATION",
        "importance": 0.3,
        "created_at": "2025-01-15T14:30:00Z",
        "access_count": 2,
        "tags": ["alex", "hiking", "technology"]
      }
    ]
  }
```

**Display:**
- Total memory count (gauge or number)
- Type breakdown as horizontal stacked bar or donut chart
- Source breakdown as horizontal stacked bar or donut chart
- Recent memories as a scrollable list (show content, type badge, importance indicator)

### 10.2 Emotion View

```
GET /characters/{slug}/emotions
→ {
    "current": {"joy": 0.4, "trust": 0.5, "fear": 0.1, "surprise": 0.2, "sadness": 0.1, "disgust": 0.05, "anger": 0.05, "anticipation": 0.6},
    "baseline": {"joy": 0.3, "trust": 0.3, "fear": 0.2, "surprise": 0.2, "sadness": 0.2, "disgust": 0.2, "anger": 0.2, "anticipation": 0.3},
    "primary_emotion": "anticipation",
    "secondary_emotion": "trust",
    "intensity": 0.25,
    "valence": 0.52,
    "description": "I am feeling quite anticipation, with an undercurrent of trust."
  }
```

**Display:**
- Radar/spider chart showing all 8 Plutchik dimensions
  - Current state as filled area
  - Baseline as dotted outline (shows deviation)
- Primary + secondary emotion badges
- Intensity meter (0-1 gauge)
- Valence indicator (negative ← neutral → positive)
- Natural language description at the bottom

### 10.3 Relationships View

```
GET /characters/{slug}/relationships
→ {
    "character": "Samuel Thompson",
    "relationships": [
      {
        "user_id": "user_abc123",
        "user_name": "Alex",
        "familiarity": 0.45,
        "trust": 0.55,
        "affection": 0.4,
        "respect": 0.5,
        "interaction_count": 23,
        "character_notes": "Alex is curious and asks good questions about my work.",
        "topics_discussed": ["hiking", "music", "philosophy", "technology"],
        "first_interaction": "2025-01-01T10:00:00Z",
        "last_interaction": "2025-01-15T14:30:00Z"
      }
    ]
  }
```

**Display:**
- List of relationships, one per user
- Each relationship as a card:
  - User name, interaction count, first/last interaction dates
  - 4 dimension bars: familiarity, trust, affection, respect (0-1 progress bars)
  - Character notes (italic, from character's perspective)
  - Topics discussed as tag chips
- Highlight the current user's relationship (match by `user_id`)

### 10.4 Knowledge Graph View

```
GET /characters/{slug}/knowledge
→ {
    "node_count": 47,
    "edge_count": 83,
    "entities_by_type": {"person": 12, "concept": 18, "place": 8, "organization": 5, "date": 4},
    "fact_count": 115,
    "recent_entities": ["Alex", "hiking trails", "Appalachian Mountains", ...]
  }
```

**Display:**
- Node/edge/fact counts as stat cards
- Entity type breakdown as bar chart or tag cloud
- Recent entities as a scrollable list

### 10.5 Research Status

```
GET /characters/{slug}/research/status
→ {
    "enabled": true,
    "seeds_total": 15,
    "seeds_researched": 12,
    "seeds_pending": 3,
    "sources_scraped": 42,
    "total_facts_extracted": 187,
    "total_memories_generated": 95,
    "recent_sources": [
      {"url": "https://en.wikipedia.org/wiki/...", "seed": "jazz history", "scraped_at": "...", "facts": 12}
    ]
  }
```

**Display:**
- Research enabled/disabled toggle (calls `PUT /characters/{slug}/research`)
- Progress bar: seeds researched / total seeds
- Stats: sources scraped, facts extracted, memories generated
- Recent sources as a list with URL, seed, fact count

### 10.6 Background Activity

```
GET /characters/{slug}/activity
```

(See Section 7.4 for response format and display.)

### 10.7 Live Logs

```
WebSocket: ws://{engine_url}/ws/logs/{slug}?token={API_KEY}
```

Receives structured log events:

```json
{
  "timestamp": "2025-01-15T14:30:00Z",
  "level": "INFO",
  "event": "llm_response",
  "data": {"attempt": 0, "tokens_in": 1500, "tokens_out": 350}
}
```

**Display:**
- Scrolling log viewer (like a terminal)
- Color-coded by level: DEBUG (gray), INFO (blue), WARNING (yellow), ERROR (red)
- Filter by level
- Auto-scroll with manual scroll override
- Keepalive pings every 30 seconds (event: "keepalive") — ignore these in display

---

## 11. Complete Endpoint Reference

All endpoints require `Authorization: Bearer <KEY>` unless noted.

### System

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/health` | No | Health check + version |
| `GET` | `/status` | Yes | Full system status with all characters |
| `GET` | `/usage` | Yes | Global token usage across all loaded characters |

### Setup & Configuration

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/setup/status` | No | Check if setup is needed |
| `POST` | `/setup/complete` | No | Mark setup as complete |
| `GET` | `/config` | Yes | Get all settings (keys redacted) |
| `PUT` | `/config/llm/{provider}` | Yes | Configure an LLM provider |
| `DELETE` | `/config/llm/{provider}` | Yes | Remove an LLM provider |
| `PUT` | `/config/llm/default` | Yes | Set default LLM backend |
| `PUT` | `/config/engine` | Yes | Update engine settings |
| `PUT` | `/config/memory` | Yes | Update memory settings |
| `PUT` | `/config/background` | Yes | Update background loop settings |
| `PUT` | `/config/safety` | Yes | Update safety settings |
| `PUT` | `/config/research` | Yes | Update research settings |

### Characters

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/characters` | Yes | List all characters (summary) |
| `GET` | `/characters/template` | Yes | Get character template schema + YAML |
| `POST` | `/characters` | Yes | Create a new character |
| `POST` | `/characters/boost` | Yes | AI-generate a full character definition |
| `GET` | `/characters/{slug}` | Yes | Get character basic info |
| `GET` | `/characters/{slug}/full` | Yes | Get complete character definition |
| `PUT` | `/characters/{slug}` | Yes | Update character definition |
| `DELETE` | `/characters/{slug}` | Yes | Delete character |
| `POST` | `/characters/{slug}/load` | Yes | Load character engine into memory |
| `POST` | `/characters/{slug}/unload` | Yes | Unload character engine |
| `PUT` | `/characters/{slug}/research` | Yes | Toggle research on/off |
| `GET` | `/characters/{slug}/usage` | Yes | Per-character token usage |

### Chat

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `POST` | `/characters/{slug}/chat` | Yes | Send a message, get response |
| `GET` | `/characters/{slug}/history/{user_id}` | Yes | Conversation history |

### Observability

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/characters/{slug}/memory` | Yes | Memory analytics |
| `GET` | `/characters/{slug}/emotions` | Yes | Emotion state |
| `GET` | `/characters/{slug}/relationships` | Yes | All relationships |
| `GET` | `/characters/{slug}/relationships/{user_id}` | Yes | Single relationship detail |
| `GET` | `/characters/{slug}/knowledge` | Yes | Knowledge graph stats |
| `GET` | `/characters/{slug}/research/status` | Yes | Research progress |
| `GET` | `/characters/{slug}/activity` | Yes | Background loop status |

### User Data

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `DELETE` | `/characters/{slug}/users/{user_id}` | Yes | Delete all user data from a character |

---

## 12. WebSocket Reference

### Chat Streaming

```
ws://{engine_url}/ws/chat/{slug}?token={API_KEY}
```

**Client → Server messages:**
```json
{"type": "message", "content": "...", "user_id": "...", "user_name": "...", "user_description": "..."}
{"type": "ping"}
```

**Server → Client messages:**
```json
{"type": "stream_start", "character": "..."}
{"type": "stream_chunk", "content": "..."}
{"type": "stream_end", "content": "...", "emotion": "...", "emotion_intensity": 0.0}
{"type": "error", "message": "..."}
{"type": "pong"}
```

### Live Logs

```
ws://{engine_url}/ws/logs/{slug}?token={API_KEY}
```

**Server → Client messages:**
```json
{"timestamp": "...", "level": "INFO", "event": "llm_response", "data": {...}}
{"timestamp": "...", "level": "DEBUG", "event": "keepalive", "data": {}}
```

No client-to-server messages needed (read-only stream).

---

## 13. Schema Reference

### CharacterCreateRequest (POST /characters body)

```typescript
interface CharacterCreateRequest {
  name: string;                    // Required
  era?: string;                    // "modern" | "Victorian" | "medieval" | custom
  setting?: string;                // First-person location description
  role?: string;                   // Occupation
  core_identity?: string;          // First-person identity (3-5 sentences)
  backstory?: string;              // First-person life story
  personality_traits?: string[];   // 4-6 traits
  speech_patterns?: {
    formality: "formal" | "casual" | "texting";
    verbosity: "terse" | "medium" | "verbose";
    text_style: "formal" | "casual" | "texting";
    dialect: string;
    catchphrases: string[];
    vocabulary_preferences: string[];
    vocabulary_avoidances: string[];
    filler_words: string[];
    example_quotes: string[];
  };
  knowledge_domains?: string[];
  knowledge_boundaries?: string[];
  research_seeds?: string[];
  physical_description?: string;
  physical_habits?: string[];
  idle_behaviors?: string[];
  time_behaviors?: {
    early_morning: string;
    morning: string;
    afternoon: string;
    evening: string;
    night: string;
  };
  baseline_emotions?: {
    joy: number;      // 0-1
    trust: number;
    fear: number;
    surprise: number;
    sadness: number;
    disgust: number;
    anger: number;
    anticipation: number;
  };
  research_enabled?: boolean;
  backend?: string;              // LLM backend override
  model?: string;                // Model override
  temperature?: number;          // Temperature override
  is_seed?: boolean;             // Seed mode for bootstrapping
  seed_prompt?: string;
}
```

### ChatRequest (POST /characters/{slug}/chat body)

```typescript
interface ChatRequest {
  message: string;
  user_id?: string;           // default: "api_user"
  user_name?: string;         // default: "someone"
  user_description?: string;  // Persona description from mobile app
}
```

### UsageResponse

```typescript
interface UsageResponse {
  character: string;
  total_calls: number;
  total_input_tokens: number;
  total_output_tokens: number;
  total_tokens: number;
  by_model: Array<{
    model: string;
    backend: string;
    calls: number;
    input_tokens: number;
    output_tokens: number;
  }>;
  by_source: Array<{
    source: string;    // "chat" | "synthesis" | "research" | "boost" | ...
    calls: number;
    input_tokens: number;
    output_tokens: number;
  }>;
}
```

---

## 14. Error Handling

### HTTP Status Codes

| Code | Meaning | When |
|------|---------|------|
| `200` | Success | Normal responses |
| `201` | Created | Character created |
| `400` | Bad Request | Invalid input, character not loaded |
| `401` | Unauthorized | Invalid or missing API key |
| `404` | Not Found | Character or resource not found |
| `409` | Conflict | Character name already exists |
| `500` | Internal Error | LLM failure, boost failure, etc. |
| `503` | Service Unavailable | Setup required (setup gate) |

### Error Response Format

```json
{
  "detail": "Human-readable error message"
}
```

### Setup Gate (503)

When setup is not complete, most endpoints return:

```json
{
  "detail": "Setup required",
  "setup_url": "/setup/status"
}
```

**App behavior:** If a 503 with `setup_url` is received, redirect to the setup wizard.

### Connection Failures

If the Engine URL is unreachable:
- Show "Engine Offline" indicator on the Engine Home Page
- Disable chat and observability features
- Keep provider settings accessible so the user can update the URL
