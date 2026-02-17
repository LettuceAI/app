# Lettuce Engine — API Reference

All endpoints except `GET /health` and the setup endpoints require `Authorization: Bearer <API_KEY>` (unless no key is configured, which disables auth entirely). Most endpoints also require setup to be complete — see the setup gate below.

**Setup gate:** Before setup is complete, only `/health`, `/status`, `/setup/*`, `/config/*`, and `/ws/*` paths are accessible. All other endpoints return `503 Setup required`.

---

## Table of Contents

- [Health & Status](#health--status)
- [Setup](#setup)
- [Configuration](#configuration)
- [Characters](#characters)
- [Chat](#chat)
- [Observability](#observability)
- [Usage & User Data](#usage--user-data)
- [WebSocket Endpoints](#websocket-endpoints)

---

## Health & Status

### GET /health

Health check. No auth required.

**Response `200 OK`:**

```json
{
  "status": "ok",
  "version": "1.1.0"
}
```

---

### GET /status

System status dashboard with loaded characters and background config.

**Auth:** Required

**Response `200 OK`:**

```json
{
  "version": "1.1.0",
  "needs_setup": false,
  "default_backend": "openrouter",
  "configured_providers": ["openrouter"],
  "characters": [
    {
      "slug": "samuel_thompson",
      "name": "Samuel Thompson",
      "loaded": true,
      "stats": {
        "backend": "openrouter",
        "memories_vector": 42,
        "memories_sqlite": 58,
        "total_turns": 120,
        "graph_nodes": 35,
        "graph_edges": 48,
        "emotion": "joy",
        "emotion_intensity": 0.65,
        "background_loops": true,
        "research_enabled": true,
        "drift_rate": 0.0
      }
    }
  ],
  "background": {
    "synthesis_interval_minutes": 10,
    "consolidation_interval_minutes": 60,
    "bm25_rebuild_interval_minutes": 15,
    "drip_research_interval_minutes": 60
  }
}
```

`stats` is `null` when the character is not loaded.

---

## Setup

### GET /setup/status

Check whether initial setup is needed. No auth required.

**Response `200 OK`:**

```json
{
  "needs_setup": true,
  "configured_providers": [],
  "has_api_key": false
}
```

---

### POST /setup/complete

Mark setup as complete. Fails if no LLM provider is configured yet.

**Auth:** None

**Request body:** None

**Response `200 OK`:**

```json
{ "status": "ok" }
```

**Errors:**
- `400` — No LLM provider configured.

---

## Configuration

### GET /config

Returns the full configuration with API keys redacted. Providers are nested under `llm.providers` and `default_backend` lives under `llm` (not `engine`).

**Auth:** Required

**Response `200 OK`:**

```json
{
  "llm": {
    "providers": {
      "openrouter": {
        "model": "z-ai/glm-5",
        "api_key": "sk-...7402",
        "max_tokens": 1024,
        "temperature": 0.9
      }
    },
    "default_backend": "openrouter"
  },
  "engine": {
    "data_dir": "./data",
    "log_level": "INFO",
    "max_history": 40
  },
  "background": {
    "synthesis_interval_minutes": 10,
    "consolidation_interval_minutes": 60,
    "bm25_rebuild_interval_minutes": 15,
    "drip_research_interval_minutes": 60
  },
  "memory": {
    "embedding_model": "all-MiniLM-L6-v2",
    "max_retrieval_results": 15,
    "dense_weight": 0.5,
    "bm25_weight": 0.3,
    "graph_weight": 0.2,
    "recency_boost_hours": 2.0,
    "random_surface_probability": 0.05
  },
  "safety": {
    "honesty_section": true,
    "user_data_deletion": true
  },
  "research": {
    "initial_scrape_on_boot": true,
    "periodic_interval_hours": 6
  }
}
```

**Notes:**
- `llm.providers` only contains configured providers. Unconfigured ones are omitted.
- If none are configured: `"providers": {}`, `"default_backend": null`.
- API keys are redacted (e.g. `"sk-...7402"`). Only send a new key on PUT if the user changes it.

---

### PUT /config/llm/{provider}

Configure an LLM provider. Valid providers: `anthropic`, `openai`, `openrouter`, `ollama`.

**Auth:** Required

**Path parameters:**

| Name | Type | Description |
|------|------|-------------|
| `provider` | `string` | One of `anthropic`, `openai`, `openrouter`, `ollama` |

**Request body:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `model` | `string` | yes | — | Model name (e.g. `"claude-sonnet-4-5-20250929"`, `"gpt-4o"`) |
| `api_key` | `string` | no | `null` | API key. Omit to keep existing. |
| `max_tokens` | `integer` | no | `1024` | Max tokens per response |
| `temperature` | `float` | no | `0.9` | Sampling temperature (0–2) |
| `base_url` | `string` | no | `null` | Base URL override (Ollama / self-hosted) |

**Response `200 OK`:** Full provider config as stored.

```json
{
  "model": "claude-sonnet-4-5-20250929",
  "api_key": "sk-ant-...",
  "max_tokens": 1024,
  "temperature": 0.9
}
```

**Errors:**
- `400` — Unknown provider name.

---

### PUT /config/llm/default

Set which configured provider is the default backend.

**Auth:** Required

**Request body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `provider` | `string` | yes | Provider ID to use as default |

**Response `200 OK`:**

```json
{ "default_backend": "anthropic" }
```

**Errors:**
- `400` — Unknown provider, or provider not yet configured.

---

### DELETE /config/llm/{provider}

Remove a configured LLM provider.

**Auth:** Required

**Path parameters:**

| Name | Type | Description |
|------|------|-------------|
| `provider` | `string` | Provider ID to remove |

**Response `200 OK`:**

```json
{ "status": "deleted", "provider": "openai" }
```

**Errors:**
- `404` — Provider not found.

---

### PUT /config/engine

Update engine settings.

**Auth:** Required

**Request body:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `data_dir` | `string` | no | `"./data"` | Path to data directory |
| `log_level` | `string` | no | `"INFO"` | `DEBUG` / `INFO` / `WARNING` / `ERROR` |
| `max_history` | `integer` | no | `40` | Max conversation turns kept in context |

**Response `200 OK`:** Full `engine` section as stored.

```json
{
  "data_dir": "./data",
  "log_level": "INFO",
  "max_history": 40
}
```

---

### PUT /config/background

Update background loop intervals.

**Auth:** Required

**Request body:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `synthesis_interval_minutes` | `integer` | no | `10` | Minutes between synthesis runs |
| `consolidation_interval_minutes` | `integer` | no | `60` | Minutes between consolidation |
| `bm25_rebuild_interval_minutes` | `integer` | no | `15` | Minutes between BM25 rebuilds |
| `drip_research_interval_minutes` | `integer` | no | `60` | Minutes between drip research |

**Response `200 OK`:** Full `background` section as stored.

```json
{
  "synthesis_interval_minutes": 10,
  "consolidation_interval_minutes": 60,
  "bm25_rebuild_interval_minutes": 15,
  "drip_research_interval_minutes": 60
}
```

---

### PUT /config/memory

Update memory retrieval settings.

**Auth:** Required

**Request body:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `embedding_model` | `string` | no | `"all-MiniLM-L6-v2"` | Sentence-transformer model |
| `max_retrieval_results` | `integer` | no | `15` | Max results from hybrid retrieval |
| `dense_weight` | `float` | no | `0.5` | Dense vector search weight (0–1) |
| `bm25_weight` | `float` | no | `0.3` | BM25 keyword search weight (0–1) |
| `graph_weight` | `float` | no | `0.2` | Knowledge graph search weight (0–1) |
| `recency_boost_hours` | `float` | no | `2.0` | Hours within which memories get recency boost |
| `random_surface_probability` | `float` | no | `0.05` | Probability of surfacing a random memory |

**Response `200 OK`:** Full `memory` section as stored.

```json
{
  "embedding_model": "all-MiniLM-L6-v2",
  "max_retrieval_results": 15,
  "dense_weight": 0.5,
  "bm25_weight": 0.3,
  "graph_weight": 0.2,
  "recency_boost_hours": 2.0,
  "random_surface_probability": 0.05
}
```

---

### PUT /config/safety

Update safety settings.

**Auth:** Required

**Request body:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `honesty_section` | `boolean` | no | `true` | Include honesty section in system prompt |
| `user_data_deletion` | `boolean` | no | `true` | Allow users to request data deletion |

**Response `200 OK`:** Full `safety` section as stored.

```json
{
  "honesty_section": true,
  "user_data_deletion": true
}
```

---

### PUT /config/research

Update research settings.

**Auth:** Required

**Request body:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `initial_scrape_on_boot` | `boolean` | no | `true` | Run research scrape on startup |
| `periodic_interval_hours` | `integer` | no | `6` | Hours between periodic research runs |

**Response `200 OK`:** Full `research` section as stored.

```json
{
  "initial_scrape_on_boot": true,
  "periodic_interval_hours": 6
}
```

---

## Characters

All character endpoints require auth and use the `/characters` prefix.

### GET /characters

List all available characters (on disk).

**Response `200 OK`:**

```json
[
  { "slug": "samuel_thompson", "name": "Samuel Thompson", "loaded": true },
  { "slug": "ada_lovelace", "name": "Ada Lovelace", "loaded": false }
]
```

---

### GET /characters/template

Return the character creation schema and raw YAML template for the UI.

**Response `200 OK`:**

```json
{
  "fields": {
    "name": { "type": "string", "required": true, "description": "Character's full name" },
    "era": { "type": "string", "default": "modern", "description": "Time period" },
    "...": "..."
  },
  "template_yaml": "# Raw YAML template string..."
}
```

---

### GET /characters/{slug}

Get basic character info.

**Path parameters:**

| Name | Type | Description |
|------|------|-------------|
| `slug` | `string` | Character slug |

**Response `200 OK`:**

```json
{
  "slug": "samuel_thompson",
  "name": "Samuel Thompson",
  "era": "modern",
  "role": "Bartender",
  "personality_traits": ["warm", "observant", "witty"],
  "loaded": true
}
```

**Errors:**
- `404` — Character not found.

---

### GET /characters/{slug}/full

Get the complete character definition including all fields.

**Path parameters:**

| Name | Type | Description |
|------|------|-------------|
| `slug` | `string` | Character slug |

**Response `200 OK`:**

```json
{
  "slug": "samuel_thompson",
  "name": "Samuel Thompson",
  "era": "modern",
  "setting": "...",
  "role": "Bartender",
  "core_identity": "...",
  "backstory": "...",
  "personality_traits": ["warm", "observant"],
  "speech_patterns": {
    "formality": "casual",
    "verbosity": "medium",
    "text_style": "casual",
    "dialect": "",
    "catchphrases": [],
    "vocabulary_preferences": [],
    "vocabulary_avoidances": [],
    "filler_words": [],
    "example_quotes": []
  },
  "knowledge_domains": [],
  "knowledge_boundaries": [],
  "research_seeds": [],
  "physical_description": "",
  "physical_habits": [],
  "idle_behaviors": [],
  "time_behaviors": {
    "early_morning": "",
    "morning": "",
    "afternoon": "",
    "evening": "",
    "night": ""
  },
  "baseline_emotions": {
    "joy": 0.3, "trust": 0.3, "fear": 0.2, "surprise": 0.2,
    "sadness": 0.2, "disgust": 0.2, "anger": 0.2, "anticipation": 0.3
  },
  "research_enabled": true,
  "backend": "",
  "model": "",
  "temperature": null,
  "loaded": true
}
```

**Errors:**
- `404` — Character not found.

---

### POST /characters

Create a new character from JSON. Writes YAML to disk.

**Request body:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | `string` | yes | — | Character's full name |
| `era` | `string` | no | `"modern"` | Time period |
| `setting` | `string` | no | `""` | First-person setting description |
| `role` | `string` | no | `""` | Occupation or position |
| `core_identity` | `string` | no | `""` | First-person identity paragraph |
| `backstory` | `string` | no | `""` | First-person backstory |
| `personality_traits` | `list[string]` | no | `[]` | 4–6 personality traits |
| `speech_patterns` | `object` | no | defaults | Speech style (see GET full response) |
| `knowledge_domains` | `list[string]` | no | `[]` | Topics they know deeply |
| `knowledge_boundaries` | `list[string]` | no | `[]` | Things they would NOT know |
| `research_seeds` | `list[string]` | no | `[]` | Search terms / URLs for research |
| `physical_description` | `string` | no | `""` | Appearance |
| `physical_habits` | `list[string]` | no | `[]` | Physical mannerisms |
| `idle_behaviors` | `list[string]` | no | `[]` | What they do when idle |
| `time_behaviors` | `object` | no | defaults | Time-of-day behaviors |
| `baseline_emotions` | `object` | no | defaults | Plutchik 8-dimension baseline (0–1) |
| `research_enabled` | `boolean` | no | `true` | Enable research loops |
| `backend` | `string` | no | `""` | LLM backend override |
| `model` | `string` | no | `""` | Model override |
| `temperature` | `float` | no | `null` | Temperature override |
| `is_seed` | `boolean` | no | `false` | Whether this is a seed character |
| `seed_prompt` | `string` | no | `""` | Seed prompt if `is_seed` |

**Response `201 Created`:** Full character response (same shape as GET `/{slug}/full`).

**Errors:**
- `400` — Name is empty.
- `409` — Character with that name already exists.

---

### PUT /characters/{slug}

Update an existing character. Rewrites YAML and unloads engine if loaded.

**Path parameters:**

| Name | Type | Description |
|------|------|-------------|
| `slug` | `string` | Character slug |

**Request body:** Same as POST /characters.

**Response `200 OK`:** Full character response.

**Errors:**
- `404` — Character not found.

---

### DELETE /characters/{slug}

Delete a character YAML file. Unloads engine if loaded.

**Path parameters:**

| Name | Type | Description |
|------|------|-------------|
| `slug` | `string` | Character slug |

**Response `200 OK`:**

```json
{ "status": "deleted", "slug": "samuel_thompson" }
```

**Errors:**
- `404` — Character not found.

---

### POST /characters/{slug}/load

Load a character engine into memory.

**Path parameters:**

| Name | Type | Description |
|------|------|-------------|
| `slug` | `string` | Character slug |

**Response `200 OK`:**

```json
{ "status": "loaded", "slug": "samuel_thompson" }
```

**Errors:**
- `404` — Character not found.

---

### POST /characters/{slug}/unload

Unload a character engine from memory.

**Path parameters:**

| Name | Type | Description |
|------|------|-------------|
| `slug` | `string` | Character slug |

**Response `200 OK`:**

```json
{ "status": "unloaded", "slug": "samuel_thompson" }
```

**Errors:**
- `400` — Character not loaded.

---

### PUT /characters/{slug}/research

Toggle research for a loaded character.

**Path parameters:**

| Name | Type | Description |
|------|------|-------------|
| `slug` | `string` | Character slug |

**Request body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `enabled` | `boolean` | yes | Enable or disable research |

**Response `200 OK`:**

```json
{ "slug": "samuel_thompson", "research_enabled": true }
```

**Errors:**
- `400` — Character not loaded.

---

### POST /characters/boost

AI-generate a full character definition from a brief seed. Returns the generated character for review — does NOT save automatically. The client should POST to `/characters` to save.

**Request body:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `seed` | `string` | yes | — | Brief description (e.g. `"jazz pianist, 1950s Harlem"`) |
| `name` | `string` | no | `""` | Optional name override |
| `era` | `string` | no | `"modern"` | Time period |

**Response `200 OK`:**

```json
{
  "character": { "...full CharacterCreateRequest shape..." },
  "status": "generated"
}
```

**Errors:**
- `400` — No LLM provider configured.
- `500` — Boost generation failed.

---

## Chat

### POST /characters/{slug}/chat

Send a message and get a response from a character.

**Auth:** Required

**Path parameters:**

| Name | Type | Description |
|------|------|-------------|
| `slug` | `string` | Character slug |

**Request body:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `message` | `string` | yes | — | The user's message |
| `user_id` | `string` | no | `"api_user"` | Unique user identifier |
| `user_name` | `string` | no | `"someone"` | Display name |
| `user_description` | `string` | no | `null` | Persona description from mobile app |

**Response `200 OK`:**

```json
{
  "response": "Hey there! What can I get you?",
  "character": "Samuel Thompson",
  "emotion": "joy",
  "emotion_intensity": 0.65
}
```

**Errors:**
- `404` — Character not found.
- `500` — Engine error.

---

### GET /characters/{slug}/history/{user_id}

Get conversation history for a user with a character.

**Auth:** Required

**Path parameters:**

| Name | Type | Description |
|------|------|-------------|
| `slug` | `string` | Character slug |
| `user_id` | `string` | User identifier |

**Query parameters:**

| Name | Type | Default | Description |
|------|------|---------|-------------|
| `limit` | `integer` | `50` | Max turns to return |

**Response `200 OK`:**

```json
[
  {
    "id": "abc123",
    "user_id": "user1",
    "user_name": "Alice",
    "role": "user",
    "content": "Hello!",
    "timestamp": "2026-02-17T12:00:00",
    "entities_mentioned": ["Alice"]
  }
]
```

**Errors:**
- `404` — Character not found.

---

## Observability

All observability endpoints require auth. Character must be loaded (auto-loads on first access).

### GET /characters/{slug}/memory

Memory breakdown for a character.

**Response `200 OK`:**

```json
{
  "total_sqlite": 58,
  "total_vector": 42,
  "by_type": { "EPISODIC": 20, "SEMANTIC": 15, "EMOTIONAL": 5, "CONVERSATION": 18 },
  "by_source": { "CONVERSATION": 30, "GENERATED": 10, "CONSOLIDATED": 8, "RESEARCH": 10 },
  "recent": [
    {
      "id": "mem_abc",
      "content": "User mentioned they like jazz",
      "type": "SEMANTIC",
      "source": "GENERATED",
      "importance": 0.7,
      "created_at": "2026-02-17T12:00:00",
      "access_count": 3,
      "tags": ["music", "preferences"]
    }
  ]
}
```

---

### GET /characters/{slug}/emotions

Current emotional state.

**Response `200 OK`:**

```json
{
  "current": {
    "joy": 0.6, "trust": 0.4, "fear": 0.1, "surprise": 0.2,
    "sadness": 0.1, "disgust": 0.05, "anger": 0.05, "anticipation": 0.5
  },
  "baseline": {
    "joy": 0.3, "trust": 0.3, "fear": 0.2, "surprise": 0.2,
    "sadness": 0.2, "disgust": 0.2, "anger": 0.2, "anticipation": 0.3
  },
  "primary_emotion": "joy",
  "secondary_emotion": "anticipation",
  "intensity": 0.65,
  "valence": 0.72,
  "description": "Feeling genuinely happy and looking forward to what's next"
}
```

---

### GET /characters/{slug}/relationships

All relationships for a character.

**Response `200 OK`:**

```json
{
  "character": "Samuel Thompson",
  "relationships": [
    {
      "user_id": "user1",
      "user_name": "Alice",
      "familiarity": 0.6,
      "trust": 0.5,
      "affection": 0.4,
      "respect": 0.5,
      "interaction_count": 25,
      "character_notes": "Likes jazz, works in tech",
      "topics_discussed": ["music", "work"],
      "first_interaction": "2026-01-10T08:00:00",
      "last_interaction": "2026-02-17T12:00:00"
    }
  ]
}
```

---

### GET /characters/{slug}/relationships/{user_id}

Single relationship detail.

**Path parameters:**

| Name | Type | Description |
|------|------|-------------|
| `slug` | `string` | Character slug |
| `user_id` | `string` | User identifier |

**Response `200 OK`:** Single `RelationshipDetail` object (same shape as array element above).

**Errors:**
- `404` — Relationship not found.

---

### GET /characters/{slug}/knowledge

Knowledge graph statistics.

**Response `200 OK`:**

```json
{
  "node_count": 35,
  "edge_count": 48,
  "entities_by_type": { "PERSON": 10, "PLACE": 8, "CONCEPT": 17 },
  "fact_count": 42,
  "recent_entities": ["Alice", "jazz", "Manhattan"]
}
```

---

### GET /characters/{slug}/research/status

Research loop status.

**Response `200 OK`:**

```json
{
  "enabled": true,
  "seeds_total": 5,
  "seeds_researched": 3,
  "seeds_pending": 2,
  "sources_scraped": 12,
  "total_facts_extracted": 45,
  "total_memories_generated": 30,
  "recent_sources": [
    { "url": "https://example.com", "title": "...", "scraped_at": "..." }
  ]
}
```

---

### GET /characters/{slug}/activity

Background loop activity status.

**Response `200 OK`:**

```json
{
  "loops_running": true,
  "synthesis": { "last_run": "...", "next_run": "...", "runs": 5 },
  "consolidation": { "last_run": "...", "next_run": "...", "runs": 2 },
  "bm25_rebuild": { "last_run": "...", "next_run": "...", "runs": 8 },
  "drip_research": { "last_run": "...", "next_run": "...", "runs": 3 }
}
```

---

## Usage & User Data

### GET /usage

Aggregate token usage across all loaded characters.

**Auth:** Required

**Response `200 OK`:**

```json
{
  "characters": [
    {
      "character": "Samuel Thompson",
      "total_calls": 50,
      "total_input_tokens": 25000,
      "total_output_tokens": 12000,
      "total_tokens": 37000,
      "by_model": [
        { "model": "z-ai/glm-5", "backend": "openrouter", "calls": 50, "input_tokens": 25000, "output_tokens": 12000 }
      ],
      "by_source": [
        { "source": "chat", "calls": 45, "input_tokens": 22000, "output_tokens": 11000 }
      ]
    }
  ],
  "total_calls": 50,
  "total_input_tokens": 25000,
  "total_output_tokens": 12000,
  "total_tokens": 37000
}
```

---

### GET /characters/{slug}/usage

Token usage for a single loaded character. Same shape as each entry in `characters` array above.

**Auth:** Required

**Errors:**
- `400` — Character not loaded.

---

### DELETE /characters/{slug}/users/{user_id}

Delete all data for a user from a loaded character (memories, relationships, conversation history).

**Auth:** Required

**Path parameters:**

| Name | Type | Description |
|------|------|-------------|
| `slug` | `string` | Character slug |
| `user_id` | `string` | User identifier to delete |

**Response `200 OK`:**

```json
{
  "status": "deleted",
  "details": { "memories_deleted": 12, "turns_deleted": 25, "relationship_deleted": true }
}
```

**Errors:**
- `400` — Character not loaded.

---

## WebSocket Endpoints

WebSocket endpoints authenticate via `?token=<API_KEY>` query parameter. Both close with code `1008` if auth fails or setup is required.

### WS /ws/chat/{slug}

Streaming chat with a character.

**Query parameters:**

| Name | Type | Description |
|------|------|-------------|
| `token` | `string` | API key |

**Client sends:**

```json
{
  "type": "message",
  "content": "Hello!",
  "user_id": "user1",
  "user_name": "Alice",
  "user_description": null
}
```

Or a ping:

```json
{ "type": "ping" }
```

**Server sends:**

Stream start:
```json
{ "type": "stream_start", "character": "Samuel Thompson" }
```

Stream chunks (one per token batch):
```json
{ "type": "stream_chunk", "content": "Hey " }
```

Stream end (includes full assembled response):
```json
{
  "type": "stream_end",
  "content": "Hey there! What can I get you?",
  "emotion": "joy",
  "emotion_intensity": 0.65
}
```

Pong:
```json
{ "type": "pong" }
```

Error:
```json
{ "type": "error", "message": "..." }
```

---

### WS /ws/logs/{slug}

Live structured log stream for a character.

**Query parameters:**

| Name | Type | Description |
|------|------|-------------|
| `token` | `string` | API key |

**Server sends** log events as they occur:

```json
{
  "timestamp": "2026-02-17T12:00:00+00:00",
  "level": "INFO",
  "event": "memory_stored",
  "data": { "type": "SEMANTIC", "importance": 0.7 }
}
```

Sends a keepalive every 30 seconds of inactivity:

```json
{
  "timestamp": "...",
  "level": "DEBUG",
  "event": "keepalive",
  "data": {}
}
```
